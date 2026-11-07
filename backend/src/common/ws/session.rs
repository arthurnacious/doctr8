//! Per-connection state and the wire protocol. No I/O here, so every rule
//! is unit-testable.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::event::{Event, EventKind};
use crate::common::{jwt::Claims, permission::Permission};

pub const MAX_ACCOUNT_SUBSCRIPTIONS: usize = 20;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Auth { token: String },
    Subscribe { account_id: Uuid },
    Unsubscribe { account_id: Uuid },
    Ping,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Ready { tenant_id: Uuid, expires_at: i64 },
    Event {
        event: EventKind,
        id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        account_id: Option<Uuid>,
        #[serde(skip_serializing_if = "Option::is_none")]
        invoice_id: Option<Uuid>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    Subscribed { account_id: Uuid },
    Unsubscribed { account_id: Uuid },
    Reauthenticated { expires_at: i64 },
    Resync,
    Pong,
    Error { code: &'static str, message: &'static str },
}

impl ServerMessage {
    pub fn from_event(e: &Event) -> Self {
        Self::Event {
            event: e.kind,
            id: e.id,
            account_id: e.account_id,
            invoice_id: e.invoice_id,
            status: e.status.clone(),
        }
    }
}

/// Why a connection must be closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    AuthFailed,
    Expired,
    NoPractice,
    AuthTimeout,
    IdentityChanged,
    BadMessage,
}

impl CloseReason {
    pub const fn code(self) -> u16 {
        match self {
            Self::AuthFailed | Self::Expired => 4001,
            Self::NoPractice => 4003,
            Self::AuthTimeout => 4008,
            Self::IdentityChanged => 4009,
            Self::BadMessage => 4400,
        }
    }

    pub const fn reason(self) -> &'static str {
        match self {
            Self::AuthFailed => "authentication failed",
            Self::Expired => "token expired",
            Self::NoPractice => "select a practice first",
            Self::AuthTimeout => "authenticate within 10 seconds",
            Self::IdentityChanged => "user or practice changed; reconnect",
            Self::BadMessage => "unrecognised message",
        }
    }
}

#[derive(Debug)]
pub struct Session {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub expires_at: i64,
    permissions: HashSet<Permission>,
    accounts: HashSet<Uuid>,
}

impl Session {
    pub fn start(claims: &Claims) -> Result<Self, CloseReason> {
        let tenant_id = claims.tid.ok_or(CloseReason::NoPractice)?;
        Ok(Self {
            user_id: claims.sub,
            tenant_id,
            expires_at: claims.exp,
            permissions: permissions(claims),
            accounts: HashSet::new(),
        })
    }

    /// A fresh token for the same user and practice (after a refresh):
    /// extends the connection and picks up permission changes.
    pub fn reauthenticate(&mut self, claims: &Claims) -> Result<(), CloseReason> {
        if claims.sub != self.user_id || claims.tid != Some(self.tenant_id) {
            return Err(CloseReason::IdentityChanged);
        }
        self.expires_at = claims.exp;
        self.permissions = permissions(claims);
        Ok(())
    }

    pub fn subscribe(&mut self, account_id: Uuid) -> Result<(), ServerMessage> {
        if !self.accounts.contains(&account_id) && self.accounts.len() >= MAX_ACCOUNT_SUBSCRIPTIONS {
            return Err(ServerMessage::Error {
                code: "too_many_subscriptions",
                message: "Unsubscribe from an account before following another",
            });
        }
        self.accounts.insert(account_id);
        Ok(())
    }

    pub fn unsubscribe(&mut self, account_id: Uuid) {
        self.accounts.remove(&account_id);
    }

    #[cfg(test)]
    pub fn accounts(&self) -> std::collections::hash_set::Iter<'_, Uuid> {
        self.accounts.iter()
    }

    pub fn is_expired(&self, now: i64) -> bool {
        now >= self.expires_at
    }

    /// Should this connection receive `event`?
    pub fn wants(&self, event: &Event) -> bool {
        event.tenant_id == self.tenant_id
            && self.permissions.contains(&event.kind.required_permission())
            && (event.kind.tenant_wide() || event.account_id.is_some_and(|a| self.accounts.contains(&a)))
    }
}

