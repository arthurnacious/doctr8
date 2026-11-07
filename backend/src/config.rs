use anyhow::{Context, bail};

/// Runtime configuration, read from environment variables.
/// Deliberately not `Debug`: it holds the JWT secret.
#[derive(Clone)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub db_max_connections: u32,
    pub jwt_secret: String,
    pub jwt_issuer: String,
    pub access_token_ttl_secs: i64,
    pub refresh_token_ttl_secs: i64,
    /// Absolute session lifetime; refreshing never extends past this.
    pub session_ttl_secs: i64,
    /// `Secure` flag on the refresh cookie. Only disable for local HTTP dev.
    pub cookie_secure: bool,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Build from any key → value source (the environment in production, a
    /// map in tests).
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let required = |key: &str| get(key).with_context(|| format!("{key} must be set"));
        let optional = |key: &str, default: &str| get(key).unwrap_or_else(|| default.to_owned());

        let jwt_secret = required("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            bail!("JWT_SECRET must be at least 32 bytes");
        }

        let config = Self {
            bind_addr: optional("BIND_ADDR", "0.0.0.0:3000"),
            database_url: required("DATABASE_URL")?,
            db_max_connections: parse(&get, "DB_MAX_CONNECTIONS", 10)?,
            jwt_secret,
            jwt_issuer: optional("JWT_ISSUER", "doctr8"),
            access_token_ttl_secs: parse(&get, "ACCESS_TOKEN_TTL_SECS", 600)?,
            refresh_token_ttl_secs: parse(&get, "REFRESH_TOKEN_TTL_SECS", 14 * 24 * 3600)?,
            session_ttl_secs: parse(&get, "SESSION_TTL_SECS", 30 * 24 * 3600)?,
            cookie_secure: parse(&get, "COOKIE_SECURE", true)?,
        };
        if config.access_token_ttl_secs <= 0 || config.refresh_token_ttl_secs <= 0 || config.session_ttl_secs <= 0 {
            bail!("token and session lifetimes must be positive");
        }
        Ok(config)
    }
}

fn parse<T>(get: &impl Fn(&str) -> Option<String>, key: &str, default: T) -> anyhow::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match get(key) {
        Some(v) => v.parse().map_err(|e| anyhow::anyhow!("{key} is invalid: {e}")),
        None => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| map.get(k).cloned()
    }

    const BASE: &[(&str, &str)] = &[
        ("DATABASE_URL", "postgres://localhost/doctr8"),
        ("JWT_SECRET", "0123456789abcdef0123456789abcdef"),
    ];

    fn with(extra: &[(&str, &str)]) -> anyhow::Result<Config> {
        let mut pairs = BASE.to_vec();
        pairs.extend_from_slice(extra);
        Config::from_lookup(lookup(&pairs))
    }

    #[test]
    fn defaults() {
        let c = with(&[]).unwrap();
        assert_eq!(c.bind_addr, "0.0.0.0:3000");
        assert_eq!(c.db_max_connections, 10);
        assert_eq!(c.jwt_issuer, "doctr8");
        assert_eq!(c.access_token_ttl_secs, 600);
        assert_eq!(c.refresh_token_ttl_secs, 14 * 24 * 3600);
        assert_eq!(c.session_ttl_secs, 30 * 24 * 3600);
        assert!(c.cookie_secure, "cookies must be Secure unless explicitly disabled");
    }

    #[test]
    fn overrides() {
        let c = with(&[("BIND_ADDR", "127.0.0.1:9"), ("ACCESS_TOKEN_TTL_SECS", "60"), ("COOKIE_SECURE", "false")]).unwrap();
        assert_eq!(c.bind_addr, "127.0.0.1:9");
        assert_eq!(c.access_token_ttl_secs, 60);
        assert!(!c.cookie_secure);
    }

    #[test]
    fn requires_secret_and_database() {
        let err = Config::from_lookup(lookup(&[("DATABASE_URL", "x")])).err().unwrap();
        assert!(err.to_string().contains("JWT_SECRET"));
        let err = Config::from_lookup(lookup(&[("JWT_SECRET", BASE[1].1)])).err().unwrap();
        assert!(err.to_string().contains("DATABASE_URL"));
    }

    #[test]
    fn rejects_short_secret() {
        let err = Config::from_lookup(lookup(&[("DATABASE_URL", "x"), ("JWT_SECRET", "short")])).err().unwrap();
        assert!(err.to_string().contains("32 bytes"));
    }

    #[test]
    fn rejects_unparseable_and_non_positive_values() {
        assert!(with(&[("DB_MAX_CONNECTIONS", "lots")]).is_err());
        assert!(with(&[("COOKIE_SECURE", "yes please")]).is_err());
        assert!(with(&[("ACCESS_TOKEN_TTL_SECS", "0")]).is_err());
        assert!(with(&[("SESSION_TTL_SECS", "-5")]).is_err());
    }
}
