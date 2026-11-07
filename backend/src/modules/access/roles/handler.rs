use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use super::{
    dto::{
        CreateRoleRequest, PermissionResponse, RoleResponse, SetPermissionsRequest,
        UpdateRoleRequest,
    },
    service::RolesService,
};
use crate::common::{error::AppResult, guard::Require, permission::perm};

pub async fn catalogue(
    State(svc): State<RolesService>,
    _auth: Require<perm::RolesView>,
) -> Json<Vec<PermissionResponse>> {
    Json(svc.catalogue())
}

pub async fn list(
    State(svc): State<RolesService>,
    auth: Require<perm::RolesView>,
) -> AppResult<Json<Vec<RoleResponse>>> {
    Ok(Json(svc.list(&auth).await?))
}

pub async fn get(
    State(svc): State<RolesService>,
    auth: Require<perm::RolesView>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<RoleResponse>> {
    Ok(Json(svc.get(&auth, id).await?))
}

pub async fn create(
    State(svc): State<RolesService>,
    auth: Require<perm::RolesManage>,
    Json(req): Json<CreateRoleRequest>,
) -> AppResult<(StatusCode, Json<RoleResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.create(&auth, req).await?)))
}

pub async fn update(
    State(svc): State<RolesService>,
    auth: Require<perm::RolesManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateRoleRequest>,
) -> AppResult<Json<RoleResponse>> {
    Ok(Json(svc.update(&auth, id, req).await?))
}

pub async fn delete(
    State(svc): State<RolesService>,
    auth: Require<perm::RolesManage>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    svc.delete(&auth, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_permissions(
    State(svc): State<RolesService>,
    auth: Require<perm::RolesManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<SetPermissionsRequest>,
) -> AppResult<Json<RoleResponse>> {
    Ok(Json(svc.set_permissions(&auth, id, req).await?))
}
