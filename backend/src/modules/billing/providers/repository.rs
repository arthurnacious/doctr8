use sqlx::PgConnection;
use uuid::Uuid;

use super::dto::{NewProvider, NewServiceCentre, ProviderResponse, ServiceCentreResponse};

const PROVIDER_COLUMNS: &str = "id, kind, title, first_name, last_name, practice_number, hpcsa_number,
    speciality, user_id, is_active, created_at, updated_at";

pub async fn list(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    kind: Option<&str>,
    include_inactive: bool,
) -> Result<Vec<ProviderResponse>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT {PROVIDER_COLUMNS} FROM providers
          WHERE tenant_id = $1 AND ($2::text IS NULL OR kind = $2) AND ($3 OR is_active)
          ORDER BY kind, lower(last_name), lower(coalesce(first_name, ''))"
    ))
    .bind(tenant_id)
    .bind(kind)
    .bind(include_inactive)
    .fetch_all(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<ProviderResponse>, sqlx::Error> {
    sqlx::query_as(&format!("SELECT {PROVIDER_COLUMNS} FROM providers WHERE tenant_id = $1 AND id = $2"))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn insert(conn: &mut PgConnection, tenant_id: Uuid, p: &NewProvider, by: Uuid) -> Result<ProviderResponse, sqlx::Error> {
    sqlx::query_as(&format!(
        "INSERT INTO providers (tenant_id, kind, title, first_name, last_name, practice_number, hpcsa_number,
                                speciality, user_id, is_active, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING {PROVIDER_COLUMNS}"
    ))
    .bind(tenant_id)
    .bind(p.kind.as_str())
    .bind(&p.title)
    .bind(&p.first_name)
    .bind(&p.last_name)
    .bind(&p.practice_number)
    .bind(&p.hpcsa_number)
    .bind(&p.speciality)
    .bind(p.user_id)
    .bind(p.is_active)
    .bind(by)
    .fetch_one(conn)
    .await
}

pub async fn replace(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid, p: &NewProvider) -> Result<Option<ProviderResponse>, sqlx::Error> {
    sqlx::query_as(&format!(
        "UPDATE providers
            SET kind = $3, title = $4, first_name = $5, last_name = $6, practice_number = $7,
                hpcsa_number = $8, speciality = $9, user_id = $10, is_active = $11
          WHERE tenant_id = $1 AND id = $2
      RETURNING {PROVIDER_COLUMNS}"
    ))
    .bind(tenant_id)
    .bind(id)
    .bind(p.kind.as_str())
    .bind(&p.title)
    .bind(&p.first_name)
    .bind(&p.last_name)
    .bind(&p.practice_number)
    .bind(&p.hpcsa_number)
    .bind(&p.speciality)
    .bind(p.user_id)
    .bind(p.is_active)
    .fetch_optional(conn)
    .await
}

/// Is `user_id` an active member of the practice?
pub async fn is_active_member(conn: &mut PgConnection, tenant_id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM memberships WHERE tenant_id = $1 AND user_id = $2 AND status = 'active')",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_one(conn)
    .await
}

// --- service centres -------------------------------------------------------------

const CENTRE_COLUMNS: &str = "id, kind, name, practice_number, default_place_of_service, is_active";

pub async fn list_centres(conn: &mut PgConnection, tenant_id: Uuid, include_inactive: bool) -> Result<Vec<ServiceCentreResponse>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT {CENTRE_COLUMNS} FROM service_centres
          WHERE tenant_id = $1 AND ($2 OR is_active)
          ORDER BY kind = 'rooms' DESC, lower(name)"
    ))
    .bind(tenant_id)
    .bind(include_inactive)
    .fetch_all(conn)
    .await
}

pub async fn find_centre(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<ServiceCentreResponse>, sqlx::Error> {
    sqlx::query_as(&format!("SELECT {CENTRE_COLUMNS} FROM service_centres WHERE tenant_id = $1 AND id = $2"))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn insert_centre(conn: &mut PgConnection, tenant_id: Uuid, c: &NewServiceCentre) -> Result<ServiceCentreResponse, sqlx::Error> {
    sqlx::query_as(&format!(
        "INSERT INTO service_centres (tenant_id, kind, name, practice_number, default_place_of_service, is_active)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING {CENTRE_COLUMNS}"
    ))
    .bind(tenant_id)
    .bind(c.kind.as_str())
    .bind(&c.name)
    .bind(&c.practice_number)
    .bind(&c.default_place_of_service)
    .bind(c.is_active)
    .fetch_one(conn)
    .await
}

pub async fn replace_centre(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid, c: &NewServiceCentre) -> Result<Option<ServiceCentreResponse>, sqlx::Error> {
    sqlx::query_as(&format!(
        "UPDATE service_centres
            SET kind = $3, name = $4, practice_number = $5, default_place_of_service = $6, is_active = $7
          WHERE tenant_id = $1 AND id = $2
      RETURNING {CENTRE_COLUMNS}"
    ))
    .bind(tenant_id)
    .bind(id)
    .bind(c.kind.as_str())
    .bind(&c.name)
    .bind(&c.practice_number)
    .bind(&c.default_place_of_service)
    .bind(c.is_active)
    .fetch_optional(conn)
    .await
}
