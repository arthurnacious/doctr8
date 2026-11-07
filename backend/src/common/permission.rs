//! The fixed permission catalogue. This enum is the source of truth; the
//! `permissions` table mirrors it (seeded by migrations) so role_permissions
//! has a real foreign key. `verify_catalogue` runs at startup.

use std::collections::BTreeSet;

use serde::{Serialize, Serializer};
use sqlx::PgPool;

use crate::common::error::{AppError, AppResult};

/// Implemented by the zero-sized types in [`perm`] so a permission can be a
/// type parameter: `Require<perm::RolesManage>`.
pub trait PermissionMarker: Send + Sync + 'static {
    const PERMISSION: Permission;
}

macro_rules! define_permissions {
    ($( $variant:ident => $key:literal : $desc:literal ),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Permission { $( $variant ),+ }

        impl Permission {
            pub const ALL: &'static [Permission] = &[ $( Permission::$variant ),+ ];

            pub const fn key(self) -> &'static str {
                match self { $( Permission::$variant => $key ),+ }
            }

            pub const fn description(self) -> &'static str {
                match self { $( Permission::$variant => $desc ),+ }
            }

            pub fn from_key(key: &str) -> Option<Self> {
                match key { $( $key => Some(Permission::$variant), )+ _ => None }
            }
        }

        /// Type-level permissions for the `Require<P>` guard.
        #[allow(dead_code)] // markers exist for every permission, used or not
        pub mod perm {
            $(
                pub struct $variant;
                impl super::PermissionMarker for $variant {
                    const PERMISSION: super::Permission = super::Permission::$variant;
                }
            )+
        }
    };
}

// Adding a permission: add a line here AND a migration inserting it into
// `permissions` (and granting it to Owner roles). Startup fails otherwise.
define_permissions! {
    PracticeView   => "practice.view"   : "View practice profile and settings",
    PracticeManage => "practice.manage" : "Edit practice profile and settings",
    MembersView    => "members.view"    : "View the practice's members",
    MembersInvite  => "members.invite"  : "Invite people to join the practice",
    MembersManage  => "members.manage"  : "Suspend or remove members",
    RolesView      => "roles.view"      : "View roles and their permissions",
    RolesManage    => "roles.manage"    : "Create, edit and delete roles",
    RolesAssign    => "roles.assign"    : "Assign roles to members",
}

impl Serialize for Permission {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.key())
    }
}

/// Parse user-supplied keys, rejecting unknown ones. Deduplicates and sorts.
pub fn parse_keys(keys: &[String]) -> AppResult<BTreeSet<Permission>> {
    let mut unknown = Vec::new();
    let mut parsed = BTreeSet::new();
    for key in keys {
        match Permission::from_key(key) {
            Some(p) => {
                parsed.insert(p);
            }
            None => unknown.push(key.as_str()),
        }
    }
    if !unknown.is_empty() {
        return Err(AppError::validation(format!(
            "Unknown permissions: {}",
            unknown.join(", ")
        )));
    }
    Ok(parsed)
}

/// Keys read from the database, silently dropping any the code doesn't know.
pub fn from_db_keys<'a>(keys: impl IntoIterator<Item = &'a str>) -> Vec<Permission> {
    let set: BTreeSet<_> = keys.into_iter().filter_map(Permission::from_key).collect();
    set.into_iter().collect()
}

pub async fn verify_catalogue(pool: &PgPool) -> anyhow::Result<()> {
    let in_db: BTreeSet<String> = sqlx::query_scalar("SELECT key FROM permissions")
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
    let in_code: BTreeSet<String> = Permission::ALL.iter().map(|p| p.key().to_owned()).collect();

    if in_db != in_code {
        anyhow::bail!(
            "permission catalogue mismatch — in code but not database: {:?}; in database but not code: {:?}. Write a migration.",
            in_code.difference(&in_db).collect::<Vec<_>>(),
            in_db.difference(&in_code).collect::<Vec<_>>(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_round_trip() {
        let keys: BTreeSet<_> = Permission::ALL.iter().map(|p| p.key()).collect();
        assert_eq!(keys.len(), Permission::ALL.len());
        for p in Permission::ALL {
            assert_eq!(Permission::from_key(p.key()), Some(*p));
        }
    }

    #[test]
    fn keys_match_db_check_constraint_shape() {
        // Mirrors CHECK (key ~ '^[a-z][a-z_]*(\.[a-z][a-z_]*)+$')
        for p in Permission::ALL {
            let segments: Vec<_> = p.key().split('.').collect();
            assert!(segments.len() >= 2, "{}", p.key());
            for s in segments {
                assert!(s.starts_with(|c: char| c.is_ascii_lowercase()), "{}", p.key());
                assert!(s.chars().all(|c| c.is_ascii_lowercase() || c == '_'), "{}", p.key());
            }
        }
    }

    #[test]
    fn parse_keys_rejects_unknown() {
        let err = parse_keys(&["roles.view".into(), "nope.nope".into()]).unwrap_err();
        assert!(err.to_string().contains("nope.nope"));
    }
}
