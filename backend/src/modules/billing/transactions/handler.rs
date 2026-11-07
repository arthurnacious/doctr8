use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use uuid::Uuid;

use super::{
    dto::{
        AllocateRemittanceRequest, BalanceResponse, JournalRequest, PaymentRequest, ReasonTypeInput, ReasonTypeResponse,
        ReasonTypeUpdate, RefundRequest, RemittanceFilter, RemittanceRequest, RemittanceResponse, RemittanceSummary,
        ReverseRequest, TransactionResponse, TransferRequest,
    },
    service::TransactionsService,
};
use crate::common::{error::AppResult, guard::Require, permission::perm};

type Created<T> = AppResult<(StatusCode, Json<T>)>;

pub async fn list_for_account(
    State(svc): State<TransactionsService>,
    auth: Require<perm::BillingView>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<TransactionResponse>>> {
    Ok(Json(svc.list_for_account(&auth, id).await?))
}

pub async fn balance(State(svc): State<TransactionsService>, auth: Require<perm::BillingView>, Path(id): Path<Uuid>) -> AppResult<Json<BalanceResponse>> {
    Ok(Json(svc.balance(&auth, id).await?))
}

pub async fn get(State(svc): State<TransactionsService>, auth: Require<perm::BillingView>, Path(id): Path<Uuid>) -> AppResult<Json<TransactionResponse>> {
    Ok(Json(svc.get(&auth, id).await?))
}

pub async fn payment(State(svc): State<TransactionsService>, auth: Require<perm::PaymentsManage>, Json(req): Json<PaymentRequest>) -> Created<TransactionResponse> {
    Ok((StatusCode::CREATED, Json(svc.payment(&auth, req).await?)))
}

pub async fn refund(State(svc): State<TransactionsService>, auth: Require<perm::PaymentsManage>, Json(req): Json<RefundRequest>) -> Created<TransactionResponse> {
    Ok((StatusCode::CREATED, Json(svc.refund(&auth, req).await?)))
}

pub async fn journal(State(svc): State<TransactionsService>, auth: Require<perm::JournalsManage>, Json(req): Json<JournalRequest>) -> Created<TransactionResponse> {
    Ok((StatusCode::CREATED, Json(svc.journal(&auth, req).await?)))
}

pub async fn transfer(State(svc): State<TransactionsService>, auth: Require<perm::JournalsManage>, Json(req): Json<TransferRequest>) -> Created<TransactionResponse> {
    Ok((StatusCode::CREATED, Json(svc.transfer(&auth, req).await?)))
}

/// Reversing any money transaction is a correction, so it needs
/// journals.manage even for payments.
pub async fn reverse(
    State(svc): State<TransactionsService>,
    auth: Require<perm::JournalsManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<ReverseRequest>,
) -> Created<TransactionResponse> {
    Ok((StatusCode::CREATED, Json(svc.reverse(&auth, id, req).await?)))
}

#[derive(Deserialize)]
pub struct ReasonFilter {
    #[serde(default)]
    include_inactive: bool,
}

pub async fn list_reasons(
    State(svc): State<TransactionsService>,
    auth: Require<perm::BillingView>,
    Query(f): Query<ReasonFilter>,
) -> AppResult<Json<Vec<ReasonTypeResponse>>> {
    Ok(Json(svc.list_reasons(&auth, f.include_inactive).await?))
}

pub async fn create_reason(State(svc): State<TransactionsService>, auth: Require<perm::JournalsManage>, Json(input): Json<ReasonTypeInput>) -> Created<ReasonTypeResponse> {
    Ok((StatusCode::CREATED, Json(svc.create_reason(&auth, input).await?)))
}

pub async fn update_reason(
    State(svc): State<TransactionsService>,
    auth: Require<perm::JournalsManage>,
    Path(id): Path<Uuid>,
    Json(input): Json<ReasonTypeUpdate>,
) -> AppResult<Json<ReasonTypeResponse>> {
    Ok(Json(svc.update_reason(&auth, id, input).await?))
}

pub async fn create_remittance(State(svc): State<TransactionsService>, auth: Require<perm::PaymentsManage>, Json(req): Json<RemittanceRequest>) -> Created<RemittanceResponse> {
    Ok((StatusCode::CREATED, Json(svc.create_remittance(&auth, req).await?)))
}

pub async fn list_remittances(
    State(svc): State<TransactionsService>,
    auth: Require<perm::BillingView>,
    Query(f): Query<RemittanceFilter>,
) -> AppResult<Json<Vec<RemittanceSummary>>> {
    Ok(Json(svc.list_remittances(&auth, f).await?))
}

pub async fn get_remittance(State(svc): State<TransactionsService>, auth: Require<perm::BillingView>, Path(id): Path<Uuid>) -> AppResult<Json<RemittanceResponse>> {
    Ok(Json(svc.get_remittance(&auth, id).await?))
}

pub async fn allocate_remittance(
    State(svc): State<TransactionsService>,
    auth: Require<perm::PaymentsManage>,
    Path(id): Path<Uuid>,
    Json(req): Json<AllocateRemittanceRequest>,
) -> Created<RemittanceResponse> {
    Ok((StatusCode::CREATED, Json(svc.allocate_remittance(&auth, id, req).await?)))
}
