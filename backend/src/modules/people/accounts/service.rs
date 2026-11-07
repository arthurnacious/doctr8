use std::collections::HashSet;

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    dto::{AccountResponse, AddPatientRequest, MedicalAidRef, PersonRef, UpdateAccountRequest},
    repository::{self as repo, AccountRow},
};
use crate::{
    common::{
        auth::TenantUser,
        db,
        error::{AppError, AppResult},
    },
    modules::{
        access::tenants::dto::FileMode,
        people::{memberships, patients, persons},
    },
};

#[derive(Clone)]
pub struct AccountsService {
    pool: PgPool,
}

impl AccountsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, actor: &TenantUser, id: Uuid) -> AppResult<AccountResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let account = load_account(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(account)
    }

    /// Change who pays, the medical aid, or open/closed status. The resulting
    /// file must still satisfy its file-mode rules (e.g. switching medical aid
    /// changes who the 00 patient must be).
    pub async fn update(&self, actor: &TenantUser, id: Uuid, req: UpdateAccountRequest) -> AppResult<AccountResponse> {
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let account = find_locked(&mut tx, t, id).await?;

        if let Some(g) = req.guarantor_person_id {
            persons::service::ensure_exists(&mut tx, t, g).await?;
        }
        let guarantor = req.guarantor_person_id.unwrap_or(account.guarantor_person_id);
        let main_member = match req.membership_id {
            None => account.main_member_person_id,
            Some(None) => None,
            Some(Some(m)) => Some(memberships::service::load(&mut tx, t, m).await?.main_member_person_id),
        };
        let principal = main_member.unwrap_or(guarantor);

        let slots = current_slots(&mut tx, t, id).await?;
        check_file_rules(FileMode::from_db(&account.file_mode)?, principal, &slots, false)?;

        repo::update(&mut tx, t, id, req.guarantor_person_id, req.membership_id, req.status.map(|s| s.as_str()))
            .await?;
        let updated = load_account(&mut tx, t, id).await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Add an existing person as a patient on an existing account. Only
    /// family-per-file accounts accept more than one patient.
    pub async fn add_patient(&self, actor: &TenantUser, id: Uuid, req: AddPatientRequest) -> AppResult<AccountResponse> {
        let code = req.validated_code()?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let account = find_locked(&mut tx, t, id).await?;
        if account.status != "open" {
            return Err(AppError::Conflict("Patients can't be added to a closed account".into()));
        }
        persons::service::ensure_exists(&mut tx, t, req.person_id).await?;

        let mut slots = current_slots(&mut tx, t, id).await?;
        let principal = account.principal();
        let code = match code {
            Some(c) => c,
            None => default_dependant_code(req.person_id, principal, account.membership_id.is_some(), &slots)?,
        };
        slots.push(Slot { person_id: req.person_id, dependant_code: code.clone() });
        check_file_rules(FileMode::from_db(&account.file_mode)?, principal, &slots, true)?;

        patients::repository::insert(&mut tx, t, req.person_id, id, &code, actor.user_id).await?;
        let updated = load_account(&mut tx, t, id).await?;
        tx.commit().await?;
        Ok(updated)
    }
}

// -----------------------------------------------------------------------------
// File-mode rules. Pure functions, mirrored by app.validate_account() in the
// database (which re-checks at COMMIT so nothing can bypass them).
// -----------------------------------------------------------------------------

/// A patient's place on an account.
#[derive(Clone)]
pub struct Slot {
    pub person_id: Uuid,
    pub dependant_code: String,
}

pub const MAIN_MEMBER: &str = "00";