fn permissions(claims: &Claims) -> HashSet<Permission> {
    claims.perms.iter().filter_map(|k| Permission::from_key(k)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::{SESSION, TENANT, USER, keys, token};

    const ACCOUNT: Uuid = Uuid::from_u128(0xACC);

    fn claims(tenant: Option<Uuid>, perms: &[Permission]) -> Claims {
        keys().verify(&token(tenant, perms)).unwrap()
    }

    fn session(perms: &[Permission]) -> Session {
        Session::start(&claims(Some(TENANT), perms)).unwrap()
    }

    fn event(kind: EventKind, account: Option<Uuid>) -> Event {
        let e = Event::new(TENANT, kind, Uuid::from_u128(1));
        match account {
            Some(a) => e.account(a),
            None => e,
        }
    }

    #[test]
    fn needs_a_selected_practice() {
        assert_eq!(Session::start(&claims(None, Permission::ALL)).unwrap_err(), CloseReason::NoPractice);
        let s = session(&[]);
        assert_eq!((s.user_id, s.tenant_id), (USER, TENANT));
        let _ = SESSION;
    }

    #[test]
    fn tenant_wide_events_need_only_permission() {
        let s = session(&[Permission::BillingView]);
        assert!(s.wants(&event(EventKind::ClaimStatusChanged, Some(ACCOUNT))));
        assert!(s.wants(&event(EventKind::RemittanceReceived, None)));
    }

    #[test]
    fn events_require_their_permission() {
        let s = session(&[Permission::PatientsView, Permission::AccountsView]);
        assert!(!s.wants(&event(EventKind::ClaimStatusChanged, None)));
    }

    #[test]
    fn account_events_need_a_subscription() {
        let mut s = session(&[Permission::BillingView]);
        let balance = event(EventKind::AccountBalanceChanged, Some(ACCOUNT));
        assert!(!s.wants(&balance));
        s.subscribe(ACCOUNT).unwrap();
        assert!(s.wants(&balance));
        assert!(!s.wants(&event(EventKind::AccountBalanceChanged, Some(Uuid::from_u128(7)))), "other account");
        s.unsubscribe(ACCOUNT);
        assert!(!s.wants(&balance));
    }

    #[test]
    fn subscription_does_not_bypass_permissions() {
        let mut s = session(&[Permission::AccountsView]);
        s.subscribe(ACCOUNT).unwrap();
        assert!(!s.wants(&event(EventKind::AccountBalanceChanged, Some(ACCOUNT))));
    }

    #[test]
    fn other_tenants_events_are_never_delivered() {
        let mut s = session(Permission::ALL);
        s.subscribe(ACCOUNT).unwrap();
        let mut e = event(EventKind::ClaimStatusChanged, Some(ACCOUNT));
        e.tenant_id = Uuid::from_u128(0xBAD);
        assert!(!s.wants(&e));
    }

    #[test]
    fn subscriptions_are_capped() {
        let mut s = session(&[Permission::BillingView]);
        for n in 0..MAX_ACCOUNT_SUBSCRIPTIONS as u128 {
            s.subscribe(Uuid::from_u128(n)).unwrap();
        }
        assert!(s.subscribe(Uuid::from_u128(999)).is_err());
        assert!(s.subscribe(Uuid::from_u128(0)).is_ok(), "re-subscribing is idempotent");
        assert_eq!(s.accounts().count(), MAX_ACCOUNT_SUBSCRIPTIONS);
    }

    #[test]
    fn reauthentication_updates_permissions_and_expiry() {
        let mut s = session(&[]);
        let before = s.expires_at;
        let fresh = Claims { exp: before + 600, ..claims(Some(TENANT), &[Permission::BillingView]) };
        s.reauthenticate(&fresh).unwrap();
        assert_eq!(s.expires_at, before + 600);
        assert!(s.wants(&event(EventKind::ClaimStatusChanged, None)));
        // And permissions can be taken away again.
        s.reauthenticate(&claims(Some(TENANT), &[])).unwrap();
        assert!(!s.wants(&event(EventKind::ClaimStatusChanged, None)));
    }

    #[test]
    fn reauthentication_cannot_switch_practice_or_user() {
        let mut s = session(&[]);
        assert_eq!(s.reauthenticate(&claims(Some(Uuid::from_u128(5)), &[])), Err(CloseReason::IdentityChanged));
        assert_eq!(s.reauthenticate(&claims(None, &[])), Err(CloseReason::IdentityChanged));
        let other_user = Claims { sub: Uuid::from_u128(77), ..claims(Some(TENANT), &[]) };
        assert_eq!(s.reauthenticate(&other_user), Err(CloseReason::IdentityChanged));
    }

    #[test]
    fn expiry() {
        let s = session(&[]);
        assert!(!s.is_expired(s.expires_at - 1));
        assert!(s.is_expired(s.expires_at));
    }

    #[test]
    fn client_messages_parse() {
        let m: ClientMessage = serde_json::from_str(r#"{"type":"auth","token":"t"}"#).unwrap();
        assert!(matches!(m, ClientMessage::Auth { token } if token == "t"));
        let m: ClientMessage = serde_json::from_str(&format!(r#"{{"type":"subscribe","account_id":"{ACCOUNT}"}}"#)).unwrap();
        assert!(matches!(m, ClientMessage::Subscribe { account_id } if account_id == ACCOUNT));
        assert!(matches!(serde_json::from_str(r#"{"type":"ping"}"#).unwrap(), ClientMessage::Ping));
        assert!(serde_json::from_str::<ClientMessage>(r#"{"type":"subscribe","account_id":"nope"}"#).is_err());
        assert!(serde_json::from_str::<ClientMessage>(r#"{"type":"shutdown"}"#).is_err());
    }

    #[test]
    fn server_messages_serialise_compactly() {
        let e = event(EventKind::AccountBalanceChanged, Some(ACCOUNT));
        let v = serde_json::to_value(ServerMessage::from_event(&e)).unwrap();
        assert_eq!(v["type"], "event");
        assert_eq!(v["event"], "account.balance_changed");
        assert_eq!(v["account_id"], ACCOUNT.to_string());
        assert!(v.get("tenant_id").is_none(), "tenant is implicit in the connection");
        assert!(v.get("status").is_none());
        assert_eq!(serde_json::to_value(ServerMessage::Resync).unwrap(), serde_json::json!({"type": "resync"}));
    }

    #[test]
    fn close_codes_are_application_range() {
        for r in [CloseReason::AuthFailed, CloseReason::Expired, CloseReason::NoPractice, CloseReason::AuthTimeout, CloseReason::IdentityChanged, CloseReason::BadMessage] {
            assert!((4000..5000).contains(&r.code()));
            assert!(!r.reason().is_empty());
        }
    }
}
