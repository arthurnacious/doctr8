use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use super::{
    dto::{CreateMembershipRequest, MembershipResponse, SchemeResponse, UpdateMembershipRequest},
    service::MembershipsService,
};
use crate::common::{auth::TenantUser, error::AppResult, guard::Require, permission::perm};

/// Reference data — any signed-in member of the practice may read it.
pub async fn schemes(
    State(svc): State<MembershipsService>,
    user: TenantUser,
) -> AppResult<Json<Vec<SchemeResponse>>> {
    Ok(Json(svc.schemes(&user).await?))
}

pub async fn create(
    State(svc): State<MembershipsService>,
    auth: Require<perm::AccountsManage>,
    Json(req): Json<CreateMembershipRequest>,
) -> AppResult<(StatusCode, Json<MembershipResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.create(&auth, req).await?)))
}

pub async fn get(
    State(svc): State<MembershipsService>,
    auth: Require<perm::AccountsView>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<MembershipResponse>> {
    Ok(Json(svc.get(&auth, id).await?))
}

pub async fn update(
    State(svc): State<MembershipsService>,
    auth: Require<perm::AccountsManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateMembershipRequest>,
) -> AppResult<Json<MembershipResponse>> {
    Ok(Json(svc.update(&auth, id, req).await?))
}
