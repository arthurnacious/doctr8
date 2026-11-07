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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::body::to_bytes;

    use super::*;
    use crate::common::test_support::db_error;

    async fn render(err: AppError) -> (StatusCode, serde_json::Value) {
        let response = err.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn every_variant_has_status_code_and_message() {
        let cases = [
            (AppError::validation("bad"), StatusCode::UNPROCESSABLE_ENTITY, "validation_failed", "bad"),
            (AppError::Unauthorized, StatusCode::UNAUTHORIZED, "unauthorized", "Authentication required"),
            (AppError::InvalidCredentials, StatusCode::UNAUTHORIZED, "invalid_credentials", "Invalid email or password"),
            (AppError::forbidden(), StatusCode::FORBIDDEN, "forbidden", "You do not have permission to do this"),
            (AppError::NotFound("role"), StatusCode::NOT_FOUND, "not_found", "role not found"),
            (AppError::Conflict("taken".into()), StatusCode::CONFLICT, "conflict", "taken"),
            (AppError::Internal(anyhow::anyhow!("x")), StatusCode::INTERNAL_SERVER_ERROR, "internal", "Something went wrong"),
        ];
        for (err, status, code, message) in cases {
            let (s, body) = render(err).await;
            assert_eq!(s, status);
            assert_eq!(body["error"]["code"], code);
            assert_eq!(body["error"]["message"], message);
        }
    }

    #[tokio::test]
    async fn internal_errors_never_leak_details_to_clients() {
        let (_, body) = render(AppError::Internal(anyhow::anyhow!("password=hunter2 at db.internal:5432"))).await;
        let text = body.to_string();
        assert!(!text.contains("hunter2") && !text.contains("db.internal"));
    }

    #[test]
    fn unique_violation_is_conflict() {
        assert!(matches!(AppError::from(db_error("23505", Some("x_uq"), "dup")), AppError::Conflict(_)));
    }

    #[test]
    fn foreign_key_violation_is_conflict() {
        assert!(matches!(AppError::from(db_error("23503", None, "fk")), AppError::Conflict(_)));
    }

    #[test]
    fn rls_violation_is_forbidden() {
        assert!(matches!(AppError::from(db_error("42501", None, "rls")), AppError::Forbidden(_)));
    }

    #[test]
    fn check_violation_is_validation_naming_only_the_constraint() {
        let err = AppError::from(db_error("23514", Some("persons_sa_id_check"), "Failing row contains (Thandi, 900215...)"));
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("persons_sa_id_check"));
                assert!(!msg.contains("Thandi"), "row values must not reach the client");
            }
            other => panic!("expected validation, got {other:?}"),
        }
    }

    #[test]
    fn file_mode_trigger_errors_pass_their_message_through() {
        let err = AppError::from(db_error("D8001", None, "Accounts opened in patient_per_file mode must have exactly one patient"));
        match err {
            AppError::Conflict(msg) => assert!(msg.contains("exactly one patient")),
            other => panic!("expected conflict, got {other:?}"),
        }
    }

    #[test]
    fn other_database_errors_are_internal() {
        assert!(matches!(AppError::from(db_error("08006", None, "connection failure")), AppError::Internal(_)));
        assert!(matches!(AppError::from(sqlx::Error::PoolTimedOut), AppError::Internal(_)));
    }

    #[test]
    fn on_unique_maps_known_constraints_only() {
        const MAP: &[(&str, &str)] = &[("tenants_slug_key", "That practice address is already taken")];
        let mapped = on_unique(MAP)(db_error("23505", Some("tenants_slug_key"), "dup"));
        assert!(matches!(&mapped, AppError::Conflict(m) if m == "That practice address is already taken"));

        let other = on_unique(MAP)(db_error("23505", Some("something_else"), "dup"));
        assert!(matches!(&other, AppError::Conflict(m) if m == "A record with these details already exists"));

        // Non-unique errors fall through to the normal mapping.
        assert!(matches!(on_unique(MAP)(db_error("42501", None, "rls")), AppError::Forbidden(_)));
    }

    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Captured {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn captured_log(f: impl FnOnce()) -> String {
        let buf = Captured::default();
        let writer = buf.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || writer.clone())
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, f);
        String::from_utf8(buf.0.lock().unwrap().clone()).unwrap()
    }

    #[test]
    fn database_errors_are_logged_without_row_values() {
        let log = captured_log(|| {
            let err = AppError::Internal(
                db_error("22P02", Some("persons_x"), "invalid input syntax for type date: \"Thandi Nkosi 9002150123088\"").into(),
            );
            let _ = err.into_response();
        });
        assert!(log.contains("22P02") && log.contains("persons_x") && log.contains("persons"), "{log}");
        assert!(!log.contains("Thandi") && !log.contains("9002150123088"), "PII leaked into log: {log}");
    }

    #[test]
    fn non_database_internal_errors_are_logged() {
        let log = captured_log(|| {
            let _ = AppError::Internal(anyhow::anyhow!("argon2 exploded")).into_response();
        });
        assert!(log.contains("argon2 exploded"));
    }

    #[test]
    fn client_errors_are_not_logged_as_errors() {
        let log = captured_log(|| {
            let _ = AppError::validation("first_name is required").into_response();
            let _ = AppError::Unauthorized.into_response();
        });
        assert!(log.is_empty(), "{log}");
    }
}
