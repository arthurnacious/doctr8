//! Route-level guard matrix: every endpoint, through the real router, with
//! a database that can't be reached. Guards run before any handler touches
//! the database, so 401/403 decisions are fully testable here.
//!
//! For each permission-guarded route we check:
//!   * no token                         → 401
//!   * valid token, no practice chosen  → 403
//!   * every permission EXCEPT the one  → 403
//!   * ONLY the required permission     → passes the guard (anything but 401/403)

use std::time::Duration;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

use crate::{
    common::{
        permission::Permission,
        test_support::{TENANT, config, token},
    },
    modules,
    state::AppState,
};

#[derive(Clone, Copy, Debug)]
enum Guard {
    /// Any valid access token.
    Authenticated,
    /// Valid token with a practice selected.
    InPractice,
    Requires(Permission),
}

const ID: &str = "00000000-0000-0000-0000-000000000042";

fn routes() -> Vec<(Method, String, Guard)> {
    use Guard::*;
    use Permission as P;
    let r = |m: Method, p: &str, g: Guard| (m, p.replace("{id}", ID), g);
    vec![
        r(Method::GET, "/users/me", Authenticated),
        r(Method::GET, "/tenants", Authenticated),
        r(Method::POST, "/tenants", Authenticated),
        r(Method::GET, "/tenants/current", Requires(P::PracticeView)),
        r(Method::PATCH, "/tenants/current", Requires(P::PracticeManage)),
        r(Method::GET, "/members", Requires(P::MembersView)),
        r(Method::PUT, "/members/{id}/roles", Requires(P::RolesAssign)),
        r(Method::GET, "/permissions", Requires(P::RolesView)),
        r(Method::GET, "/roles", Requires(P::RolesView)),
        r(Method::POST, "/roles", Requires(P::RolesManage)),
        r(Method::GET, "/roles/{id}", Requires(P::RolesView)),
        r(Method::PATCH, "/roles/{id}", Requires(P::RolesManage)),
        r(Method::DELETE, "/roles/{id}", Requires(P::RolesManage)),
        r(Method::PUT, "/roles/{id}/permissions", Requires(P::RolesManage)),
        r(Method::POST, "/persons", Requires(P::PatientsManage)),
        r(Method::POST, "/persons/search", Requires(P::PatientsView)),
        r(Method::GET, "/persons/{id}", Requires(P::PatientsView)),
        r(Method::PUT, "/persons/{id}", Requires(P::PatientsManage)),
        r(Method::GET, "/persons/{id}/files", Requires(P::PatientsView)),
        r(Method::GET, "/medical-schemes", InPractice),
        r(Method::POST, "/medical-aid-memberships", Requires(P::AccountsManage)),
        r(Method::GET, "/medical-aid-memberships/{id}", Requires(P::AccountsView)),
        r(Method::PATCH, "/medical-aid-memberships/{id}", Requires(P::AccountsManage)),
        r(Method::GET, "/accounts/{id}", Requires(P::AccountsView)),
        r(Method::PATCH, "/accounts/{id}", Requires(P::AccountsManage)),
        r(Method::POST, "/accounts/{id}/patients", Requires(P::PatientsManage)),
        r(Method::POST, "/patient-files", Requires(P::PatientsManage)),
        r(Method::GET, "/patients/{id}", Requires(P::PatientsView)),
        r(Method::PATCH, "/patients/{id}", Requires(P::PatientsManage)),
        r(Method::POST, "/patients/search", Requires(P::PatientsView)),
        r(Method::GET, "/patient-files/{id}", Requires(P::PatientsView)),
        r(Method::GET, "/patient-files/settings", Requires(P::PatientsView)),
        r(Method::PUT, "/patients/{id}/status", Requires(P::PatientsManage)),
        // coding reference
        r(Method::GET, "/coding/icd10", InPractice),
        r(Method::GET, "/coding/tariffs", InPractice),
        r(Method::GET, "/coding/nappi", InPractice),
        r(Method::GET, "/coding/modifiers", InPractice),
        // billing: providers
        r(Method::GET, "/providers", Requires(P::BillingView)),
        r(Method::POST, "/providers", Requires(P::ProvidersManage)),
        r(Method::GET, "/providers/{id}", Requires(P::BillingView)),
        r(Method::PUT, "/providers/{id}", Requires(P::ProvidersManage)),
        r(Method::GET, "/service-centres", Requires(P::BillingView)),
        r(Method::POST, "/service-centres", Requires(P::ProvidersManage)),
        r(Method::PUT, "/service-centres/{id}", Requires(P::ProvidersManage)),
        // billing: invoices and claims
        r(Method::POST, "/invoices", Requires(P::BillingManage)),
        r(Method::POST, "/invoices/search", Requires(P::BillingView)),
        r(Method::POST, "/invoice-drafts", Requires(P::BillingManage)),
        r(Method::GET, "/invoice-drafts/{id}", Requires(P::BillingView)),
        r(Method::PUT, "/invoice-drafts/{id}", Requires(P::BillingManage)),
        r(Method::DELETE, "/invoice-drafts/{id}", Requires(P::BillingManage)),
        r(Method::POST, "/invoice-drafts/{id}/finalise", Requires(P::BillingManage)),
        r(Method::GET, "/accounts/{id}/invoice-drafts", Requires(P::BillingView)),
        r(Method::GET, "/invoices/{id}", Requires(P::BillingView)),
        r(Method::GET, "/accounts/{id}/invoices", Requires(P::BillingView)),
        r(Method::POST, "/invoices/{id}/credit-notes", Requires(P::BillingManage)),
        r(Method::POST, "/invoices/{id}/claims", Requires(P::BillingManage)),
        r(Method::GET, "/claims", Requires(P::BillingView)),
        r(Method::POST, "/claims/{id}/status", Requires(P::BillingManage)),
        // billing: money
        r(Method::GET, "/accounts/{id}/transactions", Requires(P::BillingView)),
        r(Method::GET, "/accounts/{id}/balance", Requires(P::BillingView)),
        r(Method::POST, "/payments", Requires(P::PaymentsManage)),
        r(Method::POST, "/refunds", Requires(P::PaymentsManage)),
        r(Method::POST, "/journals", Requires(P::JournalsManage)),
        r(Method::POST, "/liability-transfers", Requires(P::JournalsManage)),
        r(Method::GET, "/transactions/{id}", Requires(P::BillingView)),
        r(Method::POST, "/transactions/{id}/reverse", Requires(P::JournalsManage)),
        r(Method::GET, "/journal-reason-types", Requires(P::BillingView)),
        r(Method::POST, "/journal-reason-types", Requires(P::JournalsManage)),
        r(Method::PUT, "/journal-reason-types/{id}", Requires(P::JournalsManage)),
        r(Method::GET, "/remittances", Requires(P::BillingView)),
        r(Method::POST, "/remittances", Requires(P::PaymentsManage)),
        r(Method::GET, "/remittances/{id}", Requires(P::BillingView)),
        r(Method::POST, "/remittances/{id}/allocations", Requires(P::PaymentsManage)),
        // billing: statements
        r(Method::GET, "/accounts/{id}/statement", Requires(P::BillingView)),
    ]
}

