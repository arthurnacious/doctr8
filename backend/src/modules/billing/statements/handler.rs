use axum::{
    Json,
    extract::{Path, Query, State},
};
use uuid::Uuid;

use super::{
    dto::{StatementQuery, StatementResponse},
    service::StatementsService,
};
use crate::common::{error::AppResult, guard::Require, permission::perm};

/// `GET /accounts/{id}/statement?as_of=2026-11-07&include_reversed=false&outstanding_only=false`
pub async fn statement(
    State(svc): State<StatementsService>,
    auth: Require<perm::BillingView>,
    Path(id): Path<Uuid>,
    Query(q): Query<StatementQuery>,
) -> AppResult<Json<StatementResponse>> {
    Ok(Json(svc.statement(&auth, id, q).await?))
}
