use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    common::{
        error::{AppError, AppResult},
        validation,
    },
    modules::people::persons::dto::{NewPerson, PersonInput},
};

pub const MAX_PATIENTS_PER_REQUEST: usize = 20;

#[derive(Serialize, sqlx::FromRow)]
pub struct PatientResponse {
    pub id: Uuid,
    pub person_id: Uuid,
    pub account_id: Uuid,
    pub account_no: i64,
    pub dependant_code: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct UpdatePatientRequest {
    pub dependant_code: String,
}

// -----------------------------------------------------------------------------
// POST /patient-files
//
// {
//   "patients":  [ { "person": {"new": {...}} | {"existing": "<uuid>"},
//                    "dependant_code": "01" (optional) } ],
//   "guarantor": {"patient": 0} | {"existing": "<uuid>"} | {"new": {...}},
//   "medical_aid": null
//               | {"existing": "<membership uuid>"}
//               | {"new": { "scheme_id", "plan_id", "membership_number",
//                           "main_member": {"patient": 0} | "guarantor"
//                                        | {"existing": ..} | {"new": {..}} }}
// }
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreatePatientFileRequest {
    pub patients: Vec<FilePatientInput>,
    pub guarantor: PartyInput,
    #[serde(default)]
    pub medical_aid: Option<MedicalAidInput>,
}

#[derive(Deserialize)]
pub struct FilePatientInput {
    pub person: PersonSourceInput,
    #[serde(default)]
    pub dependant_code: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonSourceInput {
    Existing(Uuid),
    New(Box<PersonInput>),
}

/// Someone playing a role on the file (guarantor, main member).
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyInput {
    /// Index into `patients`.
    Patient(usize),
    /// Same person as the guarantor (main member only).
    Guarantor,
    Existing(Uuid),
    New(Box<PersonInput>),
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MedicalAidInput {
    Existing(Uuid),
    New(NewMedicalAidInput),
}

#[derive(Deserialize)]
pub struct NewMedicalAidInput {
    pub scheme_id: Uuid,
    pub plan_id: Uuid,
    pub membership_number: String,
    pub main_member: PartyInput,
}

// --- validated form ----------------------------------------------------------

pub enum PersonSource {
    Existing(Uuid),
    New(Box<NewPerson>),
}

pub enum Party {
    Patient(usize),
    Guarantor,
    Existing(Uuid),
    New(Box<NewPerson>),
}

pub struct FilePatient {
    pub person: PersonSource,
    pub dependant_code: Option<String>,
}

pub enum MedicalAidSource {
    Existing(Uuid),
    New(Box<NewMembershipSource>),
}

pub struct NewMembershipSource {
    pub scheme_id: Uuid,
    pub plan_id: Uuid,
    pub membership_number: String,
    pub main_member: Party,
}

pub struct PatientFile {
    pub patients: Vec<FilePatient>,
    pub guarantor: Party,
    pub medical_aid: Option<MedicalAidSource>,
}

impl CreatePatientFileRequest {
    pub fn validate(self, today: NaiveDate) -> AppResult<PatientFile> {
        let n = self.patients.len();
        if n == 0 {
            return Err(AppError::validation("patients must contain at least one patient"));
        }
        if n > MAX_PATIENTS_PER_REQUEST {
            return Err(AppError::validation(format!(
                "patients can contain at most {MAX_PATIENTS_PER_REQUEST} patients"
            )));
        }

        let patients = self
            .patients
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                let at = |e| prefixed(&format!("patients[{i}]"), e);
                Ok(FilePatient {
                    person: match p.person {
                        PersonSourceInput::Existing(id) => PersonSource::Existing(id),
                        PersonSourceInput::New(input) => PersonSource::New(Box::new(input.validate(today).map_err(at)?)),
                    },
                    dependant_code: p
                        .dependant_code
                        .as_deref()
                        .map(validation::dependant_code)
                        .transpose()
                        .map_err(at)?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;

        let guarantor = party(self.guarantor, "guarantor", n, false, today)?;
        let medical_aid = match self.medical_aid {
            None => None,
            Some(MedicalAidInput::Existing(id)) => Some(MedicalAidSource::Existing(id)),
            Some(MedicalAidInput::New(m)) => Some(MedicalAidSource::New(Box::new(NewMembershipSource {
                scheme_id: m.scheme_id,
                plan_id: m.plan_id,
                membership_number: validation::membership_number(&m.membership_number)
                    .map_err(|e| prefixed("medical_aid", e))?,
                main_member: party(m.main_member, "medical_aid.main_member", n, true, today)?,
            }))),
        };

        Ok(PatientFile { patients, guarantor, medical_aid })
    }
}

fn party(input: PartyInput, field: &str, patients: usize, allow_guarantor: bool, today: NaiveDate) -> AppResult<Party> {
    Ok(match input {
        PartyInput::Patient(i) if i < patients => Party::Patient(i),
        PartyInput::Patient(_) => {
            return Err(AppError::validation(format!("{field} refers to a patient that isn't in the request")));
        }
        PartyInput::Guarantor if allow_guarantor => Party::Guarantor,
        PartyInput::Guarantor => return Err(AppError::validation(format!("{field} can't be \"guarantor\""))),
        PartyInput::Existing(id) => Party::Existing(id),
        PartyInput::New(p) => Party::New(Box::new(p.validate(today).map_err(|e| prefixed(field, e))?)),
    })
}

/// Prefix a validation message with where it came from.
fn prefixed(path: &str, err: AppError) -> AppError {
    match err {
        AppError::Validation(msg) => AppError::Validation(format!("{path}: {msg}")),
        other => other,
    }
}
