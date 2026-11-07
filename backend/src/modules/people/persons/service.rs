use chrono::Utc;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    dto::{NewPerson, PersonFile, PersonInput, PersonResponse, PersonSummary, SearchPersonsRequest},
    repository as repo,
};
use crate::{
    common::{
        auth::TenantUser,
        db,
        error::{AppError, AppResult, on_unique},
        ws::EventBus,
    },
    modules::people::accounts,
};

const PERSON_UNIQUES: &[(&str, &str)] = &[(
    "persons_identity_uq",
    "A person with this identity document already exists in this practice",
)];

#[derive(Clone)]
pub struct PersonsService {
    pool: PgPool,
    bus: EventBus,
}

impl PersonsService {
    pub fn new(pool: PgPool, bus: EventBus) -> Self {
        Self { pool, bus }
    }

    pub async fn create(&self, actor: &TenantUser, input: PersonInput) -> AppResult<PersonResponse> {
        let new = input.validate(Utc::now().date_naive())?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let person = insert_person(&mut tx, actor.tenant_id, &new, actor.user_id).await?;
        tx.commit().await?;
        Ok(person)
    }

    pub async fn get(&self, actor: &TenantUser, id: Uuid) -> AppResult<PersonResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let person = repo::find(&mut tx, actor.tenant_id, id).await?.ok_or(AppError::NotFound("person"))?;
        tx.commit().await?;
        Ok(person)
    }

    pub async fn replace(&self, actor: &TenantUser, id: Uuid, input: PersonInput) -> AppResult<PersonResponse> {
        let new = input.validate(Utc::now().date_naive())?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let person = repo::replace(&mut tx, actor.tenant_id, id, &new)
            .await
            .map_err(on_unique(PERSON_UNIQUES))?
            .ok_or(AppError::NotFound("person"))?;
        let files = accounts::repository::involving_person(&mut tx, actor.tenant_id, id).await?;
        accounts::service::announce_files(&mut tx, &self.bus, actor.tenant_id, files).await?;
        tx.commit().await?;
        Ok(person)
    }

    pub async fn search(&self, actor: &TenantUser, req: SearchPersonsRequest) -> AppResult<Vec<PersonSummary>> {
        let terms = req.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let found = repo::search(&mut tx, actor.tenant_id, &terms).await?;
        tx.commit().await?;
        Ok(found)
    }

    pub async fn files(&self, actor: &TenantUser, id: Uuid) -> AppResult<Vec<PersonFile>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        if !repo::exists(&mut tx, actor.tenant_id, id).await? {
            return Err(AppError::NotFound("person"));
        }
        let files = repo::files(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(files)
    }
}

/// Insert a person inside an existing transaction (also used when opening a
/// patient file in one step).
pub async fn insert_person(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    new: &NewPerson,
    created_by: Uuid,
) -> AppResult<PersonResponse> {
    repo::insert(conn, tenant_id, new, created_by)
        .await
        .map_err(on_unique(PERSON_UNIQUES))
}

/// 404 unless the person exists in this practice.
pub async fn ensure_exists(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<()> {
    if repo::exists(conn, tenant_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("person"))
    }
}
