use std::collections::HashMap;

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    dto::{
        CreateMembershipRequest, MembershipResponse, NewMembership, PlanResponse, SchemeResponse,
        UpdateMembershipRequest,
    },
    repository as repo,
};
use crate::{
    common::{
        auth::TenantUser,
        db,
        error::{AppError, AppResult, on_unique},
        ws::EventBus,
    },
    modules::people::{accounts, persons},
};

const MEMBERSHIP_UNIQUES: &[(&str, &str)] = &[(
    "medical_aid_memberships_number_uq",
    "This membership number is already recorded for that scheme",
)];

#[derive(Clone)]
pub struct MembershipsService {
    pool: PgPool,
    bus: EventBus,
}

impl MembershipsService {
    pub fn new(pool: PgPool, bus: EventBus) -> Self {
        Self { pool, bus }
    }

    pub async fn schemes(&self, actor: &TenantUser) -> AppResult<Vec<SchemeResponse>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let schemes = repo::active_schemes(&mut tx).await?;
        let plans = repo::active_plans(&mut tx).await?;
        tx.commit().await?;

        let mut by_scheme: HashMap<Uuid, Vec<PlanResponse>> = HashMap::new();
        for p in plans {
            by_scheme
                .entry(p.scheme_id)
                .or_default()
                .push(PlanResponse { id: p.id, name: p.name, code: p.code });
        }
        Ok(schemes
            .into_iter()
            .map(|s| SchemeResponse {
                plans: by_scheme.remove(&s.id).unwrap_or_default(),
                id: s.id,
                name: s.name,
                code: s.code,
            })
            .collect())
    }

    pub async fn create(&self, actor: &TenantUser, req: CreateMembershipRequest) -> AppResult<MembershipResponse> {
        let new = req.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let id = create_membership(&mut tx, actor.tenant_id, &new, actor.user_id).await?;
        let membership = load(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(membership)
    }

    pub async fn get(&self, actor: &TenantUser, id: Uuid) -> AppResult<MembershipResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let membership = load(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(membership)
    }

    /// Changing the main member re-validates every account using this
    /// membership at commit (dependant code 00 must follow the main member).
    pub async fn update(&self, actor: &TenantUser, id: Uuid, req: UpdateMembershipRequest) -> AppResult<MembershipResponse> {
        let changes = req.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let existing = load(&mut tx, actor.tenant_id, id).await?;

        if let Some(plan_id) = changes.plan_id
            && !repo::plan_is_offered(&mut tx, existing.scheme_id, plan_id).await?
        {
            return Err(AppError::validation("plan_id is not an active plan of this scheme"));
        }
        if let Some(person_id) = changes.main_member_person_id {
            persons::service::ensure_exists(&mut tx, actor.tenant_id, person_id).await?;
        }

        repo::update(&mut tx, actor.tenant_id, id, &changes)
            .await
            .map_err(on_unique(MEMBERSHIP_UNIQUES))?;
        let files = accounts::repository::using_membership(&mut tx, actor.tenant_id, id).await?;
        accounts::service::announce_files(&mut tx, &self.bus, actor.tenant_id, files).await?;
        let membership = load(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(membership)
    }
}

/// Create a membership inside an existing transaction (also used when
/// opening a patient file in one step).
pub async fn create_membership(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    new: &NewMembership,
    created_by: Uuid,
) -> AppResult<Uuid> {
    if !repo::plan_is_offered(conn, new.scheme_id, new.plan_id).await? {
        return Err(AppError::validation("plan_id is not an active plan of scheme_id"));
    }
    persons::service::ensure_exists(conn, tenant_id, new.main_member_person_id).await?;
    repo::insert(conn, tenant_id, new, created_by)
        .await
        .map_err(on_unique(MEMBERSHIP_UNIQUES))
}

pub async fn load(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<MembershipResponse> {
    repo::find(conn, tenant_id, id)
        .await?
        .ok_or(AppError::NotFound("medical aid membership"))
}
