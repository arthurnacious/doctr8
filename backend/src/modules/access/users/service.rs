use std::collections::{BTreeSet, HashMap};

use sqlx::PgPool;
use uuid::Uuid;

use super::{
    dto::{MemberResponse, RegisterRequest, RoleRef, SetMemberRolesRequest, UserResponse},
    repository as repo,
};
use crate::{
    common::{
        auth::{AuthUser, TenantUser},
        db::{self, Scope},
        error::{AppError, AppResult, on_unique},
        password,
        permission::Permission,
    },
    modules::access::roles,
};

#[derive(Clone)]
pub struct UsersService {
    pool: PgPool,
}

impl UsersService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn register(&self, req: RegisterRequest) -> AppResult<UserResponse> {
        let new = req.validate()?;
        let hash = password::hash(new.password.clone()).await?;
        let mut tx = db::begin(&self.pool, Scope::NONE).await?;
        let user = repo::insert_user(&mut tx, &new, &hash)
            .await
            .map_err(on_unique(&[("users_email_key", "An account with this email already exists")]))?;
        tx.commit().await?;
        Ok(user)
    }

    pub async fn me(&self, actor: &AuthUser) -> AppResult<UserResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let user = repo::find_by_id(&mut tx, actor.user_id)
            .await?
            .ok_or(AppError::Unauthorized)?;
        tx.commit().await?;
        Ok(user)
    }

    pub async fn list_members(&self, actor: &TenantUser) -> AppResult<Vec<MemberResponse>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let members = repo::list_members(&mut tx, actor.tenant_id).await?;
        let assignments = repo::list_member_roles(&mut tx, actor.tenant_id, None).await?;
        tx.commit().await?;

        let mut roles_by_user: HashMap<Uuid, Vec<RoleRef>> = HashMap::new();
        for a in assignments {
            roles_by_user
                .entry(a.user_id)
                .or_default()
                .push(RoleRef { id: a.id, name: a.name });
        }

        Ok(members
            .into_iter()
            .map(|m| MemberResponse {
                roles: roles_by_user.remove(&m.user_id).unwrap_or_default(),
                user_id: m.user_id,
                email: m.email,
                full_name: m.full_name,
                status: m.status,
                joined_at: m.joined_at,
            })
            .collect())
    }

    /// Replace a member's roles. Rules:
    /// * every role must belong to this practice;
    /// * you may only add or remove roles whose permissions you hold yourself
    ///   (no privilege escalation, no demoting someone more privileged);
    /// * the practice must keep at least one active Owner.
    pub async fn set_member_roles(
        &self,
        actor: &TenantUser,
        user_id: Uuid,
        req: SetMemberRolesRequest,
    ) -> AppResult<Vec<RoleRef>> {
        let tenant_id = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;

        match repo::membership_status(&mut tx, tenant_id, user_id).await?.as_deref() {
            None | Some("removed") => return Err(AppError::NotFound("member")),
            Some(_) => {}
        }

        let practice_roles = roles::repository::list(&mut tx, tenant_id).await?;
        let by_id: HashMap<Uuid, &roles::repository::RoleRow> =
            practice_roles.iter().map(|r| (r.id, r)).collect();

        let desired: BTreeSet<Uuid> = req.role_ids.into_iter().collect();
        if let Some(unknown) = desired.iter().find(|id| !by_id.contains_key(id)) {
            return Err(AppError::validation(format!("Unknown role {unknown}")));
        }

        let current: BTreeSet<Uuid> = repo::list_member_roles(&mut tx, tenant_id, Some(user_id))
            .await?
            .into_iter()
            .map(|r| r.id)
            .collect();

        let added: Vec<Uuid> = desired.difference(&current).copied().collect();
        let removed: Vec<Uuid> = current.difference(&desired).copied().collect();

        if !added.is_empty() || !removed.is_empty() {
            let changed: Vec<Uuid> = added.iter().chain(&removed).copied().collect();
            let grants = roles::repository::permissions_for_roles(&mut tx, tenant_id, &changed).await?;
            let exceeds_actor = grants
                .iter()
                .any(|g| Permission::from_key(&g.permission_key).is_none_or(|p| !actor.has(p)));
            if exceeds_actor {
                return Err(AppError::Forbidden(
                    "You can only assign or remove roles whose permissions you hold yourself",
                ));
            }

            for role_id in &removed {
                if by_id[role_id].is_system
                    && repo::count_active_holders_excluding(&mut tx, tenant_id, *role_id, user_id).await? == 0
                {
                    return Err(AppError::Conflict(
                        "The practice must keep at least one active Owner".into(),
                    ));
                }
            }

            repo::remove_member_roles(&mut tx, tenant_id, user_id, &removed).await?;
            for role_id in &added {
                repo::add_member_role(&mut tx, tenant_id, user_id, *role_id, actor.user_id).await?;
            }
        }

        let roles = repo::list_member_roles(&mut tx, tenant_id, Some(user_id))
            .await?
            .into_iter()
            .map(|r| RoleRef { id: r.id, name: r.name })
            .collect();
        tx.commit().await?;
        Ok(roles)
    }
}
