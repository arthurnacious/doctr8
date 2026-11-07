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
    PatientsView   => "patients.view"   : "View patient demographics and files",
    PatientsManage => "patients.manage" : "Register patients and edit their details",
    AccountsView   => "accounts.view"   : "View accounts and medical aid details",
    AccountsManage => "accounts.manage" : "Edit accounts and medical aid memberships",
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

    /// Every key inserted into `permissions` by any migration. Lets the
    /// enum/database drift check run without a database.
    fn keys_seeded_by_migrations() -> BTreeSet<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        files.sort();
        let mut keys = BTreeSet::new();
        for file in files {
            let sql = std::fs::read_to_string(&file).unwrap();
            for (_, rest) in sql.match_indices("INSERT INTO permissions").map(|(i, _)| sql.split_at(i)) {
                let stmt = &rest[..rest.find(';').unwrap()];
                for chunk in stmt.split("('").skip(1) {
                    keys.insert(chunk[..chunk.find('\'').unwrap()].to_owned());
                }
            }
        }
        keys
    }

    #[test]
    fn enum_matches_permissions_seeded_by_migrations() {
        let in_code: BTreeSet<String> = Permission::ALL.iter().map(|p| p.key().to_owned()).collect();
        assert_eq!(in_code, keys_seeded_by_migrations(), "add a migration (and grant it to Owner roles)");
    }

    #[test]
    fn descriptions_are_present() {
        for p in Permission::ALL {
            assert!(!p.description().is_empty(), "{}", p.key());
        }
    }

    #[test]
    fn markers_carry_their_permission() {
        assert_eq!(<perm::RolesManage as PermissionMarker>::PERMISSION, Permission::RolesManage);
        assert_eq!(<perm::PatientsView as PermissionMarker>::PERMISSION, Permission::PatientsView);
    }

    #[test]
    fn serializes_as_key() {
        assert_eq!(serde_json::to_string(&Permission::AccountsManage).unwrap(), "\"accounts.manage\"");
    }

    #[test]
    fn parse_keys_dedupes_and_sorts() {
        let parsed = parse_keys(&["roles.view".into(), "practice.view".into(), "roles.view".into()]).unwrap();
        assert_eq!(parsed.into_iter().collect::<Vec<_>>(), vec![Permission::PracticeView, Permission::RolesView]);
        assert!(parse_keys(&[]).unwrap().is_empty());
    }

    #[test]
    fn parse_keys_lists_all_unknown_keys() {
        let err = parse_keys(&["a.b".into(), "roles.view".into(), "c.d".into()]).unwrap_err().to_string();
        assert!(err.contains("a.b") && err.contains("c.d") && !err.contains("roles.view"));
    }

    #[test]
    fn from_db_keys_drops_unknown_and_dedupes() {
        let perms = from_db_keys(["roles.view", "gone.away", "roles.view", "members.view"]);
        assert_eq!(perms, vec![Permission::MembersView, Permission::RolesView]);
    }

    #[test]
    fn parse_keys_rejects_unknown() {
        let err = parse_keys(&["roles.view".into(), "nope.nope".into()]).unwrap_err();
        assert!(err.to_string().contains("nope.nope"));
    }
}
