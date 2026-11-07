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
        let jwt_secret = required("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            bail!("JWT_SECRET must be at least 32 bytes");
        }

        Ok(Self {
            bind_addr: optional("BIND_ADDR", "0.0.0.0:3000"),
            database_url: required("DATABASE_URL")?,
            db_max_connections: parse("DB_MAX_CONNECTIONS", 10)?,
            jwt_secret,
            jwt_issuer: optional("JWT_ISSUER", "doctr8"),
            access_token_ttl_secs: parse("ACCESS_TOKEN_TTL_SECS", 600)?,
            refresh_token_ttl_secs: parse("REFRESH_TOKEN_TTL_SECS", 14 * 24 * 3600)?,
            session_ttl_secs: parse("SESSION_TTL_SECS", 30 * 24 * 3600)?,
            cookie_secure: parse("COOKIE_SECURE", true)?,
        })
    }
}

fn required(key: &str) -> anyhow::Result<String> {
    std::env::var(key).with_context(|| format!("{key} must be set"))
}

fn optional(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn parse<T>(key: &str, default: T) -> anyhow::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(v) => v
            .parse()
            .map_err(|e| anyhow::anyhow!("{key} is invalid: {e}")),
        Err(_) => Ok(default),
    }
}
