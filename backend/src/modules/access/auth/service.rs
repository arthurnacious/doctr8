use std::sync::Arc;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    dto::{AuthResponse, LoginRequest, SessionTokens},
    repository as repo,
};
use crate::{
    common::{
        db::{self, Scope},
        error::{AppError, AppResult},
        jwt::JwtKeys,
        password,
        permission,
    },
    config::Config,
    modules::access::{tenants, tenants::dto::PracticeSummary, users},
};

/// Which practice the new access token should be for.
enum Target {
    /// Login without a choice: pick the only practice if there's exactly one.
    AutoSingle,
    /// Explicit choice: fail if the user isn't an active member.
    Exact(Uuid),
    /// Refresh: keep the session's practice if still a member, else drop it.
    KeepIfMember(Uuid),
    /// No practice selected.
    Unselected,
}

#[derive(Clone)]
pub struct AuthService {
    pool: PgPool,
    jwt: JwtKeys,
    config: Arc<Config>,
}

impl AuthService {
    pub fn new(pool: PgPool, jwt: JwtKeys, config: Arc<Config>) -> Self {
        Self { pool, jwt, config }
    }

    pub async fn login(&self, req: LoginRequest, user_agent: Option<String>) -> AppResult<SessionTokens> {
        let email = req.email.trim().to_lowercase();

        // Verify the password outside any transaction; argon2 is slow by design.
        let creds = {
            let mut conn = self.pool.acquire().await?;
            repo::find_credentials_by_email(&mut conn, &email).await?
        };
        let stored_hash = creds.as_ref().and_then(|c| c.password_hash.clone());
        let password_ok = password::verify(req.password, stored_hash).await?;

        let creds = match creds {
            Some(c) if password_ok && c.status == "active" => c,
            // Same error for unknown email, wrong password and disabled
            // account, so the response doesn't reveal which accounts exist.
            _ => return Err(AppError::InvalidCredentials),
        };

        let mut tx = db::begin(&self.pool, Scope::user(creds.id)).await?;
        repo::touch_last_login(&mut tx, creds.id).await?;
        let session_id =
            repo::create_session(&mut tx, creds.id, user_agent.as_deref(), self.config.session_ttl_secs).await?;

        let target = req.tenant_id.map_or(Target::AutoSingle, Target::Exact);
        let tokens = self.establish(&mut tx, creds.id, session_id, target, None).await?;
        tx.commit().await?;
        Ok(tokens)
    }

    /// Rotate the refresh token, keeping the session's current practice.
    pub async fn refresh(&self, raw_token: &str) -> AppResult<SessionTokens> {
        self.rotate(raw_token, None).await
    }

    /// Rotate the refresh token and move the session to another practice.
    pub async fn switch_practice(&self, raw_token: &str, tenant_id: Uuid) -> AppResult<SessionTokens> {
        self.rotate(raw_token, Some(tenant_id)).await
    }

