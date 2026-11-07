use std::sync::Arc;

use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::{AppError, AppResult},
    permission::Permission,
};

const AUDIENCE: &str = "doctr8-api";

/// Access-token payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// User id.
    pub sub: Uuid,
    /// Session id (auth_sessions.id).
    pub sid: Uuid,
    /// Selected practice. Absent until the user picks one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tid: Option<Uuid>,
    /// Permission keys in `tid`. Unknown keys are ignored on decode.
    #[serde(default)]
    pub perms: Vec<String>,
    pub iss: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
}

pub struct AccessToken {
    pub token: String,
    pub expires_in: i64,
}

#[derive(Clone)]
pub struct JwtKeys {
    inner: Arc<Inner>,
}

struct Inner {
    encoding: EncodingKey,
    decoding: DecodingKey,
    validation: Validation,
    issuer: String,
    ttl_secs: i64,
}

impl JwtKeys {
    pub fn new(secret: &[u8], issuer: &str, ttl_secs: i64) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[issuer]);
        validation.set_audience(&[AUDIENCE]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = 30;

        Self {
            inner: Arc::new(Inner {
                encoding: EncodingKey::from_secret(secret),
                decoding: DecodingKey::from_secret(secret),
                validation,
                issuer: issuer.to_owned(),
                ttl_secs,
            }),
        }
    }

    pub fn issue(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        tenant_id: Option<Uuid>,
        permissions: &[Permission],
    ) -> AppResult<AccessToken> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: user_id,
            sid: session_id,
            tid: tenant_id,
            perms: permissions.iter().map(|p| p.key().to_owned()).collect(),
            iss: self.inner.issuer.clone(),
            aud: AUDIENCE.to_owned(),
            iat: now,
            exp: now + self.inner.ttl_secs,
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &self.inner.encoding)
            .map_err(|e| AppError::Internal(e.into()))?;
        Ok(AccessToken { token, expires_in: self.inner.ttl_secs })
    }

    pub fn verify(&self, token: &str) -> AppResult<Claims> {
        decode::<Claims>(token, &self.inner.decoding, &self.inner.validation)
            .map(|data| data.claims)
            .map_err(|_| AppError::Unauthorized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::{ISSUER, SECRET, SESSION, TENANT, USER, keys};

    #[test]
    fn issue_then_verify_round_trips_claims() {
        let issued = keys().issue(USER, SESSION, Some(TENANT), &[Permission::RolesView, Permission::PatientsView]).unwrap();
        assert_eq!(issued.expires_in, 600);
        let c = keys().verify(&issued.token).unwrap();
        assert_eq!(c.sub, USER);
        assert_eq!(c.sid, SESSION);
        assert_eq!(c.tid, Some(TENANT));
        assert_eq!(c.perms, vec!["roles.view", "patients.view"]);
        assert_eq!(c.iss, ISSUER);
        assert_eq!(c.aud, AUDIENCE);
        assert_eq!(c.exp - c.iat, 600);
    }

    #[test]
    fn tid_is_omitted_until_a_practice_is_selected() {
        let token = keys().issue(USER, SESSION, None, &[]).unwrap().token;
        let payload = token.split('.').nth(1).unwrap();
        use base64::Engine;
        let json = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert!(v.get("tid").is_none());
        assert_eq!(keys().verify(&token).unwrap().tid, None);
    }

    #[test]
    fn expired_tokens_are_rejected_beyond_leeway() {
        let expired = JwtKeys::new(SECRET.as_bytes(), ISSUER, -120).issue(USER, SESSION, None, &[]).unwrap();
        assert!(matches!(keys().verify(&expired.token), Err(AppError::Unauthorized)));
    }

    #[test]
    fn small_clock_skew_is_tolerated() {
        let just_expired = JwtKeys::new(SECRET.as_bytes(), ISSUER, -5).issue(USER, SESSION, None, &[]).unwrap();
        assert!(keys().verify(&just_expired.token).is_ok());
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let other = JwtKeys::new(b"another-secret-another-secret-0000", ISSUER, 600);
        let token = other.issue(USER, SESSION, None, &[]).unwrap().token;
        assert!(keys().verify(&token).is_err());
    }

    #[test]
    fn wrong_issuer_is_rejected() {
        let other = JwtKeys::new(SECRET.as_bytes(), "someone-else", 600);
        let token = other.issue(USER, SESSION, None, &[]).unwrap().token;
        assert!(keys().verify(&token).is_err());
    }

    #[test]
    fn tampered_payload_is_rejected() {
        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let token = keys().issue(USER, SESSION, Some(TENANT), &[]).unwrap().token;
        let parts: Vec<&str> = token.split('.').collect();
        let mut claims: serde_json::Value = serde_json::from_slice(&engine.decode(parts[1]).unwrap()).unwrap();
        claims["perms"] = serde_json::json!(["roles.manage"]);
        let forged = format!("{}.{}.{}", parts[0], engine.encode(claims.to_string()), parts[2]);
        assert!(keys().verify(&forged).is_err());
    }

    #[test]
    fn none_algorithm_is_rejected() {
        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let token = keys().issue(USER, SESSION, Some(TENANT), &[]).unwrap().token;
        let payload = token.split('.').nth(1).unwrap();
        let header = engine.encode(r#"{"alg":"none","typ":"JWT"}"#);
        assert!(keys().verify(&format!("{header}.{payload}.")).is_err());
    }

    #[test]
    fn garbage_is_rejected() {
        for t in ["", "abc", "a.b.c", "Bearer x"] {
            assert!(matches!(keys().verify(t), Err(AppError::Unauthorized)), "{t}");
        }
    }
}