fn app() -> Router {
    // Port 1 refuses connections; fail fast if a handler reaches the DB.
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(250))
        .connect_lazy(&config().database_url)
        .unwrap();
    modules::router().with_state(AppState::new(pool, config()))
}

async fn send(method: &Method, path: &str, bearer: Option<String>) -> (StatusCode, serde_json::Value) {
    let mut req = Request::builder().method(method.clone()).uri(path);
    if let Some(t) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let body = if matches!(*method, Method::GET | Method::DELETE) {
        Body::empty()
    } else {
        req = req.header(header::CONTENT_TYPE, "application/json");
        Body::from("{}")
    };
    let response = app().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

fn all_except(p: Permission) -> Vec<Permission> {
    Permission::ALL.iter().copied().filter(|x| *x != p).collect()
}

fn passed_guard(status: StatusCode) -> bool {
    status != StatusCode::UNAUTHORIZED && status != StatusCode::FORBIDDEN
}

#[tokio::test]
async fn every_protected_route_requires_a_token() {
    for (m, p, _) in routes() {
        let (status, body) = send(&m, &p, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{m} {p}");
        assert_eq!(body["error"]["code"], "unauthorized", "{m} {p}");
    }
}

#[tokio::test]
async fn invalid_tokens_are_rejected_everywhere() {
    for (m, p, _) in routes() {
        let (status, _) = send(&m, &p, Some("not.a.jwt".into())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{m} {p}");
    }
}

#[tokio::test]
async fn practice_routes_need_a_selected_practice() {
    for (m, p, guard) in routes() {
        if matches!(guard, Guard::Authenticated) {
            continue;
        }
        let (status, _) = send(&m, &p, Some(token(None, Permission::ALL))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{m} {p}");
    }
}

#[tokio::test]
async fn each_route_demands_exactly_its_permission() {
    for (m, p, guard) in routes() {
        let Guard::Requires(perm) = guard else { continue };

        let (status, body) = send(&m, &p, Some(token(Some(TENANT), &all_except(perm)))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{m} {p} must require {}", perm.key());
        assert_eq!(body["error"]["code"], "forbidden");

        let (status, _) = send(&m, &p, Some(token(Some(TENANT), &[perm]))).await;
        assert!(passed_guard(status), "{m} {p} should accept {} alone, got {status}", perm.key());
    }
}

#[tokio::test]
async fn authenticated_and_in_practice_routes_need_no_permissions() {
    for (m, p, guard) in routes() {
        let tenant = match guard {
            Guard::Authenticated => None,
            Guard::InPractice => Some(TENANT),
            Guard::Requires(_) => continue,
        };
        let (status, _) = send(&m, &p, Some(token(tenant, &[]))).await;
        assert!(passed_guard(status), "{m} {p} got {status}");
    }
}

#[tokio::test]
async fn permission_catalogue_is_served_without_the_database() {
    let (status, body) = send(&Method::GET, "/permissions", Some(token(Some(TENANT), &[Permission::RolesView]))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), Permission::ALL.len());
}

#[tokio::test]
async fn database_outage_is_a_generic_500() {
    let (status, body) = send(&Method::GET, "/roles", Some(token(Some(TENANT), &[Permission::RolesView]))).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"]["message"], "Something went wrong");
    assert!(!body.to_string().contains("127.0.0.1"), "no connection details leak");
}

#[tokio::test]
async fn public_routes() {
    let (status, _) = send(&Method::GET, "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    // Refresh, switch and logout authenticate with the refresh cookie, not a bearer token.
    for path in ["/auth/refresh", "/auth/switch-practice"] {
        let (status, _) = send(&Method::POST, path, None).await;
        assert!(status == StatusCode::UNAUTHORIZED || status == StatusCode::UNPROCESSABLE_ENTITY, "{path}: {status}");
    }
    let (status, _) = send(&Method::POST, "/auth/logout", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "logout without a cookie is a no-op");
}

#[tokio::test]
async fn malformed_bodies_and_ids_are_client_errors() {
    let t = Some(token(Some(TENANT), Permission::ALL));
    let (status, _) = send(&Method::GET, "/roles/not-a-uuid", t.clone()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // `{}` is missing required fields.
    let (status, _) = send(&Method::POST, "/patient-files", t).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn posted_money_has_no_edit_or_delete_endpoints() {
    let t = Some(token(Some(TENANT), Permission::ALL));
    for (m, p) in [
        (Method::DELETE, "/invoices/{id}"),
        (Method::PATCH, "/invoices/{id}"),
        (Method::PUT, "/invoices/{id}"),
        (Method::DELETE, "/transactions/{id}"),
        (Method::PATCH, "/transactions/{id}"),
        (Method::DELETE, "/remittances/{id}"),
    ] {
        let (status, _) = send(&m, &p.replace("{id}", ID), t.clone()).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{m} {p}");
    }
}

#[tokio::test]
async fn statement_date_must_parse() {
    let t = Some(token(Some(TENANT), &[Permission::BillingView]));
    let (status, _) = send(&Method::GET, &format!("/accounts/{ID}/statement?as_of=yesterday"), t).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unknown_routes_404() {
    let (status, _) = send(&Method::GET, "/patients", Some(token(Some(TENANT), Permission::ALL))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no bulk patient listing endpoint exists");
    let (status, _) = send(&Method::DELETE, &format!("/persons/{}", Uuid::nil()), Some(token(Some(TENANT), Permission::ALL))).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "persons are never deleted via the API");
}
