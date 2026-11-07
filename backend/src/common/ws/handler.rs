//! `GET /ws` — the socket loop. Thin glue around `Session`.

use std::time::Duration;

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use tokio::sync::broadcast::error::RecvError;

use super::{
    bus::{EventBus, Signal},
    session::{ClientMessage, CloseReason, ServerMessage, Session},
};
use crate::common::jwt::{Claims, JwtKeys};

const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_MESSAGE_BYTES: usize = 16 * 1024;

/// Browsers can't set headers on a WebSocket, so the token normally comes
/// as the first message (never in the URL, which ends up in logs). Non-browser
/// clients may send `Authorization: Bearer` on the upgrade request instead.
pub async fn upgrade(
    ws: WebSocketUpgrade,
    State(bus): State<EventBus>,
    State(keys): State<JwtKeys>,
    headers: HeaderMap,
) -> Response {
    let preauth = match headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        None => None,
        Some(value) => match value.strip_prefix("Bearer ").map(|t| keys.verify(t)) {
            Some(Ok(claims)) => Some(claims),
            _ => return StatusCode::UNAUTHORIZED.into_response(),
        },
    };
    ws.max_message_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| run(socket, bus, keys, preauth))
}

async fn run(mut socket: WebSocket, bus: EventBus, keys: JwtKeys, preauth: Option<Claims>) {
    let claims = match preauth {
        Some(c) => c,
        None => match tokio::time::timeout(AUTH_TIMEOUT, first_auth(&mut socket, &keys)).await {
            Ok(Ok(c)) => c,
            Ok(Err(reason)) => return close(socket, reason).await,
            Err(_) => return close(socket, CloseReason::AuthTimeout).await,
        },
    };
    let mut session = match Session::start(&claims) {
        Ok(s) => s,
        Err(reason) => return close(socket, reason).await,
    };

    let mut events = bus.subscribe(session.tenant_id);
    let ready = ServerMessage::Ready { tenant_id: session.tenant_id, expires_at: session.expires_at };
    if send(&mut socket, &ready).await.is_err() {
        return;
    }

    loop {
        let until_expiry = Duration::from_secs((session.expires_at - Utc::now().timestamp()).max(0) as u64);
        tokio::select! {
            incoming = socket.recv() => {
                let text = match incoming {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                    Some(Ok(Message::Binary(_))) => return close(socket, CloseReason::BadMessage).await,
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return,
                };
                let reply = match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Auth { token }) => match keys.verify(&token) {
                        Ok(claims) => match session.reauthenticate(&claims) {
                            Ok(()) => ServerMessage::Reauthenticated { expires_at: session.expires_at },
                            Err(reason) => return close(socket, reason).await,
                        },
                        Err(_) => return close(socket, CloseReason::AuthFailed).await,
                    },
                    Ok(ClientMessage::Subscribe { account_id }) => match session.subscribe(account_id) {
                        Ok(()) => ServerMessage::Subscribed { account_id },
                        Err(error) => error,
                    },
                    Ok(ClientMessage::Unsubscribe { account_id }) => {
                        session.unsubscribe(account_id);
                        ServerMessage::Unsubscribed { account_id }
                    }
                    Ok(ClientMessage::Ping) => ServerMessage::Pong,
                    Err(_) => return close(socket, CloseReason::BadMessage).await,
                };
                if send(&mut socket, &reply).await.is_err() {
                    return;
                }
            }
            signal = events.recv() => {
                let message = match signal {
                    Ok(Signal::Event(event)) if session.wants(&event) => ServerMessage::from_event(&event),
                    Ok(Signal::Event(_)) => continue,
                    Ok(Signal::Resync) | Err(RecvError::Lagged(_)) => ServerMessage::Resync,
                    Err(RecvError::Closed) => return,
                };
                if send(&mut socket, &message).await.is_err() {
                    return;
                }
            }
            _ = tokio::time::sleep(until_expiry) => {
                if session.is_expired(Utc::now().timestamp()) {
                    return close(socket, CloseReason::Expired).await;
                }
            }
        }
    }
}

async fn first_auth(socket: &mut WebSocket, keys: &JwtKeys) -> Result<Claims, CloseReason> {
    loop {
        match socket.recv().await {
            Some(Ok(Message::Text(text))) => {
                return match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Auth { token }) => keys.verify(&token).map_err(|_| CloseReason::AuthFailed),
                    _ => Err(CloseReason::AuthFailed),
                };
            }
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
            _ => return Err(CloseReason::AuthFailed),
        }
    }
}

async fn send(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    let text = serde_json::to_string(message).unwrap_or_default();
    socket.send(Message::Text(text.into())).await
}

async fn close(mut socket: WebSocket, reason: CloseReason) {
    let frame = CloseFrame { code: reason.code(), reason: reason.reason().into() };
    let _ = socket.send(Message::Close(Some(frame))).await;
}