/// Check the complete set of patients an account would have.
///
/// * The principal (medical aid main member, or guarantor if private) is
///   always 00, and only the principal may be 00.
/// * patient-per-file / new-debtor-per-visit: exactly one patient.
/// * family-per-file: exactly one 00 patient plus any dependants.
/// * new-debtor-per-visit: no adding patients to an existing account.
pub fn check_file_rules(mode: FileMode, principal: Uuid, slots: &[Slot], adding_to_existing: bool) -> AppResult<()> {
    if slots.is_empty() {
        return Err(AppError::validation("A file needs at least one patient"));
    }
    if adding_to_existing && mode == FileMode::DebtorPerVisit {
        return Err(AppError::Conflict(
            "This account was opened in new-debtor-per-visit mode: open a new account for each visit instead of adding patients".into(),
        ));
    }

    let mut persons = HashSet::new();
    let mut codes = HashSet::new();
    for s in slots {
        if !persons.insert(s.person_id) {
            return Err(AppError::validation("The same person can't be a patient twice on one account"));
        }
        if !codes.insert(s.dependant_code.as_str()) {
            return Err(AppError::validation("Each patient on an account needs a different dependant code"));
        }
        let is_main_code = s.dependant_code == MAIN_MEMBER;
        let is_principal = s.person_id == principal;
        if is_main_code && !is_principal {
            return Err(AppError::validation(
                "Dependant code 00 is reserved for the principal member (the medical aid main member, or the guarantor on a private account)",
            ));
        }
        if is_principal && !is_main_code {
            return Err(AppError::validation("The principal member must have dependant code 00"));
        }
    }

    match mode {
        FileMode::PatientPerFile | FileMode::DebtorPerVisit if slots.len() > 1 => Err(AppError::Conflict(format!(
            "This account was opened in {} mode and can have only one patient",
            mode.label()
        ))),
        FileMode::FamilyPerFile if !codes.contains(MAIN_MEMBER) => Err(AppError::validation(
            "A family file must include the main member (dependant code 00) as a patient",
        )),
        _ => Ok(()),
    }
}

/// Dependant code to use when the caller didn't give one.
pub fn default_dependant_code(person_id: Uuid, principal: Uuid, has_medical_aid: bool, taken: &[Slot]) -> AppResult<String> {
    if person_id == principal {
        return Ok(MAIN_MEMBER.into());
    }
    if has_medical_aid {
        return Err(AppError::validation(
            "dependant_code is required for medical aid dependants: use the code on the membership card",
        ));
    }
    (1..=99)
        .map(|n| format!("{n:02}"))
        .find(|c| !taken.iter().any(|s| &s.dependant_code == c))
        .ok_or_else(|| AppError::validation("No free dependant codes left on this account"))
}

// -----------------------------------------------------------------------------
// Helpers shared with the patients module
// -----------------------------------------------------------------------------

async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<AccountRow> {
    repo::find(conn, tenant_id, id).await?.ok_or(AppError::NotFound("account"))
}

