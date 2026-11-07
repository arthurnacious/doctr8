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
    let cookie = Cookie::build((REFRESH_COOKIE, tokens.refresh_token))
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(SameSite::Strict)
        .path(REFRESH_COOKIE_PATH)
        .max_age(time::Duration::seconds(tokens.refresh_max_age_secs))
        .build();
    (jar.add(cookie), Json(tokens.body))
}
