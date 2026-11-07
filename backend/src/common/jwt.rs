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
