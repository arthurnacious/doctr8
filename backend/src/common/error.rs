use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

pub type AppResult<T> = Result<T, AppError>;

/// The one error type every handler and service returns.
/// Renders as `{"error": {"code": "...", "message": "..."}}`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("Authentication required")]
    Unauthorized,
    #[error("Invalid email or password")]
    InvalidCredentials,
    #[error("{0}")]
    Forbidden(&'static str),
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Conflict(String),
    #[error("Something went wrong")]
    Internal(anyhow::Error),
}

impl AppError {
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn forbidden() -> Self {
        Self::Forbidden("You do not have permission to do this")
    }

    fn parts(&self) -> (StatusCode, &'static str) {
        match self {
            Self::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "validation_failed"),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::InvalidCredentials => (StatusCode::UNAUTHORIZED, "invalid_credentials"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.parts();
        if let Self::Internal(err) = &self {
            log_internal(err);
        }
        let body = json!({ "error": { "code": code, "message": self.to_string() } });
        (status, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

/// Log an internal error WITHOUT personal information.
///
/// Postgres error messages and DETAIL lines can echo row values ("Failing row
/// contains (...)", "invalid input syntax for type date: \"...\""), so for
/// database errors we only log the SQLSTATE, constraint and table.
fn log_internal(err: &anyhow::Error) {
    match err.downcast_ref::<sqlx::Error>() {
        Some(sqlx::Error::Database(db)) => tracing::error!(
            sqlstate = db.code().as_deref().unwrap_or("?"),
            constraint = db.constraint().unwrap_or("-"),
            table = db.table().unwrap_or("-"),
            "database error"
        ),
        Some(other) => tracing::error!(kind = sqlx_error_kind(other), "database error"),
        None => tracing::error!(error = %err, "internal error"),
    }
}

fn sqlx_error_kind(err: &sqlx::Error) -> &'static str {
    match err {
        sqlx::Error::Configuration(_) => "configuration",
        sqlx::Error::Io(_) => "io",
        sqlx::Error::Tls(_) => "tls",
        sqlx::Error::Protocol(_) => "protocol",
        sqlx::Error::RowNotFound => "row_not_found",
        sqlx::Error::ColumnNotFound(_) => "column_not_found",
        sqlx::Error::ColumnDecode { .. } => "column_decode",
        sqlx::Error::Decode(_) => "decode",
        sqlx::Error::PoolTimedOut => "pool_timed_out",
        sqlx::Error::PoolClosed => "pool_closed",
        _ => "other",
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &err {
            match db.code().as_deref() {
                // Our own file-mode rule violations from triggers (D8xxx).
                // Messages are written by us and contain no personal data.
                Some(code) if code.starts_with("D8") => {
                    return Self::Conflict(db.message().to_owned());
                }
                // check_violation — the service should have caught it first.
                Some("23514") => {
                    return Self::Validation(format!(
                        "A value is not allowed ({})",
                        db.constraint().unwrap_or("check")
                    ));
                }
                // unique_violation
                Some("23505") => {
                    return Self::Conflict("A record with these details already exists".into());
                }
                // foreign_key_violation
                Some("23503") => {
                    return Self::Conflict(
                        "This change conflicts with related records".into(),
                    );
                }
                // insufficient_privilege — includes RLS WITH CHECK failures
                Some("42501") => return Self::forbidden(),
                _ => {}
            }
        }
        Self::Internal(err.into())
    }
}

/// Map specific unique constraints to friendly messages:
/// `.map_err(on_unique(&[("tenants_slug_key", "That address is taken")]))`
pub fn on_unique(
    mappings: &'static [(&'static str, &'static str)],
) -> impl Fn(sqlx::Error) -> AppError {
    move |err| {
        if let sqlx::Error::Database(db) = &err
            && db.code().as_deref() == Some("23505")
            && let Some(constraint) = db.constraint()
            && let Some((_, msg)) = mappings.iter().find(|(name, _)| *name == constraint)
        {
            return AppError::Conflict((*msg).to_owned());
        }
        err.into()
    }
}
