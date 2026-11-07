//! Event bus.
//!
//! Publishing: `bus.publish(&mut tx, &event)` runs `pg_notify` inside the
//! caller's transaction. Postgres delivers notifications only when that
//! transaction COMMITs (and drops them on rollback), so clients are never
//! told about changes that didn't happen, and never before they're visible.
//!
//! Delivery: every API instance runs `listen()`, which LISTENs on one
//! channel and fans each event out to that instance's per-tenant broadcast
//! channels. Multiple instances therefore work without extra infrastructure.
//! If the listener loses its connection, events may be missed, so every
//! connected client is told to `resync`.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use sqlx::{PgConnection, PgPool, postgres::PgListener};
use tokio::sync::broadcast;
use uuid::Uuid;

use super::event::Event;

pub const CHANNEL: &str = "doctr8_events";
/// Per-tenant buffer. A client that falls this far behind gets a `resync`.
const TENANT_BUFFER: usize = 256;

#[derive(Debug, Clone)]
pub enum Signal {
    Event(Arc<Event>),
    /// Events may have been lost; clients should refetch what's on screen.
    Resync,
}

#[derive(Clone, Default)]
pub struct EventBus {
    tenants: Arc<Mutex<HashMap<Uuid, broadcast::Sender<Signal>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    fn map(&self) -> MutexGuard<'_, HashMap<Uuid, broadcast::Sender<Signal>>> {
        self.tenants.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Queue `event` for delivery when the caller's transaction commits.
    pub async fn publish(&self, conn: &mut PgConnection, event: &Event) -> Result<(), sqlx::Error> {
        let payload = serde_json::to_string(event).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(CHANNEL)
            .bind(payload)
            .execute(conn)
            .await?;
        Ok(())
    }

    /// Fan an event out to this instance's subscribers in its tenant.
    pub fn dispatch(&self, event: Event) {
        let sender = self.map().get(&event.tenant_id).cloned();
        if let Some(sender) = sender {
            let _ = sender.send(Signal::Event(Arc::new(event)));
        }
    }

    /// Tell every connected client to refetch.
    pub fn resync_all(&self) {
        for sender in self.map().values() {
            let _ = sender.send(Signal::Resync);
        }
    }

    pub fn subscribe(&self, tenant_id: Uuid) -> Subscription {
        let rx = self
            .map()
            .entry(tenant_id)
            .or_insert_with(|| broadcast::channel(TENANT_BUFFER).0)
            .subscribe();
        Subscription { rx: Some(rx), tenant_id, bus: self.clone() }
    }

    /// Tenants with at least one live subscription on this instance.
    pub fn active_tenants(&self) -> usize {
        self.map().len()
    }

    /// Run forever: LISTEN for events and dispatch them, reconnecting with
    /// backoff. Spawn once per process.
    pub async fn listen(self, pool: PgPool) {
        let mut backoff = Duration::from_millis(500);
        loop {
            match self.listen_once(&pool).await {
                Ok(()) => backoff = Duration::from_millis(500),
                Err(err) => tracing::warn!(error = %err, "event listener disconnected"),
            }
            self.resync_all();
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    }

    async fn listen_once(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        let mut listener = PgListener::connect_with(pool).await?;
        listener.listen(CHANNEL).await?;
        tracing::info!(channel = CHANNEL, "event listener connected");
        loop {
            match listener.try_recv().await? {
                Some(notification) => match serde_json::from_str::<Event>(notification.payload()) {
                    Ok(event) => self.dispatch(event),
                    // Never log the payload itself.
                    Err(_) => tracing::warn!("ignoring malformed event notification"),
                },
                // The listener reconnected by itself; anything sent meanwhile is lost.
                None => self.resync_all(),
            }
        }
    }
}

/// A connection's view of its tenant's events. Dropping it frees the
/// tenant's channel when it was the last subscriber.
pub struct Subscription {
    rx: Option<broadcast::Receiver<Signal>>,
    tenant_id: Uuid,
    bus: EventBus,
}

impl Subscription {
    pub async fn recv(&mut self) -> Result<Signal, broadcast::error::RecvError> {
        match self.rx.as_mut() {
            Some(rx) => rx.recv().await,
            None => Err(broadcast::error::RecvError::Closed),
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        drop(self.rx.take());
        let mut map = self.bus.map();
        if map.get(&self.tenant_id).is_some_and(|s| s.receiver_count() == 0) {
            map.remove(&self.tenant_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast::error::{RecvError, TryRecvError};

    use super::*;
    use crate::common::ws::event::EventKind;

    fn event(tenant: u128, id: u128) -> Event {
        Event::new(Uuid::from_u128(tenant), EventKind::RemittanceReceived, Uuid::from_u128(id))
    }

    fn try_recv(sub: &mut Subscription) -> Result<Signal, TryRecvError> {
        sub.rx.as_mut().unwrap().try_recv()
    }

    #[test]
    fn delivers_only_within_the_tenant() {
        let bus = EventBus::new();
        let mut a = bus.subscribe(Uuid::from_u128(1));
        let mut b = bus.subscribe(Uuid::from_u128(2));
        bus.dispatch(event(1, 10));
        assert!(matches!(try_recv(&mut a), Ok(Signal::Event(e)) if e.id == Uuid::from_u128(10)));
        assert!(matches!(try_recv(&mut b), Err(TryRecvError::Empty)));
    }

    #[test]
    fn every_subscriber_in_a_tenant_gets_the_event() {
        let bus = EventBus::new();
        let mut a = bus.subscribe(Uuid::from_u128(1));
        let mut b = bus.subscribe(Uuid::from_u128(1));
        bus.dispatch(event(1, 10));
        assert!(matches!(try_recv(&mut a), Ok(Signal::Event(_))));
        assert!(matches!(try_recv(&mut b), Ok(Signal::Event(_))));
    }

    #[test]
    fn dispatch_without_subscribers_is_a_noop() {
        let bus = EventBus::new();
        bus.dispatch(event(1, 10));
        assert_eq!(bus.active_tenants(), 0, "no channel is created just to publish");
    }

    #[test]
    fn channels_are_freed_with_their_last_subscriber() {
        let bus = EventBus::new();
        let a = bus.subscribe(Uuid::from_u128(1));
        let b = bus.subscribe(Uuid::from_u128(1));
        let c = bus.subscribe(Uuid::from_u128(2));
        assert_eq!(bus.active_tenants(), 2);
        drop(a);
        assert_eq!(bus.active_tenants(), 2, "tenant 1 still has a subscriber");
        drop(b);
        assert_eq!(bus.active_tenants(), 1);
        drop(c);
        assert_eq!(bus.active_tenants(), 0);
    }

    #[test]
    fn resync_reaches_every_tenant() {
        let bus = EventBus::new();
        let mut a = bus.subscribe(Uuid::from_u128(1));
        let mut b = bus.subscribe(Uuid::from_u128(2));
        bus.resync_all();
        assert!(matches!(try_recv(&mut a), Ok(Signal::Resync)));
        assert!(matches!(try_recv(&mut b), Ok(Signal::Resync)));
    }

    #[tokio::test]
    async fn slow_consumers_see_lag_instead_of_blocking_others() {
        let bus = EventBus::new();
        let mut slow = bus.subscribe(Uuid::from_u128(1));
        for i in 0..(TENANT_BUFFER as u128 + 10) {
            bus.dispatch(event(1, i));
        }
        assert!(matches!(slow.recv().await, Err(RecvError::Lagged(_))));
    }
}
