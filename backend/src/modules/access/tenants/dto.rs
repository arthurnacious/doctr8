use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::AppResult,
    validation::{self, double_option},
};

/// A practice as shown in the practice picker.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PracticeSummary {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TenantResponse {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub pcns_number: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub pcns_number: Option<String>,
}

pub struct NewTenant {
    pub name: String,
    pub slug: String,
    pub pcns_number: Option<String>,
}

impl CreateTenantRequest {
    pub fn validate(self) -> AppResult<NewTenant> {
        Ok(NewTenant {
            name: validation::text("name", &self.name, 200)?,
            slug: validation::slug(&self.slug)?,
            pcns_number: match self.pcns_number.as_deref().map(str::trim) {
                None | Some("") => None,
                Some(v) => Some(validation::pcns_number(v)?),
            },
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenantRequest {
    #[serde(default)]
    pub name: Option<String>,
    /// Omit to leave unchanged, `null` to clear.
    #[serde(default, deserialize_with = "double_option")]
    pub pcns_number: Option<Option<String>>,
}

pub struct TenantChanges {
    pub name: Option<String>,
    pub pcns_number: Option<Option<String>>,
}

impl UpdateTenantRequest {
    pub fn validate(self) -> AppResult<TenantChanges> {
        Ok(TenantChanges {
            name: self.name.map(|n| validation::text("name", &n, 200)).transpose()?,
            pcns_number: match self.pcns_number {
                None => None,
                Some(None) => Some(None),
                Some(Some(v)) if v.trim().is_empty() => Some(None),
                Some(Some(v)) => Some(Some(validation::pcns_number(&v)?)),
            },
        })
    }
}
