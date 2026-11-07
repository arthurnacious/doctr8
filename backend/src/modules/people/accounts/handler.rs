use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use super::{
    dto::{AccountResponse, AddPatientRequest, UpdateAccountRequest},
    service::AccountsService,
};
use crate::common::{error::AppResult, guard::Require, permission::perm};

pub async fn get(
    State(svc): State<AccountsService>,
    auth: Require<perm::AccountsView>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AccountResponse>> {
    Ok(Json(svc.get(&auth, id).await?))
}

pub async fn update(
    State(svc): State<AccountsService>,
    auth: Require<perm::AccountsManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAccountRequest>,
) -> AppResult<Json<AccountResponse>> {
    Ok(Json(svc.update(&auth, id, req).await?))
}

pub async fn add_patient(
    State(svc): State<AccountsService>,
    auth: Require<perm::PatientsManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddPatientRequest>,
) -> AppResult<(StatusCode, Json<AccountResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.add_patient(&auth, id, req).await?)))
}
