use axum::{
    Json,
    extract::{Query, State},
};

use super::{
    dto::{Icd10Code, Modifier, NappiCode, SearchQuery, TariffCode, TariffPrice, TariffPriceQuery},
    service::CodingService,
};
use crate::common::{auth::TenantUser, error::AppResult};

// Reference data: any member signed in to a practice may search it.

pub async fn icd10(State(svc): State<CodingService>, user: TenantUser, Query(q): Query<SearchQuery>) -> AppResult<Json<Vec<Icd10Code>>> {
    Ok(Json(svc.icd10(&user, q).await?))
}

pub async fn tariffs(State(svc): State<CodingService>, user: TenantUser, Query(q): Query<SearchQuery>) -> AppResult<Json<Vec<TariffCode>>> {
    Ok(Json(svc.tariffs(&user, q).await?))
}

pub async fn nappi(State(svc): State<CodingService>, user: TenantUser, Query(q): Query<SearchQuery>) -> AppResult<Json<Vec<NappiCode>>> {
    Ok(Json(svc.nappi(&user, q).await?))
}

pub async fn modifiers(State(svc): State<CodingService>, user: TenantUser, Query(q): Query<SearchQuery>) -> AppResult<Json<Vec<Modifier>>> {
    Ok(Json(svc.modifiers(&user, q).await?))
}

/// `GET /coding/tariff-price?code=0190&plan_id=…&date=…`
pub async fn tariff_price(State(svc): State<CodingService>, user: TenantUser, Query(q): Query<TariffPriceQuery>) -> AppResult<Json<TariffPrice>> {
    Ok(Json(svc.tariff_price(&user, q).await?))
}