    /// Revoke the session behind this refresh token. Idempotent.
    pub async fn logout(&self, raw_token: Option<&str>) -> AppResult<()> {
        let Some(raw) = raw_token else { return Ok(()) };
        let mut tx = db::begin(&self.pool, Scope::NONE).await?;
        if let Some(token) = repo::find_token_for_update(&mut tx, &hash_token(raw)).await? {
            repo::revoke_session(&mut tx, token.session_id, "logout").await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn rotate(&self, raw_token: &str, switch_to: Option<Uuid>) -> AppResult<SessionTokens> {
        let mut tx = db::begin(&self.pool, Scope::NONE).await?;
        let token = repo::find_token_for_update(&mut tx, &hash_token(raw_token))
            .await?
            .ok_or(AppError::Unauthorized)?;

        if token.revoked {
            return Err(AppError::Unauthorized);
        }
        if token.consumed {
            // Someone presented a token that was already rotated: either it
            // was stolen or a client is replaying. Kill the whole session.
            repo::revoke_session(&mut tx, token.session_id, "reuse_detected").await?;
            tx.commit().await?;
            tracing::warn!(
                session_id = %token.session_id,
                user_id = %token.user_id,
                "refresh token reuse detected; session revoked"
            );
            return Err(AppError::Unauthorized);
        }
        if !token.live || !token.user_active {
            return Err(AppError::Unauthorized);
        }

        repo::consume_token(&mut tx, token.id).await?;

        let target = match (switch_to, token.tenant_id) {
            (Some(t), _) => Target::Exact(t),
            (None, Some(t)) => Target::KeepIfMember(t),
            (None, None) => Target::Unselected,
        };
        let tokens = self
            .establish(&mut tx, token.user_id, token.session_id, target, Some(token.id))
            .await?;
        tx.commit().await?;
        Ok(tokens)
    }

    /// Resolve the practice + permissions, record the choice on the session,
    /// issue a fresh refresh token and an access token.
    async fn establish(
        &self,
        conn: &mut PgConnection,
        user_id: Uuid,
        session_id: Uuid,
        target: Target,
        replaces: Option<Uuid>,
    ) -> AppResult<SessionTokens> {
        // User scope: can see own memberships across practices, nothing else.
        db::set_scope(conn, Scope::user(user_id)).await?;

        let user = users::repository::find_by_id(conn, user_id)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let practices = tenants::repository::list_active_for_user(conn, user_id).await?;
        let practice = choose_practice(target, &practices)?;

        let permissions = match &practice {
            Some(p) => {
                // Membership verified above — now it's safe to enter the tenant.
                db::set_scope(conn, Scope::tenant(user_id, p.id)).await?;
                let keys = repo::load_permission_keys(conn, p.id, user_id).await?;
                permission::from_db_keys(keys.iter().map(String::as_str))
            }
            None => Vec::new(),
        };

        repo::set_session_tenant(conn, session_id, practice.as_ref().map(|p| p.id)).await?;

        let (raw_refresh, refresh_hash) = new_refresh_token();
        let ttl = self.config.refresh_token_ttl_secs;
        let new_id = repo::insert_refresh_token(conn, session_id, &refresh_hash, ttl).await?;
        if let Some(old_id) = replaces {
            repo::link_replacement(conn, old_id, new_id).await?;
        }

        let access = self
            .jwt
            .issue(user_id, session_id, practice.as_ref().map(|p| p.id), &permissions)?;

        Ok(SessionTokens {
            body: AuthResponse {
                access_token: access.token,
                token_type: "Bearer",
                expires_in: access.expires_in,
                user,
                practice,
                permissions,
                practices,
            },
            refresh_token: raw_refresh,
            refresh_max_age_secs: ttl,
        })
    }
}

/// Decide which practice the new token is for, given the practices the user
/// is an active member of.
fn choose_practice(target: Target, practices: &[PracticeSummary]) -> AppResult<Option<PracticeSummary>> {
    let find = |id: Uuid| practices.iter().find(|p| p.id == id).cloned();
    Ok(match target {
        Target::Exact(id) => Some(find(id).ok_or(AppError::Forbidden(
            "You are not an active member of that practice",
        ))?),
        Target::KeepIfMember(id) => find(id),
        Target::AutoSingle if practices.len() == 1 => practices.first().cloned(),
        Target::AutoSingle | Target::Unselected => None,
    })
}

/// 256-bit random opaque token. Returns (raw for the client, hash for the DB).
fn new_refresh_token() -> (String, Vec<u8>) {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let raw = URL_SAFE_NO_PAD.encode(bytes);
    let hash = hash_token(&raw);
    (raw, hash)
}

fn hash_token(raw: &str) -> Vec<u8> {
    Sha256::digest(raw.as_bytes()).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn practice(n: u128) -> PracticeSummary {
        PracticeSummary { id: Uuid::from_u128(n), slug: format!("p{n}"), name: format!("Practice {n}") }
    }

    fn chosen(target: Target, practices: &[PracticeSummary]) -> Option<Uuid> {
        choose_practice(target, practices).unwrap().map(|p| p.id)
    }

    #[test]
    fn auto_single_picks_the_only_practice() {
        assert_eq!(chosen(Target::AutoSingle, &[practice(1)]), Some(Uuid::from_u128(1)));
    }

    #[test]
    fn auto_single_leaves_choice_to_user_when_zero_or_many() {
        assert_eq!(chosen(Target::AutoSingle, &[]), None);
        assert_eq!(chosen(Target::AutoSingle, &[practice(1), practice(2)]), None);
    }

    #[test]
    fn exact_requires_active_membership() {
        let ps = [practice(1), practice(2)];
        assert_eq!(chosen(Target::Exact(Uuid::from_u128(2)), &ps), Some(Uuid::from_u128(2)));
        let err = choose_practice(Target::Exact(Uuid::from_u128(3)), &ps).unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[test]
    fn keep_if_member_drops_practice_silently_after_removal() {
        let ps = [practice(1)];
        assert_eq!(chosen(Target::KeepIfMember(Uuid::from_u128(1)), &ps), Some(Uuid::from_u128(1)));
        assert_eq!(chosen(Target::KeepIfMember(Uuid::from_u128(9)), &ps), None);
    }

    #[test]
    fn unselected_selects_nothing() {
        assert_eq!(chosen(Target::Unselected, &[practice(1)]), None);
    }

    #[test]
    fn refresh_tokens_are_256_bit_url_safe_and_hashed() {
        let (raw, hash) = new_refresh_token();
        assert_eq!(raw.len(), 43, "32 bytes base64url without padding");
        assert!(raw.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_eq!(URL_SAFE_NO_PAD.decode(&raw).unwrap().len(), 32);
        assert_eq!(hash.len(), 32, "SHA-256");
        assert_eq!(hash, hash_token(&raw), "stored hash must match a re-hash of the presented token");
        assert_ne!(hash, raw.as_bytes(), "raw token is never what we store");
    }

    #[test]
    fn refresh_tokens_are_unique() {
        let tokens: std::collections::HashSet<String> = (0..200).map(|_| new_refresh_token().0).collect();
        assert_eq!(tokens.len(), 200);
    }

    #[test]
    fn token_hash_is_deterministic_and_input_sensitive() {
        assert_eq!(hash_token("abc"), hash_token("abc"));
        assert_ne!(hash_token("abc"), hash_token("abd"));
    }
}
