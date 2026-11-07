use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use super::{
    dto::{MemberResponse, RegisterRequest, RoleRef, SetMemberRolesRequest, UserResponse},
    service::UsersService,
};
use crate::common::{
    auth::AuthUser,
    error::AppResult,
    guard::Require,
    permission::perm,
};

pub async fn register(
    State(svc): State<UsersService>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.register(req).await?)))
}

pub async fn me(State(svc): State<UsersService>, user: AuthUser) -> AppResult<Json<UserResponse>> {
    Ok(Json(svc.me(&user).await?))
}

pub async fn list_members(
    State(svc): State<UsersService>,
    auth: Require<perm::MembersView>,
) -> AppResult<Json<Vec<MemberResponse>>> {
    Ok(Json(svc.list_members(&auth).await?))
}

pub async fn set_member_roles(
    State(svc): State<UsersService>,
    auth: Require<perm::RolesAssign>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<SetMemberRolesRequest>,
) -> AppResult<Json<Vec<RoleRef>>> {
    Ok(Json(svc.set_member_roles(&auth, user_id, req).await?))
}
