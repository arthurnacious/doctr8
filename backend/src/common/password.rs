//! Argon2id hashing, run off the async runtime.

use std::sync::OnceLock;

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString,
};
use rand::rngs::OsRng;

use crate::common::error::AppResult;

pub const MIN_LENGTH: usize = 12;
/// Upper bound so a huge password can't be used to burn CPU.
pub const MAX_LENGTH: usize = 256;

pub async fn hash(password: String) -> AppResult<String> {
    let hashed = tokio::task::spawn_blocking(move || hash_sync(&password))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(hashed)
}

/// Verify `password` against `stored`. When there is no stored hash (unknown
/// email, invite not accepted) we still run a full verification against a
/// dummy hash so response time doesn't reveal whether the account exists.
pub async fn verify(password: String, stored: Option<String>) -> AppResult<bool> {
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let real = stored.is_some();
        let hash = stored.unwrap_or_else(|| dummy_hash().to_owned());
        let parsed = PasswordHash::new(&hash).map_err(|e| anyhow::anyhow!("{e}"))?;
        let matches = Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok();
        Ok(real && matches)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(ok)
}

fn hash_sync(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| hash_sync("doctr8-timing-equaliser").expect("hashing dummy password"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hash_then_verify() {
        let h = hash("correct horse battery".into()).await.unwrap();
        assert!(h.starts_with("$argon2id$"), "argon2id PHC string");
        assert!(verify("correct horse battery".into(), Some(h.clone())).await.unwrap());
        assert!(!verify("correct horse batterY".into(), Some(h)).await.unwrap());
    }

    #[tokio::test]
    async fn hashes_are_salted() {
        let a = hash("same password here".into()).await.unwrap();
        let b = hash("same password here".into()).await.unwrap();
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn missing_hash_never_verifies() {
        // Unknown user / invite not accepted: even the dummy password fails.
        assert!(!verify("doctr8-timing-equaliser".into(), None).await.unwrap());
        assert!(!verify("anything".into(), None).await.unwrap());
    }

    #[tokio::test]
    async fn corrupt_stored_hash_is_an_error_not_a_match() {
        assert!(verify("x".into(), Some("not-a-phc-string".into())).await.is_err());
    }

    // Compile-time: a policy change here should be deliberate.
    const _: () = assert!(MIN_LENGTH == 12 && MAX_LENGTH >= 128);
}
