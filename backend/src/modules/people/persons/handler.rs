use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use super::{
    dto::{PersonFile, PersonInput, PersonResponse, PersonSummary, SearchPersonsRequest},
    service::PersonsService,
};
use crate::common::{error::AppResult, guard::Require, permission::perm};

pub async fn create(
    State(svc): State<PersonsService>,
    auth: Require<perm::PatientsManage>,
    Json(input): Json<PersonInput>,
) -> AppResult<(StatusCode, Json<PersonResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.create(&auth, input).await?)))
}

pub async fn get(
    State(svc): State<PersonsService>,
    auth: Require<perm::PatientsView>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PersonResponse>> {
    Ok(Json(svc.get(&auth, id).await?))
}

pub async fn replace(
    State(svc): State<PersonsService>,
    auth: Require<perm::PatientsManage>,
    Path(id): Path<Uuid>,
    Json(input): Json<PersonInput>,
) -> AppResult<Json<PersonResponse>> {
    Ok(Json(svc.replace(&auth, id, input).await?))
}

pub async fn search(
    State(svc): State<PersonsService>,
    auth: Require<perm::PatientsView>,
    Json(req): Json<SearchPersonsRequest>,
) -> AppResult<Json<Vec<PersonSummary>>> {
    Ok(Json(svc.search(&auth, req).await?))
}

pub async fn files(
    State(svc): State<PersonsService>,
    auth: Require<perm::PatientsView>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<PersonFile>>> {
    Ok(Json(svc.files(&auth, id).await?))
}
