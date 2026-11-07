//! Sessions and refresh tokens are global tables (no RLS): they must be
//! readable before we know who the caller is.

use sqlx::PgConnection;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct Credentials {
    pub id: Uuid,
    pub password_hash: Option<String>,
    pub status: String,
}

pub async fn find_credentials_by_email(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<Credentials>, sqlx::Error> {
    sqlx::query_as("SELECT id, password_hash, status FROM users WHERE email = $1::citext")
        .bind(email)
        .fetch_optional(conn)
        .await
}

pub async fn touch_last_login(conn: &mut PgConnection, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn create_session(
    conn: &mut PgConnection,
    user_id: Uuid,
    user_agent: Option<&str>,
    ttl_secs: i64,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO auth_sessions (user_id, user_agent, expires_at)
         VALUES ($1, $2, now() + make_interval(secs => $3))
         RETURNING id",
    )
    .bind(user_id)
    .bind(user_agent)
    .bind(ttl_secs as f64)
    .fetch_one(conn)
    .await
}

pub async fn set_session_tenant(
    conn: &mut PgConnection,
    session_id: Uuid,
    tenant_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE auth_sessions SET tenant_id = $2, last_used_at = now() WHERE id = $1")
        .bind(session_id)
        .bind(tenant_id)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn revoke_session(conn: &mut PgConnection, session_id: Uuid, reason: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE auth_sessions SET revoked_at = now(), revoked_reason = $2
          WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(session_id)
    .bind(reason)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn insert_refresh_token(
    conn: &mut PgConnection,
    session_id: Uuid,
    token_hash: &[u8],
    ttl_secs: i64,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO refresh_tokens (session_id, token_hash, expires_at)
         VALUES ($1, $2, now() + make_interval(secs => $3))
         RETURNING id",
    )
    .bind(session_id)
    .bind(token_hash)
    .bind(ttl_secs as f64)
    .fetch_one(conn)
    .await
}

#[derive(sqlx::FromRow)]
pub struct PresentedToken {
    pub id: Uuid,
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub consumed: bool,
    pub revoked: bool,
    /// Token and session both unexpired (evaluated with the DB clock).
    pub live: bool,
    pub user_active: bool,
}

/// Look up a presented refresh token and lock it and its session, so two
/// concurrent refreshes with the same token are serialised.
pub async fn find_token_for_update(
    conn: &mut PgConnection,
    token_hash: &[u8],
) -> Result<Option<PresentedToken>, sqlx::Error> {
    sqlx::query_as(
        "SELECT rt.id, rt.session_id, s.user_id, s.tenant_id,
                rt.consumed_at IS NOT NULL                    AS consumed,
                s.revoked_at  IS NOT NULL                     AS revoked,
                (rt.expires_at > now() AND s.expires_at > now()) AS live,
                u.status = 'active'                           AS user_active
           FROM refresh_tokens rt
           JOIN auth_sessions s ON s.id = rt.session_id
           JOIN users u         ON u.id = s.user_id
          WHERE rt.token_hash = $1
            FOR UPDATE OF rt, s",
    )
    .bind(token_hash)
    .fetch_optional(conn)
    .await
}

pub async fn consume_token(conn: &mut PgConnection, token_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE refresh_tokens SET consumed_at = now() WHERE id = $1")
        .bind(token_id)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn link_replacement(conn: &mut PgConnection, old_id: Uuid, new_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE refresh_tokens SET replaced_by = $2 WHERE id = $1")
        .bind(old_id)
        .bind(new_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// Union of permission keys from all the user's roles in the practice.
/// Requires app.tenant_id to be set to `tenant_id`.
pub async fn load_permission_keys(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT DISTINCT rp.permission_key
           FROM membership_roles mr
           JOIN role_permissions rp ON rp.tenant_id = mr.tenant_id AND rp.role_id = mr.role_id
          WHERE mr.tenant_id = $1 AND mr.user_id = $2",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_all(conn)
    .await
}
