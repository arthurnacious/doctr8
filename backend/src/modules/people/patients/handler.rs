use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use super::{
    dto::{CreatePatientFileRequest, PatientResponse, UpdatePatientRequest},
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
