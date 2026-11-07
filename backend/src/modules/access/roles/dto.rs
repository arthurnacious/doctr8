use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::AppResult,
    permission::{self, Permission},
    validation::{self, double_option},
};

#[derive(Debug, Serialize)]
pub struct PermissionResponse {
    pub key: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RoleResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub permissions: Vec<Permission>,
    pub member_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

pub struct NewRole {
    pub name: String,
    pub description: Option<String>,
    pub permissions: BTreeSet<Permission>,
}

impl CreateRoleRequest {
    pub fn validate(self) -> AppResult<NewRole> {
        Ok(NewRole {
            name: validation::text("name", &self.name, 64)?,
            description: validation::optional_text("description", self.description.as_deref(), 500)?,
            permissions: permission::parse_keys(&self.permissions)?,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    #[serde(default)]
    pub name: Option<String>,
    /// Omit to leave unchanged, `null` to clear.
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
}

pub struct RoleChanges {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

impl UpdateRoleRequest {
    pub fn validate(self) -> AppResult<RoleChanges> {
        Ok(RoleChanges {
            name: self.name.map(|n| validation::text("name", &n, 64)).transpose()?,
            description: match self.description {
                None => None,
                Some(d) => Some(validation::optional_text("description", d.as_deref(), 500)?),
            },
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct SetPermissionsRequest {
    /// The complete set of permission keys (replaces existing).
    pub permissions: Vec<String>,
}
