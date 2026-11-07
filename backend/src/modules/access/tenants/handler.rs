use axum::{Json, extract::State, http::StatusCode};

use super::{
    dto::{CreateTenantRequest, PracticeSummary, TenantResponse, UpdateTenantRequest},
    service::TenantsService,
};
use crate::common::{
    auth::AuthUser,
    error::AppResult,
    guard::Require,
    permission::perm,
};

pub async fn create(
    State(svc): State<TenantsService>,
    user: AuthUser,
    Json(req): Json<CreateTenantRequest>,
) -> AppResult<(StatusCode, Json<TenantResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.create(&user, req).await?)))
}

pub async fn list_mine(
    State(svc): State<TenantsService>,
    user: AuthUser,
) -> AppResult<Json<Vec<PracticeSummary>>> {
    Ok(Json(svc.list_mine(&user).await?))
}

pub async fn current(
    State(svc): State<TenantsService>,
    auth: Require<perm::PracticeView>,
) -> AppResult<Json<TenantResponse>> {
    Ok(Json(svc.current(&auth).await?))
}

pub async fn update_current(
    State(svc): State<TenantsService>,
    auth: Require<perm::PracticeManage>,
    Json(req): Json<UpdateTenantRequest>,
) -> AppResult<Json<TenantResponse>> {
    Ok(Json(svc.update_current(&auth, req).await?))
}
