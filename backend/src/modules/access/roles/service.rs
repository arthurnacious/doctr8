use std::collections::{BTreeSet, HashMap};

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    dto::{
        CreateRoleRequest, PermissionResponse, RoleResponse, SetPermissionsRequest,
        UpdateRoleRequest,
    },
    repository::{self as repo, RoleRow},
};
use crate::common::{
    auth::TenantUser,
    db,
    error::{AppError, AppResult, on_unique},
    permission::{self, Permission},
};

/// Name of the system role created with every practice.
pub const OWNER_ROLE_NAME: &str = "Owner";

const ROLE_UNIQUES: &[(&str, &str)] = &[("roles_tenant_name_uq", "A role with this name already exists")];

#[derive(Clone)]
pub struct RolesService {
    pool: PgPool,
}

impl RolesService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn catalogue(&self) -> Vec<PermissionResponse> {
        Permission::ALL
            .iter()
            .map(|p| PermissionResponse { key: p.key(), description: p.description() })
            .collect()
    }

    pub async fn list(&self, actor: &TenantUser) -> AppResult<Vec<RoleResponse>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let rows = repo::list(&mut tx, actor.tenant_id).await?;
        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let grants = repo::permissions_for_roles(&mut tx, actor.tenant_id, &ids).await?;
        tx.commit().await?;

        let mut by_role: HashMap<Uuid, Vec<&str>> = HashMap::new();
        for g in &grants {
            by_role.entry(g.role_id).or_default().push(&g.permission_key);
        }
        Ok(rows
            .into_iter()
            .map(|row| {
                let perms = permission::from_db_keys(by_role.remove(&row.id).unwrap_or_default());
                to_response(row, perms)
            })
            .collect())
    }

    pub async fn get(&self, actor: &TenantUser, id: Uuid) -> AppResult<RoleResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let role = load(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(role)
    }

    pub async fn create(&self, actor: &TenantUser, req: CreateRoleRequest) -> AppResult<RoleResponse> {
        let new = req.validate()?;
        ensure_actor_holds(actor, new.permissions.iter())?;

        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let id = repo::insert_role(&mut tx, actor.tenant_id, &new.name, new.description.as_deref(), false)
            .await
            .map_err(on_unique(ROLE_UNIQUES))?;
        repo::add_permissions(&mut tx, actor.tenant_id, id, &keys(new.permissions.iter()), actor.user_id).await?;
        let role = load(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(role)
    }

    pub async fn update(&self, actor: &TenantUser, id: Uuid, req: UpdateRoleRequest) -> AppResult<RoleResponse> {
        let changes = req.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;

        let existing = find(&mut tx, actor.tenant_id, id).await?;
        if existing.is_system && changes.name.as_deref().is_some_and(|n| n != existing.name) {
            return Err(AppError::Forbidden("System roles cannot be renamed"));
        }

        repo::update_role(
            &mut tx,
            actor.tenant_id,
            id,
            changes.name.as_deref(),
            changes.description.as_ref().map(|d| d.as_deref()),
        )
        .await
        .map_err(on_unique(ROLE_UNIQUES))?;

        let role = load(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(role)
    }

    pub async fn delete(&self, actor: &TenantUser, id: Uuid) -> AppResult<()> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let existing = find(&mut tx, actor.tenant_id, id).await?;
        if existing.is_system {
            return Err(AppError::Forbidden("System roles cannot be deleted"));
        }
        if existing.member_count > 0 {
            return Err(AppError::Conflict(format!(
                "This role is assigned to {} member(s). Remove it from them first.",
                existing.member_count
            )));
        }
        repo::delete_role(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Replace a role's permissions. You can only grant or revoke permissions
    /// you hold yourself, and the Owner role is fixed.
    pub async fn set_permissions(
        &self,
        actor: &TenantUser,
        id: Uuid,
        req: SetPermissionsRequest,
    ) -> AppResult<RoleResponse> {
        let desired = permission::parse_keys(&req.permissions)?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;

        let existing = find(&mut tx, actor.tenant_id, id).await?;
        if existing.is_system {
            return Err(AppError::Forbidden("The Owner role always has every permission"));
        }

        let current: BTreeSet<Permission> = permission::from_db_keys(
            repo::permissions_for_roles(&mut tx, actor.tenant_id, &[id])
                .await?
                .iter()
                .map(|g| g.permission_key.as_str()),
        )
        .into_iter()
        .collect();

        let added: Vec<Permission> = desired.difference(&current).copied().collect();
        let removed: Vec<Permission> = current.difference(&desired).copied().collect();
        ensure_actor_holds(actor, added.iter().chain(&removed))?;

        repo::remove_permissions(&mut tx, actor.tenant_id, id, &keys(removed.iter())).await?;
        repo::add_permissions(&mut tx, actor.tenant_id, id, &keys(added.iter()), actor.user_id).await?;

        let role = load(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(role)
    }
}

fn ensure_actor_holds<'a>(actor: &TenantUser, perms: impl IntoIterator<Item = &'a Permission>) -> AppResult<()> {
    let missing: Vec<&str> = perms.into_iter().filter(|p| !actor.has(**p)).map(|p| p.key()).collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(AppError::Forbidden("You can only grant or revoke permissions you hold yourself"))
    }
}

fn keys<'a>(perms: impl IntoIterator<Item = &'a Permission>) -> Vec<String> {
    perms.into_iter().map(|p| p.key().to_owned()).collect()
}

async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<RoleRow> {
    repo::find(conn, tenant_id, id).await?.ok_or(AppError::NotFound("role"))
}

async fn load(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<RoleResponse> {
    let row = find(conn, tenant_id, id).await?;
    let grants = repo::permissions_for_roles(conn, tenant_id, &[id]).await?;
    let perms = permission::from_db_keys(grants.iter().map(|g| g.permission_key.as_str()));
    Ok(to_response(row, perms))
}

fn to_response(row: RoleRow, permissions: Vec<Permission>) -> RoleResponse {
    RoleResponse {
        id: row.id,
        name: row.name,
        description: row.description,
        is_system: row.is_system,
        permissions,
        member_count: row.member_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
