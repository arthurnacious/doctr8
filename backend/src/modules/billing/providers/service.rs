use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    dto::{NewProvider, ProviderFilter, ProviderInput, ProviderKind, ProviderResponse, ServiceCentreInput, ServiceCentreResponse},
    repository as repo,
};
use crate::common::{
    auth::TenantUser,
    db,
    error::{AppError, AppResult, on_unique},
};

const PROVIDER_UNIQUES: &[(&str, &str)] = &[
    ("providers_hpcsa_uq", "A provider with this HPCSA number already exists in this practice"),
    ("providers_user_uq", "That user is already linked to another provider"),
];
const CENTRE_UNIQUES: &[(&str, &str)] = &[("service_centres_name_uq", "A service centre with this name already exists")];

#[derive(Clone)]
pub struct ProvidersService {
    pool: PgPool,
}

impl ProvidersService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, actor: &TenantUser, f: ProviderFilter) -> AppResult<Vec<ProviderResponse>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let rows = repo::list(&mut tx, actor.tenant_id, f.kind.map(ProviderKind::as_str), f.include_inactive).await?;
        tx.commit().await?;
        Ok(rows)
    }

    pub async fn get(&self, actor: &TenantUser, id: Uuid) -> AppResult<ProviderResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let p = find(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(p)
    }

    pub async fn create(&self, actor: &TenantUser, input: ProviderInput) -> AppResult<ProviderResponse> {
        let new = input.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        check_user_link(&mut tx, actor.tenant_id, &new).await?;
        let p = repo::insert(&mut tx, actor.tenant_id, &new, actor.user_id)
            .await
            .map_err(on_unique(PROVIDER_UNIQUES))?;
        tx.commit().await?;
        Ok(p)
    }

    /// Existing invoices keep pointing at the same provider row; editing a
    /// provider's details changes how they print on old invoices too.
    pub async fn replace(&self, actor: &TenantUser, id: Uuid, input: ProviderInput) -> AppResult<ProviderResponse> {
        let new = input.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        check_user_link(&mut tx, actor.tenant_id, &new).await?;
        let p = repo::replace(&mut tx, actor.tenant_id, id, &new)
            .await
            .map_err(on_unique(PROVIDER_UNIQUES))?
            .ok_or(AppError::NotFound("provider"))?;
        tx.commit().await?;
        Ok(p)
    }

    pub async fn list_centres(&self, actor: &TenantUser, include_inactive: bool) -> AppResult<Vec<ServiceCentreResponse>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let rows = repo::list_centres(&mut tx, actor.tenant_id, include_inactive).await?;
        tx.commit().await?;
        Ok(rows)
    }

    pub async fn create_centre(&self, actor: &TenantUser, input: ServiceCentreInput) -> AppResult<ServiceCentreResponse> {
        let new = input.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let c = repo::insert_centre(&mut tx, actor.tenant_id, &new).await.map_err(on_unique(CENTRE_UNIQUES))?;
        tx.commit().await?;
        Ok(c)
    }

    pub async fn replace_centre(&self, actor: &TenantUser, id: Uuid, input: ServiceCentreInput) -> AppResult<ServiceCentreResponse> {
        let new = input.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let c = repo::replace_centre(&mut tx, actor.tenant_id, id, &new)
            .await
            .map_err(on_unique(CENTRE_UNIQUES))?
            .ok_or(AppError::NotFound("service centre"))?;
        tx.commit().await?;
        Ok(c)
    }
}

async fn check_user_link(conn: &mut PgConnection, tenant_id: Uuid, p: &NewProvider) -> AppResult<()> {
    if let Some(user_id) = p.user_id
        && !repo::is_active_member(conn, tenant_id, user_id).await?
    {
        return Err(AppError::validation("user_id must be an active member of this practice"));
    }
    Ok(())
}

pub async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<ProviderResponse> {
    repo::find(conn, tenant_id, id).await?.ok_or(AppError::NotFound("provider"))
}

/// An active provider of the given kind (if `kind` is set), for use on an invoice.
pub async fn usable(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid, kind: Option<ProviderKind>, role: &'static str) -> AppResult<ProviderResponse> {
    let p = repo::find(conn, tenant_id, id).await?.ok_or(AppError::NotFound(role))?;
    if !p.is_active {
        return Err(AppError::validation(format!("The {role} is inactive")));
    }
    if let Some(k) = kind
        && p.kind != k.as_str()
    {
        return Err(AppError::validation(format!("The {role} must be one of the practice's own providers")));
    }
    Ok(p)
}

pub async fn usable_centre(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<ServiceCentreResponse> {
    let c = repo::find_centre(conn, tenant_id, id).await?.ok_or(AppError::NotFound("service centre"))?;
    if !c.is_active {
        return Err(AppError::validation("The service centre is inactive"));
    }
    Ok(c)
}
