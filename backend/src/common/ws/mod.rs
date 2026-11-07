//! Real-time notifications over WebSocket.
//!
//! * `event`   — what an event looks like and who may receive it
//! * `bus`     — publish (inside a DB transaction, via NOTIFY) and fan-out
//! * `session` — per-connection auth, subscriptions and filtering (pure)
//! * `handler` — the `/ws` endpoint and socket loop
//!
//! Events are small notifications: what changed and its id. Clients refetch
//! over REST, so every read is still permission-checked by the API. Events
//! never carry personal information.
//!
//! Protocol (JSON text frames):
//!
//! ```text
//! client → {"type":"auth","token":"<access JWT>"}         first frame, and again after each refresh
//!          {"type":"subscribe","account_id":"<uuid>"}     follow one account (an open account screen)
//!          {"type":"unsubscribe","account_id":"<uuid>"}
//!          {"type":"ping"}
//! server → {"type":"ready","tenant_id":..,"expires_at":..}
//!          {"type":"event","event":"claim.status_changed","id":..,"account_id":..,"invoice_id":..,"status":..}
//!          {"type":"subscribed"|"unsubscribed","account_id":..}
//!          {"type":"resync"}        events may have been missed: refetch what's on screen
//!          {"type":"pong"} | {"type":"error","code":..,"message":..}
//! close codes: 4001 auth failed/expired · 4003 no practice selected · 4008 auth timeout
//!              4009 practice or user changed (reconnect) · 4400 bad message
//! ```

pub mod bus;
pub mod event;
pub mod handler;
pub mod session;

use axum::{Router, routing::get};

pub use bus::EventBus;
pub use event::{Event, EventKind};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/ws", get(handler::upgrade))
}
