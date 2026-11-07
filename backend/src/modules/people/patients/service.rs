use chrono::Utc;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    dto::{
        CreatePatientFileRequest, MainMemberRef, MedicalAidSource, Party, PatientListItem, PatientPage,
        PatientResponse, PatientSearchRequest, PersonSource, SetStatusRequest, UpdatePatientRequest,
        mask_id_number,
    },
    repository as repo,
};
use crate::{
    common::{
        auth::TenantUser,
        db,
        error::{AppError, AppResult, on_unique},
        validation,
        ws::EventBus,
    },
    modules::{
        access::tenants::dto::FileMode,
        people::{
            accounts::{
                self,
                dto::AccountResponse,
                service::{Slot, check_file_rules, default_dependant_code},
            },
            memberships::{self, dto::NewMembership},
            persons,
        },
    },
};

const PATIENT_UNIQUES: &[(&str, &str)] = &[
    ("patients_dependant_code_uq", "Another patient on this account already has that dependant code"),
    ("patients_person_uq", "This person is already a patient on this account"),
];

#[derive(Clone)]
pub struct PatientsService {
    pool: PgPool,
    bus: EventBus,
}

impl PatientsService {
    pub fn new(pool: PgPool, bus: EventBus) -> Self {
        Self { pool, bus }
    }

    /// The patient list: one search box, paged on the server, ID numbers masked.
    pub async fn search(&self, actor: &TenantUser, req: PatientSearchRequest) -> AppResult<PatientPage> {
        let s = req.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let total = repo::search_count(&mut tx, actor.tenant_id, &s).await?;
        let rows = if total == 0 { Vec::new() } else { repo::search_page(&mut tx, actor.tenant_id, &s).await? };
        tx.commit().await?;
        Ok(PatientPage {
            items: rows
                .into_iter()
                .map(|r| PatientListItem {
                    id_number_masked: r.id_number.as_deref().map(|n| mask_id_number(r.id_type.as_deref(), n)),
                    main_member: match (r.main_member_person_id, r.main_member_first_name, r.main_member_last_name) {
                        (Some(person_id), Some(first_name), Some(last_name)) => Some(MainMemberRef { person_id, first_name, last_name }),
                        _ => None,
                    },
                    patient_id: r.patient_id,
                    person_id: r.person_id,
                    account_id: r.account_id,
                    account_no: r.account_no,
                    dependant_code: r.dependant_code,
                    status: r.status,
                    title: r.title,
                    first_name: r.first_name,
                    last_name: r.last_name,
                    date_of_birth: r.date_of_birth,
                    scheme_name: r.scheme_name,
                    plan_name: r.plan_name,
                    membership_number: r.membership_number,
                })
                .collect(),
            total,
            page: s.page,
            page_size: s.page_size,
        })
    }

    /// Mark a patient active or inactive. Nothing is removed from the file.
    pub async fn set_status(&self, actor: &TenantUser, id: Uuid, req: SetStatusRequest) -> AppResult<PatientResponse> {
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        if !repo::set_status(&mut tx, t, id, req.status.as_str()).await? {
            return Err(AppError::NotFound("patient"));
        }
        let patient = find(&mut tx, t, id).await?;
        accounts::service::announce_files(&mut tx, &self.bus, t, [patient.account_id]).await?;
        tx.commit().await?;
        Ok(patient)
    }

