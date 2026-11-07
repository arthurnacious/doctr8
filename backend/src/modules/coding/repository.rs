use sqlx::PgConnection;

use super::dto::{Icd10Code, Modifier, NappiCode, Search, TariffCode};

pub async fn search_icd10(conn: &mut PgConnection, s: &Search) -> Result<Vec<Icd10Code>, sqlx::Error> {
    sqlx::query_as(
        "SELECT code, description, valid_as_primary FROM icd10_codes
          WHERE is_active
            AND ($1::text IS NULL OR code LIKE $1 || '%' OR description ILIKE '%' || $2 || '%')
          ORDER BY (code LIKE coalesce($1, '') || '%') DESC, code
          LIMIT $3",
    )
    .bind(&s.code_prefix)
    .bind(&s.text)
    .bind(s.limit)
    .fetch_all(conn)
    .await
}

pub async fn search_tariffs(conn: &mut PgConnection, s: &Search) -> Result<Vec<TariffCode>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, code_set, code, description FROM tariff_codes
          WHERE is_active AND code_set = $3
            AND ($1::text IS NULL OR code LIKE $1 || '%' OR description ILIKE '%' || $2 || '%')
          ORDER BY (code LIKE coalesce($1, '') || '%') DESC, code
          LIMIT $4",
    )
    .bind(&s.code_prefix)
    .bind(&s.text)
    .bind(&s.code_set)
    .bind(s.limit)
    .fetch_all(conn)
    .await
}

pub async fn search_nappi(conn: &mut PgConnection, s: &Search) -> Result<Vec<NappiCode>, sqlx::Error> {
    sqlx::query_as(
        "SELECT code, description FROM nappi_codes
          WHERE is_active
            AND ($1::text IS NULL OR code LIKE $1 || '%' OR description ILIKE '%' || $2 || '%')
          ORDER BY code LIMIT $3",
    )
    .bind(&s.code_prefix)
    .bind(&s.text)
    .bind(s.limit)
    .fetch_all(conn)
    .await
}

pub async fn list_modifiers(conn: &mut PgConnection, s: &Search) -> Result<Vec<Modifier>, sqlx::Error> {
    sqlx::query_as(
        "SELECT code, description FROM modifiers
          WHERE is_active
            AND ($1::text IS NULL OR code LIKE $1 || '%' OR description ILIKE '%' || $2 || '%')
          ORDER BY code LIMIT $3",
    )
    .bind(&s.code_prefix)
    .bind(&s.text)
    .bind(s.limit)
    .fetch_all(conn)
    .await
}

// --- lookups used when building invoice lines ---------------------------------

pub async fn find_tariff(conn: &mut PgConnection, code_set: &str, code: &str) -> Result<Option<TariffCode>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, code_set, code, description FROM tariff_codes
          WHERE code_set = $1 AND code = $2 AND is_active",
    )
    .bind(code_set)
    .bind(code)
    .fetch_optional(conn)
    .await
}

pub async fn find_nappi(conn: &mut PgConnection, code: &str) -> Result<Option<NappiCode>, sqlx::Error> {
    sqlx::query_as("SELECT code, description FROM nappi_codes WHERE code = $1 AND is_active")
        .bind(code)
        .fetch_optional(conn)
        .await
}

/// Of `codes`, those that aren't active ICD-10 codes.
pub async fn unknown_icd10(conn: &mut PgConnection, codes: &[String]) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT c FROM unnest($1::text[]) AS c
          WHERE NOT EXISTS (SELECT 1 FROM icd10_codes i WHERE i.code = c AND i.is_active)",
    )
    .bind(codes)
    .fetch_all(conn)
    .await
}

/// Of `codes`, those that may not be the primary (first) diagnosis.
pub async fn not_primary_icd10(conn: &mut PgConnection, codes: &[String]) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT code FROM icd10_codes WHERE code = ANY($1) AND NOT valid_as_primary")
        .bind(codes)
        .fetch_all(conn)
        .await
}

pub async fn unknown_modifiers(conn: &mut PgConnection, codes: &[String]) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT c FROM unnest($1::text[]) AS c
          WHERE NOT EXISTS (SELECT 1 FROM modifiers m WHERE m.code = c AND m.is_active)",
    )
    .bind(codes)
    .fetch_all(conn)
    .await
}
