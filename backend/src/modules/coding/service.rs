use sqlx::PgPool;

use super::{
    dto::{Icd10Code, Modifier, NappiCode, SearchQuery, TariffCode},
    repository as repo,
};
use crate::common::{auth::TenantUser, db, error::AppResult};

#[derive(Clone)]
pub struct CodingService {
    pool: PgPool,
}

impl CodingService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn icd10(&self, actor: &TenantUser, q: SearchQuery) -> AppResult<Vec<Icd10Code>> {
        let s = q.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let rows = repo::search_icd10(&mut tx, &s).await?;
        tx.commit().await?;
        Ok(rows)
    }

    pub async fn tariffs(&self, actor: &TenantUser, q: SearchQuery) -> AppResult<Vec<TariffCode>> {
        let s = q.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let rows = repo::search_tariffs(&mut tx, &s).await?;
        tx.commit().await?;
        Ok(rows)
    }

    pub async fn nappi(&self, actor: &TenantUser, q: SearchQuery) -> AppResult<Vec<NappiCode>> {
        let s = q.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let rows = repo::search_nappi(&mut tx, &s).await?;
        tx.commit().await?;
        Ok(rows)
    }

    pub async fn modifiers(&self, actor: &TenantUser, q: SearchQuery) -> AppResult<Vec<Modifier>> {
        let s = q.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let rows = repo::list_modifiers(&mut tx, &s).await?;
        tx.commit().await?;
        Ok(rows)
    }
}
