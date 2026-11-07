//! End-to-end WebSocket behaviour without a database: a real server on a
//! local port, a real WebSocket client, and events injected straight into
//! the bus (what the Postgres listener does after a COMMIT).

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest, protocol::frame::coding::CloseCode},
};
use uuid::Uuid;

use crate::{
    common::{
        jwt::JwtKeys,
        permission::Permission,
        test_support::{ISSUER, SECRET, SESSION, TENANT, USER, config, token},
        ws::{Event, EventBus, EventKind},
    },
    modules,
    state::AppState,
};

type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;

const ACCOUNT: Uuid = Uuid::from_u128(0xACC0);

async fn server() -> (String, EventBus) {
    let bus = EventBus::new();
    let pool = PgPoolOptions::new().connect_lazy(&config().database_url).unwrap();
    let app = modules::router().with_state(AppState::with_events(pool, config(), bus.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("ws://{addr}/ws"), bus)
}

async fn send(c: &mut Client, v: serde_json::Value) {
    c.send(Message::Text(v.to_string().into())).await.unwrap();
}

/// Next JSON message, or None if nothing arrives within `wait`.
async fn next(c: &mut Client, wait: Duration) -> Option<serde_json::Value> {
    match tokio::time::timeout(wait, c.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => Some(serde_json::from_str(&t).unwrap()),
        Ok(Some(Ok(Message::Close(frame)))) => Some(serde_json::json!({
            "closed": frame.map(|f| u16::from(f.code)).unwrap_or(0)
        })),
        _ => None,
    }
}

async fn expect(c: &mut Client) -> serde_json::Value {
    next(c, Duration::from_secs(2)).await.expect("expected a message")
}

async fn nothing(c: &mut Client) {
    let got = next(c, Duration::from_millis(250)).await;
    assert!(got.is_none(), "expected silence, got {got:?}");
}

async fn connect_and_auth(url: &str, bearer: String) -> Client {
    let (mut c, _) = connect_async(url).await.unwrap();
    send(&mut c, serde_json::json!({"type": "auth", "token": bearer})).await;
    let ready = expect(&mut c).await;
    assert_eq!(ready["type"], "ready", "{ready}");
    assert_eq!(ready["tenant_id"], TENANT.to_string());
    c
}

/// Wait until the server has registered the socket's tenant subscription.
async fn settle(bus: &EventBus, tenants: usize) {
    for _ in 0..50 {
        if bus.active_tenants() >= tenants {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn claim_event(tenant: Uuid) -> Event {
    Event::new(tenant, EventKind::ClaimStatusChanged, Uuid::from_u128(1)).account(ACCOUNT).status("accepted")
}

#[tokio::test]
async fn tenant_wide_events_reach_permitted_clients() {
    let (url, bus) = server().await;
    let mut c = connect_and_auth(&url, token(Some(TENANT), &[Permission::BillingView])).await;
    settle(&bus, 1).await;
    bus.dispatch(claim_event(TENANT));
    let e = expect(&mut c).await;
    assert_eq!(e["type"], "event");
    assert_eq!(e["event"], "claim.status_changed");
    assert_eq!(e["status"], "accepted");
    assert_eq!(e["account_id"], ACCOUNT.to_string());
    assert!(e.get("tenant_id").is_none());
}

#[tokio::test]
async fn clients_without_permission_get_nothing() {
    let (url, bus) = server().await;
    let mut c = connect_and_auth(&url, token(Some(TENANT), &[Permission::PatientsView])).await;
    settle(&bus, 1).await;
    bus.dispatch(claim_event(TENANT));
    nothing(&mut c).await;
}

#[tokio::test]
async fn other_practices_events_are_never_delivered() {
    let (url, bus) = server().await;
    let mut c = connect_and_auth(&url, token(Some(TENANT), Permission::ALL)).await;
    // A second practice with its own subscriber.
    let other = Uuid::from_u128(0x07E2);
    let mut o = {
        let (mut c, _) = connect_async(&url).await.unwrap();
        send(&mut c, serde_json::json!({"type": "auth", "token": token(Some(other), Permission::ALL)})).await;
        assert_eq!(expect(&mut c).await["type"], "ready");
        c
    };
    settle(&bus, 2).await;
    bus.dispatch(claim_event(other));
    nothing(&mut c).await;
    assert_eq!(expect(&mut o).await["event"], "claim.status_changed");
}

#[tokio::test]
async fn account_events_need_a_subscription() {
    let (url, bus) = server().await;
    let mut c = connect_and_auth(&url, token(Some(TENANT), &[Permission::BillingView])).await;
    settle(&bus, 1).await;

    bus.dispatch(Event::balance_changed(TENANT, ACCOUNT));
    nothing(&mut c).await;

    send(&mut c, serde_json::json!({"type": "subscribe", "account_id": ACCOUNT})).await;
    assert_eq!(expect(&mut c).await, serde_json::json!({"type": "subscribed", "account_id": ACCOUNT}));
    bus.dispatch(Event::balance_changed(TENANT, Uuid::from_u128(0xFFFF)));
    bus.dispatch(Event::balance_changed(TENANT, ACCOUNT));
    let e = expect(&mut c).await;
    assert_eq!((e["event"].as_str(), e["id"].as_str()), (Some("account.balance_changed"), Some(ACCOUNT.to_string().as_str())));
    nothing(&mut c).await;

    send(&mut c, serde_json::json!({"type": "unsubscribe", "account_id": ACCOUNT})).await;
    assert_eq!(expect(&mut c).await["type"], "unsubscribed");
    bus.dispatch(Event::balance_changed(TENANT, ACCOUNT));
    nothing(&mut c).await;
}

#[tokio::test]
async fn resync_is_forwarded() {
    let (url, bus) = server().await;
    let mut c = connect_and_auth(&url, token(Some(TENANT), &[])).await;
    settle(&bus, 1).await;
    bus.resync_all();
    assert_eq!(expect(&mut c).await, serde_json::json!({"type": "resync"}));
}

#[tokio::test]
async fn ping_pong() {
    let (url, _) = server().await;
    let mut c = connect_and_auth(&url, token(Some(TENANT), &[])).await;
    send(&mut c, serde_json::json!({"type": "ping"})).await;
    assert_eq!(expect(&mut c).await, serde_json::json!({"type": "pong"}));
}

#[tokio::test]
async fn bad_token_closes_with_4001() {
    let (url, _) = server().await;
    let (mut c, _) = connect_async(&url).await.unwrap();
    send(&mut c, serde_json::json!({"type": "auth", "token": "nope"})).await;
    assert_eq!(expect(&mut c).await["closed"], 4001);
}

#[tokio::test]
async fn first_message_must_be_auth() {
    let (url, _) = server().await;
    let (mut c, _) = connect_async(&url).await.unwrap();
    send(&mut c, serde_json::json!({"type": "subscribe", "account_id": ACCOUNT})).await;
    assert_eq!(expect(&mut c).await["closed"], 4001);
}

#[tokio::test]
async fn token_without_practice_closes_with_4003() {
    let (url, _) = server().await;
    let (mut c, _) = connect_async(&url).await.unwrap();
    send(&mut c, serde_json::json!({"type": "auth", "token": token(None, Permission::ALL)})).await;
    assert_eq!(expect(&mut c).await["closed"], 4003);
}

#[tokio::test]
async fn unknown_messages_close_with_4400() {
    let (url, _) = server().await;
    let mut c = connect_and_auth(&url, token(Some(TENANT), &[])).await;
    send(&mut c, serde_json::json!({"type": "shutdown"})).await;
    assert_eq!(expect(&mut c).await["closed"], 4400);
}

#[tokio::test]
async fn bearer_header_authenticates_non_browser_clients() {
    let (url, _) = server().await;
    let mut req = url.clone().into_client_request().unwrap();
    req.headers_mut().insert("authorization", format!("Bearer {}", token(Some(TENANT), &[])).parse().unwrap());
    let (mut c, _) = connect_async(req).await.unwrap();
    assert_eq!(expect(&mut c).await["type"], "ready");

    let mut bad = url.into_client_request().unwrap();
    bad.headers_mut().insert("authorization", "Bearer junk".parse().unwrap());
    assert!(connect_async(bad).await.is_err(), "invalid header token is refused before upgrading");
}

#[tokio::test]
async fn reauthentication_extends_and_updates_permissions() {
    let (url, bus) = server().await;
    let mut c = connect_and_auth(&url, token(Some(TENANT), &[])).await;
    settle(&bus, 1).await;
    bus.dispatch(claim_event(TENANT));
    nothing(&mut c).await;

    send(&mut c, serde_json::json!({"type": "auth", "token": token(Some(TENANT), &[Permission::BillingView])})).await;
    assert_eq!(expect(&mut c).await["type"], "reauthenticated");
    bus.dispatch(claim_event(TENANT));
    assert_eq!(expect(&mut c).await["event"], "claim.status_changed");
}

#[tokio::test]
async fn reauthenticating_into_another_practice_closes_with_4009() {
    let (url, _) = server().await;
    let mut c = connect_and_auth(&url, token(Some(TENANT), &[])).await;
    send(&mut c, serde_json::json!({"type": "auth", "token": token(Some(Uuid::from_u128(9)), &[])})).await;
    assert_eq!(expect(&mut c).await["closed"], 4009);
}

#[tokio::test]
async fn expired_tokens_end_the_connection() {
    let (url, _) = server().await;
    // Valid for one second (within verification leeway), then the server closes.
    let short = JwtKeys::new(SECRET.as_bytes(), ISSUER, 1).issue(USER, SESSION, Some(TENANT), &[]).unwrap().token;
    let mut c = connect_and_auth(&url, short).await;
    let closed = next(&mut c, Duration::from_secs(4)).await.expect("server should close");
    assert_eq!(closed["closed"], 4001);
}

#[tokio::test]
async fn closing_the_socket_frees_the_tenant_channel() {
    let (url, bus) = server().await;
    let c = connect_and_auth(&url, token(Some(TENANT), &[])).await;
    settle(&bus, 1).await;
    assert_eq!(bus.active_tenants(), 1);
    drop(c);
    for _ in 0..100 {
        if bus.active_tenants() == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("tenant channel was not released");
}

#[test]
fn close_code_type_is_compatible() {
    // Guard against tungstenite changing its CloseCode conversions.
    assert_eq!(u16::from(CloseCode::from(4001)), 4001);
}
