use sqlx::PgPool;
use uuid::Uuid;

use super::{
    dto::{CreateTenantRequest, PracticeSummary, TenantResponse, UpdateTenantRequest},
    repository as repo,
};
use crate::{
    common::{
        auth::{AuthUser, TenantUser},
        db::{self, Scope},
        error::{AppError, AppResult, on_unique},
        permission::Permission,
    },
    modules::access::{roles, users},
};

const TENANT_UNIQUES: &[(&str, &str)] = &[
    ("tenants_slug_key", "That practice address is already taken"),
    ("tenants_pcns_number_key", "A practice with this PCNS number is already registered"),
];

#[derive(Clone)]
pub struct TenantsService {
    pool: PgPool,
}

impl TenantsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a practice with the caller as its first Owner.
    ///
    /// We generate the tenant id up front and set it as the RLS tenant before
    /// inserting, so the `current_tenant_insert` policy is satisfied and every
    /// follow-up insert (role, membership, assignment) lands in this tenant.
    pub async fn create(&self, actor: &AuthUser, req: CreateTenantRequest) -> AppResult<TenantResponse> {
        let new = req.validate()?;
        let tenant_id = Uuid::new_v4();
        let mut tx = db::begin(&self.pool, Scope::tenant(actor.user_id, tenant_id)).await?;

        let tenant = repo::insert(&mut tx, tenant_id, &new)
            .await
            .map_err(on_unique(TENANT_UNIQUES))?;

        let owner_role = roles::repository::insert_role(
            &mut tx,
            tenant_id,
            roles::service::OWNER_ROLE_NAME,
            Some("Full access to the practice"),
            true,
        )
        .await?;
        let all: Vec<String> = Permission::ALL.iter().map(|p| p.key().to_owned()).collect();
        roles::repository::add_permissions(&mut tx, tenant_id, owner_role, &all, actor.user_id).await?;

        users::repository::insert_membership(&mut tx, tenant_id, actor.user_id, "active").await?;
        users::repository::add_member_role(&mut tx, tenant_id, actor.user_id, owner_role, actor.user_id)
            .await?;

        tx.commit().await?;
        Ok(tenant)
    }

    pub async fn list_mine(&self, actor: &AuthUser) -> AppResult<Vec<PracticeSummary>> {
        let mut tx = db::begin(&self.pool, Scope::user(actor.user_id)).await?;
        let practices = repo::list_active_for_user(&mut tx, actor.user_id).await?;
        tx.commit().await?;
        Ok(practices)
    }

    pub async fn current(&self, actor: &TenantUser) -> AppResult<TenantResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let tenant = repo::find(&mut tx, actor.tenant_id)
            .await?
            .ok_or(AppError::NotFound("practice"))?;
        tx.commit().await?;
        Ok(tenant)
    }

    pub async fn update_current(&self, actor: &TenantUser, req: UpdateTenantRequest) -> AppResult<TenantResponse> {
        let changes = req.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let tenant = repo::update(&mut tx, actor.tenant_id, &changes)
            .await
            .map_err(on_unique(TENANT_UNIQUES))?
            .ok_or(AppError::NotFound("practice"))?;
        tx.commit().await?;
        Ok(tenant)
    }
}
