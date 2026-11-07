use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use uuid::Uuid;

use super::{
    dto::{ClaimResponse, ClaimStatusRequest, ClaimsFilter, CreateInvoiceRequest, CreditNoteRequest, InvoiceListItem, InvoicePage, InvoiceResponse, InvoiceSearchRequest, SubmitClaimRequest},
    service::InvoicesService,
};
use crate::common::{error::AppResult, guard::Require, permission::perm};

pub async fn create(
    State(svc): State<InvoicesService>,
    auth: Require<perm::BillingManage>,
    Json(req): Json<CreateInvoiceRequest>,
) -> AppResult<(StatusCode, Json<InvoiceResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.create(&auth, req).await?)))
}

pub async fn get(State(svc): State<InvoicesService>, auth: Require<perm::BillingView>, Path(id): Path<Uuid>) -> AppResult<Json<InvoiceResponse>> {
    Ok(Json(svc.get(&auth, id).await?))
}

pub async fn list_for_account(
    State(svc): State<InvoicesService>,
    auth: Require<perm::BillingView>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<InvoiceListItem>>> {
    Ok(Json(svc.list_for_account(&auth, id).await?))
}

pub async fn credit(
    State(svc): State<InvoicesService>,
    auth: Require<perm::BillingManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreditNoteRequest>,
) -> AppResult<(StatusCode, Json<InvoiceResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.credit(&auth, id, req).await?)))
}

pub async fn submit_claim(
    State(svc): State<InvoicesService>,
    auth: Require<perm::BillingManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<SubmitClaimRequest>,
) -> AppResult<(StatusCode, Json<ClaimResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.submit_claim(&auth, id, req).await?)))
}

pub async fn set_claim_status(
    State(svc): State<InvoicesService>,
    auth: Require<perm::BillingManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<ClaimStatusRequest>,
) -> AppResult<Json<ClaimResponse>> {
    Ok(Json(svc.set_claim_status(&auth, id, req).await?))
}

pub async fn list_claims(
    State(svc): State<InvoicesService>,
    auth: Require<perm::BillingView>,
    Query(f): Query<ClaimsFilter>,
) -> AppResult<Json<Vec<ClaimResponse>>> {
    Ok(Json(svc.list_claims(&auth, f).await?))
}

/// `POST /invoices/search`: the practice's invoices, filtered and paged.
pub async fn search(
    State(svc): State<InvoicesService>,
    auth: Require<perm::BillingView>,
    Json(req): Json<InvoiceSearchRequest>,
) -> AppResult<Json<InvoicePage>> {
    Ok(Json(svc.search(&auth, req).await?))
}
