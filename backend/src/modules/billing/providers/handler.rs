use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use uuid::Uuid;

use super::{
    dto::{ProviderFilter, ProviderInput, ProviderResponse, ServiceCentreInput, ServiceCentreResponse},
    service::ProvidersService,
};
use crate::common::{error::AppResult, guard::Require, permission::perm};

pub async fn list(
    State(svc): State<ProvidersService>,
    auth: Require<perm::BillingView>,
    Query(filter): Query<ProviderFilter>,
) -> AppResult<Json<Vec<ProviderResponse>>> {
    Ok(Json(svc.list(&auth, filter).await?))
}

pub async fn get(State(svc): State<ProvidersService>, auth: Require<perm::BillingView>, Path(id): Path<Uuid>) -> AppResult<Json<ProviderResponse>> {
    Ok(Json(svc.get(&auth, id).await?))
}

pub async fn create(
    State(svc): State<ProvidersService>,
    auth: Require<perm::ProvidersManage>,
    Json(input): Json<ProviderInput>,
) -> AppResult<(StatusCode, Json<ProviderResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.create(&auth, input).await?)))
}

pub async fn replace(
    State(svc): State<ProvidersService>,
    auth: Require<perm::ProvidersManage>,
    Path(id): Path<Uuid>,
    Json(input): Json<ProviderInput>,
) -> AppResult<Json<ProviderResponse>> {
    Ok(Json(svc.replace(&auth, id, input).await?))
}

#[derive(Deserialize)]
pub struct CentreFilter {
    #[serde(default)]
    include_inactive: bool,
}

pub async fn list_centres(
    State(svc): State<ProvidersService>,
    auth: Require<perm::BillingView>,
    Query(f): Query<CentreFilter>,
) -> AppResult<Json<Vec<ServiceCentreResponse>>> {
    Ok(Json(svc.list_centres(&auth, f.include_inactive).await?))
}

pub async fn create_centre(
    State(svc): State<ProvidersService>,
    auth: Require<perm::ProvidersManage>,
    Json(input): Json<ServiceCentreInput>,
) -> AppResult<(StatusCode, Json<ServiceCentreResponse>)> {
    Ok((StatusCode::CREATED, Json(svc.create_centre(&auth, input).await?)))
}

pub async fn replace_centre(
    State(svc): State<ProvidersService>,
    auth: Require<perm::ProvidersManage>,
    Path(id): Path<Uuid>,
    Json(input): Json<ServiceCentreInput>,
) -> AppResult<Json<ServiceCentreResponse>> {
    Ok(Json(svc.replace_centre(&auth, id, input).await?))
}