    pub async fn get_file(&self, actor: &TenantUser, account_id: Uuid) -> AppResult<AccountResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let file = accounts::service::load_account(&mut tx, actor.tenant_id, account_id).await?;
        tx.commit().await?;
        Ok(file)
    }

    pub async fn get(&self, actor: &TenantUser, id: Uuid) -> AppResult<PatientResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let patient = find(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(patient)
    }

    pub async fn update(&self, actor: &TenantUser, id: Uuid, req: UpdatePatientRequest) -> AppResult<PatientResponse> {
        let code = validation::dependant_code(&req.dependant_code)?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;

        let patient = find(&mut tx, t, id).await?;
        let account = accounts::service::find_locked(&mut tx, t, patient.account_id).await?;
        let slots: Vec<Slot> = accounts::service::current_slots(&mut tx, t, patient.account_id)
            .await?
            .into_iter()
            .map(|mut s| {
                if s.person_id == patient.person_id {
                    s.dependant_code = code.clone();
                }
                s
            })
            .collect();
        check_file_rules(FileMode::from_db(&account.file_mode)?, account.principal(), &slots, false)?;

        repo::update_code(&mut tx, t, id, &code).await.map_err(on_unique(PATIENT_UNIQUES))?;
        accounts::service::announce_files(&mut tx, &self.bus, t, [patient.account_id]).await?;
        let patient = find(&mut tx, t, id).await?;
        tx.commit().await?;
        Ok(patient)
    }

    /// Open a new patient file in one transaction: create or reuse the
    /// people involved, the medical aid membership if any, a new account
    /// (stamped with the practice's file mode) and its patients.
    ///
    /// Under new-debtor-per-visit this is also how a returning patient's
    /// visit gets its new account: pass `{"existing": person_id}`.
    pub async fn create_file(&self, actor: &TenantUser, req: CreatePatientFileRequest) -> AppResult<AccountResponse> {
        let file = req.validate(Utc::now().date_naive())?;
        let t = actor.tenant_id;
        let by = actor.user_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;

        // 1. People
        let mut patient_ids = Vec::with_capacity(file.patients.len());
        for p in &file.patients {
            let id = match &p.person {
                PersonSource::Existing(id) => {
                    persons::service::ensure_exists(&mut tx, t, *id).await?;
                    *id
                }
                PersonSource::New(new) => persons::service::insert_person(&mut tx, t, new, by).await?.id,
            };
            patient_ids.push(id);
        }
        let guarantor = resolve_party(&mut tx, t, by, &file.guarantor, &patient_ids, None).await?;

        // 2. Medical aid
        let (membership_id, principal) = match &file.medical_aid {
            None => (None, guarantor),
            Some(MedicalAidSource::Existing(id)) => {
                let m = memberships::service::load(&mut tx, t, *id).await?;
                (Some(m.id), m.main_member_person_id)
            }
            Some(MedicalAidSource::New(m)) => {
                let main = resolve_party(&mut tx, t, by, &m.main_member, &patient_ids, Some(guarantor)).await?;
                let new = NewMembership {
                    scheme_id: m.scheme_id,
                    plan_id: m.plan_id,
                    membership_number: m.membership_number.clone(),
                    main_member_person_id: main,
                };
                (Some(memberships::service::create_membership(&mut tx, t, &new, by).await?), main)
            }
        };

        // 3. Account — the database stamps the practice's current file mode.
        let account_no = accounts::repository::next_account_no(&mut tx, t).await?;
        let (account_id, mode) =
            accounts::repository::insert(&mut tx, t, account_no, guarantor, membership_id, by).await?;
        let mode = FileMode::from_db(&mode)?;

        // 4. Patients, checked against the file-mode rules first.
        let mut slots: Vec<Slot> = Vec::with_capacity(patient_ids.len());
        for (p, person_id) in file.patients.iter().zip(&patient_ids) {
            let code = match &p.dependant_code {
                Some(c) => c.clone(),
                None => default_dependant_code(*person_id, principal, membership_id.is_some(), &slots)?,
            };
            slots.push(Slot { person_id: *person_id, dependant_code: code });
        }
        check_file_rules(mode, principal, &slots, false)?;
        for s in &slots {
            repo::insert(&mut tx, t, s.person_id, account_id, &s.dependant_code, by)
                .await
                .map_err(on_unique(PATIENT_UNIQUES))?;
        }

        accounts::service::announce_files(&mut tx, &self.bus, t, [account_id]).await?;
        let account = accounts::service::load_account(&mut tx, t, account_id).await?;
        // Deferred constraint triggers re-validate the whole file here.
        tx.commit().await?;
        tracing::info!(account_id = %account_id, patients = slots.len(), mode = mode.as_str(), "patient file opened");
        Ok(account)
    }
}

async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<PatientResponse> {
    repo::find(conn, tenant_id, id).await?.ok_or(AppError::NotFound("patient"))
}

async fn resolve_party(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    by: Uuid,
    party: &Party,
    patient_ids: &[Uuid],
    guarantor: Option<Uuid>,
) -> AppResult<Uuid> {
    match party {
        Party::Patient(i) => Ok(patient_ids[*i]),
        Party::Guarantor => guarantor.ok_or_else(|| AppError::validation("guarantor is not known yet")),
        Party::Existing(id) => {
            persons::service::ensure_exists(conn, tenant_id, *id).await?;
            Ok(*id)
        }
        Party::New(new) => Ok(persons::service::insert_person(conn, tenant_id, new, by).await?.id),
    }
}