pub async fn find_locked(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<AccountRow> {
    if !repo::lock(conn, tenant_id, id).await? {
        return Err(AppError::NotFound("account"));
    }
    find(conn, tenant_id, id).await
}

pub async fn current_slots(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid) -> AppResult<Vec<Slot>> {
    Ok(repo::patients(conn, tenant_id, account_id)
        .await?
        .into_iter()
        .map(|p| Slot { person_id: p.person_id, dependant_code: p.dependant_code })
        .collect())
}

pub async fn load_account(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<AccountResponse> {
    let row = find(conn, tenant_id, id).await?;
    let patients = repo::patients(conn, tenant_id, id).await?;
    let medical_aid = match (row.membership_id, row.scheme_name, row.plan_name, row.membership_number, row.main_member_person_id) {
        (Some(membership_id), Some(scheme_name), Some(plan_name), Some(membership_number), Some(main)) => {
            Some(MedicalAidRef { membership_id, scheme_name, plan_name, membership_number, main_member_person_id: main })
        }
        _ => None,
    };
    Ok(AccountResponse {
        id: row.id,
        account_no: row.account_no,
        file_mode: row.file_mode,
        status: row.status,
        guarantor: PersonRef {
            id: row.guarantor_person_id,
            first_name: row.guarantor_first_name,
            last_name: row.guarantor_last_name,
        },
        medical_aid,
        patients,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn slot(person: u128, code: &str) -> Slot {
        Slot { person_id: id(person), dependant_code: code.into() }
    }

    const P: u128 = 1; // principal

    #[test]
    fn single_patient_modes_allow_exactly_one() {
        for mode in [FileMode::PatientPerFile, FileMode::DebtorPerVisit] {
            assert!(check_file_rules(mode, id(P), &[slot(2, "01")], false).is_ok());
            assert!(check_file_rules(mode, id(P), &[slot(P, "00"), slot(2, "01")], false).is_err());
        }
    }

    #[test]
    fn single_patient_file_need_not_include_main_member() {
        // A dependant can have their own file under patient-per-file.
        assert!(check_file_rules(FileMode::PatientPerFile, id(P), &[slot(3, "03")], false).is_ok());
    }

    #[test]
    fn family_needs_exactly_one_main_member() {
        let m = FileMode::FamilyPerFile;
        assert!(check_file_rules(m, id(P), &[slot(P, "00"), slot(2, "01"), slot(3, "02")], false).is_ok());
        assert!(check_file_rules(m, id(P), &[slot(2, "01")], false).is_err());
    }

    #[test]
    fn code_00_only_for_principal() {
        let m = FileMode::FamilyPerFile;
        assert!(check_file_rules(m, id(P), &[slot(2, "00")], false).is_err());
        assert!(check_file_rules(m, id(P), &[slot(P, "01")], false).is_err());
    }

    #[test]
    fn no_duplicates() {
        let m = FileMode::FamilyPerFile;
        assert!(check_file_rules(m, id(P), &[slot(P, "00"), slot(2, "01"), slot(3, "01")], false).is_err());
        assert!(check_file_rules(m, id(P), &[slot(P, "00"), slot(2, "01"), slot(2, "02")], false).is_err());
    }

    #[test]
    fn debtor_per_visit_never_adds_to_existing() {
        assert!(check_file_rules(FileMode::DebtorPerVisit, id(P), &[slot(2, "01")], true).is_err());
        assert!(check_file_rules(FileMode::FamilyPerFile, id(P), &[slot(P, "00"), slot(2, "01")], true).is_ok());
    }

    #[test]
    fn default_codes() {
        assert_eq!(default_dependant_code(id(P), id(P), true, &[]).unwrap(), "00");
        assert!(default_dependant_code(id(2), id(P), true, &[]).is_err());
        let taken = [slot(P, "00"), slot(2, "01")];
        assert_eq!(default_dependant_code(id(3), id(P), false, &taken).unwrap(), "02");
    }
}

/// Exhaustive file-mode matrix. Mirrors app.validate_account() in
/// migrations/0002 — if you change one, change the other.
#[cfg(test)]
mod rule_matrix {
    use super::*;

    const ALL_MODES: [FileMode; 3] = [FileMode::PatientPerFile, FileMode::FamilyPerFile, FileMode::DebtorPerVisit];
    const PRINCIPAL: u128 = 1;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn slots(spec: &[(u128, &str)]) -> Vec<Slot> {
        spec.iter().map(|(p, c)| Slot { person_id: id(*p), dependant_code: (*c).into() }).collect()
    }

    fn ok(mode: FileMode, spec: &[(u128, &str)], adding: bool) -> bool {
        check_file_rules(mode, id(PRINCIPAL), &slots(spec), adding).is_ok()
    }

    #[test]
    fn empty_file_is_never_valid() {
        for m in ALL_MODES {
            assert!(!ok(m, &[], false), "{m:?}");
        }
    }

    #[test]
    fn principal_alone_is_valid_in_every_mode() {
        for m in ALL_MODES {
            assert!(ok(m, &[(PRINCIPAL, "00")], false), "{m:?}");
        }
    }

    #[test]
    fn principal_and_dependants() {
        let family = [(PRINCIPAL, "00"), (2, "01"), (3, "02"), (4, "07")];
        assert!(ok(FileMode::FamilyPerFile, &family, false));
        assert!(!ok(FileMode::PatientPerFile, &family, false));
        assert!(!ok(FileMode::DebtorPerVisit, &family, false));
    }

    #[test]
    fn lone_dependant_is_valid_only_in_single_patient_modes() {
        assert!(ok(FileMode::PatientPerFile, &[(2, "01")], false));
        assert!(ok(FileMode::DebtorPerVisit, &[(2, "01")], false));
        assert!(!ok(FileMode::FamilyPerFile, &[(2, "01")], false));
    }

    #[test]
    fn code_00_belongs_to_the_principal_in_every_mode() {
        for m in ALL_MODES {
            assert!(!ok(m, &[(2, "00")], false), "non-principal as 00 in {m:?}");
            assert!(!ok(m, &[(PRINCIPAL, "03")], false), "principal not 00 in {m:?}");
        }
    }

    #[test]
    fn adding_to_existing_accounts() {
        assert!(ok(FileMode::FamilyPerFile, &[(PRINCIPAL, "00"), (2, "01")], true));
        assert!(!ok(FileMode::PatientPerFile, &[(PRINCIPAL, "00"), (2, "01")], true), "second patient");
        assert!(!ok(FileMode::DebtorPerVisit, &[(PRINCIPAL, "00")], true), "never add to a visit account");
    }

    #[test]
    fn edits_on_existing_debtor_accounts_are_allowed() {
        // Renumbering a patient isn't "adding".
        assert!(ok(FileMode::DebtorPerVisit, &[(2, "04")], false));
    }

    #[test]
    fn duplicates_rejected_in_every_mode() {
        for m in ALL_MODES {
            assert!(!ok(m, &[(2, "01"), (2, "02")], false), "{m:?}");
            assert!(!ok(m, &[(2, "01"), (3, "01")], false), "{m:?}");
        }
    }

    #[test]
    fn error_kinds_guide_the_client() {
        let conflict = check_file_rules(FileMode::PatientPerFile, id(PRINCIPAL), &slots(&[(PRINCIPAL, "00"), (2, "01")]), false);
        assert!(matches!(conflict, Err(AppError::Conflict(ref m)) if m.contains("patient per file")));
        let visit = check_file_rules(FileMode::DebtorPerVisit, id(PRINCIPAL), &slots(&[(2, "01")]), true);
        assert!(matches!(visit, Err(AppError::Conflict(ref m)) if m.contains("new account for each visit")));
        let family = check_file_rules(FileMode::FamilyPerFile, id(PRINCIPAL), &slots(&[(2, "01")]), false);
        assert!(matches!(family, Err(AppError::Validation(ref m)) if m.contains("main member")));
    }

    #[test]
    fn default_codes_fill_gaps_and_run_out() {
        let taken = slots(&[(PRINCIPAL, "00"), (2, "02"), (3, "03")]);
        assert_eq!(default_dependant_code(id(9), id(PRINCIPAL), false, &taken).unwrap(), "01");
        let full: Vec<Slot> = (0..=99).map(|n| Slot { person_id: id(100 + n), dependant_code: format!("{n:02}") }).collect();
        assert!(default_dependant_code(id(9), id(PRINCIPAL), false, &full).is_err());
    }

    #[test]
    fn principal_defaults_to_00_even_on_medical_aid() {
        assert_eq!(default_dependant_code(id(PRINCIPAL), id(PRINCIPAL), true, &[]).unwrap(), MAIN_MEMBER);
    }

    #[test]
    fn principal_is_main_member_or_guarantor() {
        let row = |main: Option<Uuid>| repo::AccountRow {
            id: id(50),
            account_no: 1,
            file_mode: "patient_per_file".into(),
            status: "open".into(),
            guarantor_person_id: id(2),
            guarantor_first_name: String::new(),
            guarantor_last_name: String::new(),
            membership_id: main.map(|_| id(60)),
            scheme_name: None,
            plan_name: None,
            membership_number: None,
            main_member_person_id: main,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(row(None).principal(), id(2), "private: guarantor");
        assert_eq!(row(Some(id(3))).principal(), id(3), "medical aid: main member");
    }
}
