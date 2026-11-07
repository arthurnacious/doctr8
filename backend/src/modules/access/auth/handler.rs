use std::sync::Arc;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header::USER_AGENT},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

use super::{
    dto::{AuthResponse, LoginRequest, SessionTokens, SwitchPracticeRequest},
    service::AuthService,
};
use crate::{
    common::error::{AppError, AppResult},
    config::Config,
};

const REFRESH_COOKIE: &str = "doctr8_rt";
const REFRESH_COOKIE_PATH: &str = "/auth";

pub async fn login(
    State(svc): State<AuthService>,
    State(config): State<Arc<Config>>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> AppResult<(CookieJar, Json<AuthResponse>)> {
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(512).collect());
    let tokens = svc.login(req, user_agent).await?;
    Ok(respond(jar, &config, tokens))
}

pub async fn refresh(
    State(svc): State<AuthService>,
    State(config): State<Arc<Config>>,
    jar: CookieJar,
) -> AppResult<(CookieJar, Json<AuthResponse>)> {
    let raw = refresh_token(&jar)?;
    let tokens = svc.refresh(&raw).await?;
    Ok(respond(jar, &config, tokens))
}

pub async fn switch_practice(
    State(svc): State<AuthService>,
    State(config): State<Arc<Config>>,
    jar: CookieJar,
    Json(req): Json<SwitchPracticeRequest>,
) -> AppResult<(CookieJar, Json<AuthResponse>)> {
    let raw = refresh_token(&jar)?;
    let tokens = svc.switch_practice(&raw, req.tenant_id).await?;
    Ok(respond(jar, &config, tokens))
}

pub async fn logout(
    State(svc): State<AuthService>,
    jar: CookieJar,
) -> AppResult<(CookieJar, StatusCode)> {
    let raw = jar.get(REFRESH_COOKIE).map(|c| c.value().to_owned());
    svc.logout(raw.as_deref()).await?;
    let jar = jar.remove(Cookie::build(REFRESH_COOKIE).path(REFRESH_COOKIE_PATH));
    Ok((jar, StatusCode::NO_CONTENT))
}

fn refresh_token(jar: &CookieJar) -> AppResult<String> {
    jar.get(REFRESH_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or(AppError::Unauthorized)
}

fn respond(jar: CookieJar, config: &Config, tokens: SessionTokens) -> (CookieJar, Json<AuthResponse>) {
    let cookie = refresh_cookie(config, tokens.refresh_token, tokens.refresh_max_age_secs);
    (jar.add(cookie), Json(tokens.body))
}

/// HttpOnly (no JS access), SameSite=Strict (no cross-site sends), and only
/// sent to /auth so ordinary API calls never carry it.
fn refresh_cookie(config: &Config, value: String, max_age_secs: i64) -> Cookie<'static> {
    Cookie::build((REFRESH_COOKIE, value))
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(SameSite::Strict)
        .path(REFRESH_COOKIE_PATH)
        .max_age(time::Duration::seconds(max_age_secs))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support;

    #[test]
    fn refresh_cookie_is_locked_down() {
        let c = refresh_cookie(&test_support::config(), "tok".into(), 3600);
        assert_eq!(c.name(), REFRESH_COOKIE);
        assert_eq!(c.value(), "tok");
        assert_eq!(c.http_only(), Some(true));
        assert_eq!(c.secure(), Some(true));
        assert_eq!(c.same_site(), Some(SameSite::Strict));
        assert_eq!(c.path(), Some("/auth"));
        assert_eq!(c.max_age(), Some(time::Duration::seconds(3600)));
    }

    #[test]
    fn secure_flag_follows_config() {
        let mut config = test_support::config();
        config.cookie_secure = false;
        assert_eq!(refresh_cookie(&config, "t".into(), 1).secure(), Some(false));
    }

    #[test]
    fn refresh_token_read_from_cookie() {
        let jar = CookieJar::new();
        assert!(matches!(refresh_token(&jar), Err(AppError::Unauthorized)));
        let jar = jar.add(Cookie::new(REFRESH_COOKIE, "abc"));
        assert_eq!(refresh_token(&jar).unwrap(), "abc");
    }
}
