use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::permission::Permission;

/// Everything that can be announced. Add new domains' events here (e.g.
/// `appointment.booked`) with their permission and audience.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
    #[serde(rename = "invoice.created")]
    InvoiceCreated,
    #[serde(rename = "invoice.credited")]
    InvoiceCredited,
    #[serde(rename = "claim.status_changed")]
    ClaimStatusChanged,
    #[serde(rename = "remittance.received")]
    RemittanceReceived,
    #[serde(rename = "remittance.allocated")]
    RemittanceAllocated,
    #[serde(rename = "account.balance_changed")]
    AccountBalanceChanged,
}

impl EventKind {
    pub const ALL: &'static [EventKind] = &[
        Self::InvoiceCreated,
        Self::InvoiceCredited,
        Self::ClaimStatusChanged,
        Self::RemittanceReceived,
        Self::RemittanceAllocated,
        Self::AccountBalanceChanged,
    ];

    /// A connection only receives the event if its token holds this.
    pub const fn required_permission(self) -> Permission {
        match self {
            Self::InvoiceCreated
            | Self::InvoiceCredited
            | Self::ClaimStatusChanged
            | Self::RemittanceReceived
            | Self::RemittanceAllocated
            | Self::AccountBalanceChanged => Permission::BillingView,
        }
    }

    /// Delivered to every connection in the practice (dashboards, claim
    /// queues). Otherwise only to connections subscribed to the event's account.
    pub const fn tenant_wide(self) -> bool {
        matches!(self, Self::ClaimStatusChanged | Self::RemittanceReceived | Self::RemittanceAllocated)
    }
}

/// A notification. Ids and statuses only — never names or other personal data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub tenant_id: Uuid,
    pub kind: EventKind,
    /// The thing that changed (invoice, claim, remittance, account ...).
    pub id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_id: Option<Uuid>,
    /// e.g. the new claim status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

impl Event {
    pub fn new(tenant_id: Uuid, kind: EventKind, id: Uuid) -> Self {
        Self { tenant_id, kind, id, account_id: None, invoice_id: None, status: None }
    }

    pub fn account(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    pub fn invoice(mut self, invoice_id: Uuid) -> Self {
        self.invoice_id = Some(invoice_id);
        self
    }

    pub fn status(mut self, status: &str) -> Self {
        self.status = Some(status.to_owned());
        self
    }

    pub fn balance_changed(tenant_id: Uuid, account_id: Uuid) -> Self {
        Self::new(tenant_id, EventKind::AccountBalanceChanged, account_id).account(account_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names() {
        assert_eq!(serde_json::to_value(EventKind::ClaimStatusChanged).unwrap(), "claim.status_changed");
        assert_eq!(serde_json::to_value(EventKind::AccountBalanceChanged).unwrap(), "account.balance_changed");
        for k in EventKind::ALL {
            let name = serde_json::to_value(k).unwrap();
            assert_eq!(serde_json::from_value::<EventKind>(name).unwrap(), *k);
        }
    }

    #[test]
    fn optional_fields_are_omitted() {
        let e = Event::new(Uuid::nil(), EventKind::RemittanceReceived, Uuid::from_u128(1));
        let v = serde_json::to_value(&e).unwrap();
        assert!(v.get("account_id").is_none() && v.get("status").is_none());
        assert_eq!(serde_json::from_value::<Event>(v).unwrap(), e);
    }

    #[test]
    fn builders() {
        let e = Event::new(Uuid::nil(), EventKind::ClaimStatusChanged, Uuid::from_u128(1))
            .account(Uuid::from_u128(2))
            .invoice(Uuid::from_u128(3))
            .status("paid");
        assert_eq!((e.account_id, e.invoice_id, e.status.as_deref()), (Some(Uuid::from_u128(2)), Some(Uuid::from_u128(3)), Some("paid")));
        let b = Event::balance_changed(Uuid::nil(), Uuid::from_u128(9));
        assert_eq!((b.id, b.account_id), (Uuid::from_u128(9), Some(Uuid::from_u128(9))));
    }

    #[test]
    fn audiences() {
        assert!(EventKind::ClaimStatusChanged.tenant_wide());
        assert!(!EventKind::AccountBalanceChanged.tenant_wide(), "balances only go to open account screens");
        assert!(!EventKind::InvoiceCreated.tenant_wide());
        for k in EventKind::ALL {
            assert_eq!(k.required_permission(), Permission::BillingView);
        }
    }

    #[test]
    fn payload_fits_in_a_postgres_notify() {
        let e = Event::new(Uuid::max(), EventKind::ClaimStatusChanged, Uuid::max())
            .account(Uuid::max())
            .invoice(Uuid::max())
            .status("submitted");
        assert!(serde_json::to_string(&e).unwrap().len() < 8000);
    }
}
