use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use super::{
    dto::{CreateDraftRequest, DraftResponse, UpdateDraftRequest},
    service::DraftsService,
};
use crate::{
    common::{error::AppResult, guard::Require, permission::perm},
    modules::billing::invoices::dto::InvoiceResponse,
};

pub async fn create(
    State(svc): State<DraftsService>,
    auth: Require<perm::BillingManage>,
    Json(req): Json<CreateDraftRequest>,
) -> AppResult<(StatusCode, Json<DraftResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.create(&auth, req).await?)))
}

pub async fn get(State(svc): State<DraftsService>, auth: Require<perm::BillingView>, Path(id): Path<Uuid>) -> AppResult<Json<DraftResponse>> {
    Ok(Json(svc.get(&auth, id).await?))
}

pub async fn list_for_account(
    State(svc): State<DraftsService>,
    auth: Require<perm::BillingView>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<DraftResponse>>> {
    Ok(Json(svc.list_for_account(&auth, id).await?))
}

pub async fn update(
    State(svc): State<DraftsService>,
    auth: Require<perm::BillingManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDraftRequest>,
) -> AppResult<Json<DraftResponse>> {
    Ok(Json(svc.update(&auth, id, req).await?))
}

pub async fn delete(State(svc): State<DraftsService>, auth: Require<perm::BillingManage>, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    svc.delete(&auth, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn finalise(
    State(svc): State<DraftsService>,
    auth: Require<perm::BillingManage>,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<InvoiceResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.finalise(&auth, id).await?)))
}
