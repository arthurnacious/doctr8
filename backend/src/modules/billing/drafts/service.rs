use sqlx::PgPool;
use uuid::Uuid;

use super::{
    dto::{BillingContext, CreateDraftRequest, DraftResponse, UpdateDraftRequest, as_invoice_request, check_content},
    repository::{self as repo, DraftRow},
};
use crate::{
    common::{
        auth::TenantUser,
        db,
        error::{AppError, AppResult},
        ws::EventBus,
    },
    modules::{
        billing::{
            invoices::{self, dto::InvoiceResponse},
            today, transactions,
        },
        people,
    },
};

#[derive(Clone)]
pub struct DraftsService {
    pool: PgPool,
    bus: EventBus,
}

impl DraftsService {
    pub fn new(pool: PgPool, bus: EventBus) -> Self {
        Self { pool, bus }
    }

    pub async fn create(&self, actor: &TenantUser, req: CreateDraftRequest) -> AppResult<DraftResponse> {
        let content = check_content(&req.content)?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let account = transactions::repository::account_billing(&mut tx, actor.tenant_id, req.account_id)
            .await?
            .ok_or(AppError::NotFound("account"))?;
        if account.status != "open" {
            return Err(AppError::Conflict("Invoices can't be added to a closed account".into()));
        }
        let row = repo::insert(&mut tx, actor.tenant_id, req.account_id, &content, actor.user_id).await?;
        tx.commit().await?;
        response(row)
    }

    pub async fn context(&self, actor: &TenantUser, account_id: Uuid) -> AppResult<BillingContext> {
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let account = people::accounts::repository::find(&mut tx, t, account_id).await?.ok_or(AppError::NotFound("account"))?;
        let out = BillingContext {
            account_id,
            account_no: account.account_no,
            status: account.status,
            medical_aid: repo::context_medical_aid(&mut tx, t, account_id).await?,
            patients: repo::context_patients(&mut tx, t, account_id).await?,
        };
        tx.commit().await?;
        Ok(out)
    }

    pub async fn get(&self, actor: &TenantUser, id: Uuid) -> AppResult<DraftResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let row = repo::find(&mut tx, actor.tenant_id, id, false).await?.ok_or(AppError::NotFound("draft"))?;
        tx.commit().await?;
        response(row)
    }

    pub async fn list_for_account(&self, actor: &TenantUser, account_id: Uuid) -> AppResult<Vec<DraftResponse>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        if transactions::repository::account_billing(&mut tx, actor.tenant_id, account_id).await?.is_none() {
            return Err(AppError::NotFound("account"));
        }
        let rows = repo::for_account(&mut tx, actor.tenant_id, account_id).await?;
        tx.commit().await?;
        rows.into_iter().map(response).collect()
    }

    pub async fn update(&self, actor: &TenantUser, id: Uuid, req: UpdateDraftRequest) -> AppResult<DraftResponse> {
        let content = check_content(&req.content)?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let current = repo::find(&mut tx, actor.tenant_id, id, true).await?.ok_or(AppError::NotFound("draft"))?;
        if let Some(expected) = req.expected_updated_at
            && expected != current.updated_at
        {
            return Err(AppError::Conflict("Someone else changed this draft. Reload it to see their changes.".into()));
        }
        let row = repo::update(&mut tx, actor.tenant_id, id, &content).await?.ok_or(AppError::NotFound("draft"))?;
        tx.commit().await?;
        response(row)
    }

    pub async fn delete(&self, actor: &TenantUser, id: Uuid) -> AppResult<()> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        if !repo::delete(&mut tx, actor.tenant_id, id).await? {
            return Err(AppError::NotFound("draft"));
        }
        tx.commit().await?;
        Ok(())
    }

    /// Validate the draft, post it as an invoice and delete the draft, all
    /// or nothing. A failed finalise leaves the draft as it was.
    pub async fn finalise(&self, actor: &TenantUser, id: Uuid) -> AppResult<InvoiceResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let draft = repo::find(&mut tx, actor.tenant_id, id, true).await?.ok_or(AppError::NotFound("draft"))?;
        let content: serde_json::Value = serde_json::from_str(&draft.content).map_err(anyhow::Error::from)?;
        let inv = as_invoice_request(draft.account_id, &content)?.validate(today())?;
        let invoice_id = invoices::service::post_invoice(&mut tx, &self.bus, actor, &inv).await?;
        repo::delete(&mut tx, actor.tenant_id, id).await?;
        let out = invoices::service::load(&mut tx, actor.tenant_id, invoice_id).await?;
        tx.commit().await?;
        Ok(out)
    }
}

fn response(r: DraftRow) -> AppResult<DraftResponse> {
    Ok(DraftResponse {
        content: serde_json::from_str(&r.content).map_err(anyhow::Error::from)?,
        id: r.id,
        account_id: r.account_id,
        created_by: r.created_by,
        created_at: r.created_at,
        updated_at: r.updated_at,
    })
}
