use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use super::{
    dto::{CreatePatientFileRequest, PatientPage, PatientResponse, PatientSearchRequest, SetStatusRequest, UpdatePatientRequest},
    service::PatientsService,
};
use crate::{
    common::{error::AppResult, guard::Require, permission::perm},
    modules::people::accounts::dto::AccountResponse,
};

pub async fn create_file(
    State(svc): State<PatientsService>,
    auth: Require<perm::PatientsManage>,
    Json(req): Json<CreatePatientFileRequest>,
) -> AppResult<(StatusCode, Json<AccountResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.create_file(&auth, req).await?)))
}

pub async fn get(
    State(svc): State<PatientsService>,
    auth: Require<perm::PatientsView>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PatientResponse>> {
    Ok(Json(svc.get(&auth, id).await?))
}

pub async fn update(
    State(svc): State<PatientsService>,
    auth: Require<perm::PatientsManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePatientRequest>,
) -> AppResult<Json<PatientResponse>> {
    Ok(Json(svc.update(&auth, id, req).await?))
}

pub async fn search(
    State(svc): State<PatientsService>,
    auth: Require<perm::PatientsView>,
    Json(req): Json<PatientSearchRequest>,
) -> AppResult<Json<PatientPage>> {
    Ok(Json(svc.search(&auth, req).await?))
}

pub async fn set_status(
    State(svc): State<PatientsService>,
    auth: Require<perm::PatientsManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<SetStatusRequest>,
) -> AppResult<Json<PatientResponse>> {
    Ok(Json(svc.set_status(&auth, id, req).await?))
}

/// A patient file (the account) as patient staff see it: who pays, the
/// medical aid and everyone on it. Needs patients.view, not accounts.view.
pub async fn get_file(
    State(svc): State<PatientsService>,
    auth: Require<perm::PatientsView>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AccountResponse>> {
    Ok(Json(svc.get_file(&auth, id).await?))
}
