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

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::common::test_support::today;

    fn new_person(first: &str) -> Value {
        json!({"new": {"first_name": first, "last_name": "Test"}})
    }

    fn validate(v: Value) -> AppResult<PatientFile> {
        serde_json::from_value::<CreatePatientFileRequest>(v).unwrap().validate(today())
    }

    fn err(v: Value) -> String {
        validate(v).err().expect("expected a validation error").to_string()
    }

    #[test]
    fn minimal_private_file() {
        let f = validate(json!({"patients": [{"person": new_person("A")}], "guarantor": {"patient": 0}})).unwrap();
        assert_eq!(f.patients.len(), 1);
        assert!(matches!(f.patients[0].person, PersonSource::New(_)));
        assert!(f.patients[0].dependant_code.is_none());
        assert!(matches!(f.guarantor, Party::Patient(0)));
        assert!(f.medical_aid.is_none());
    }

    #[test]
    fn wire_format_variants() {
        let id = Uuid::from_u128(7);
        let f = validate(json!({
            "patients": [{"person": {"existing": id}, "dependant_code": "02"}],
            "guarantor": {"existing": id},
            "medical_aid": {"existing": id}
        }))
        .unwrap();
        assert!(matches!(f.patients[0].person, PersonSource::Existing(x) if x == id));
        assert_eq!(f.patients[0].dependant_code.as_deref(), Some("02"));
        assert!(matches!(f.guarantor, Party::Existing(x) if x == id));
        assert!(matches!(f.medical_aid, Some(MedicalAidSource::Existing(x)) if x == id));

        let f = validate(json!({
            "patients": [{"person": new_person("A")}],
            "guarantor": new_person("Payer"),
            "medical_aid": {"new": {"scheme_id": id, "plan_id": id, "membership_number": " m 1 ", "main_member": "guarantor"}}
        }))
        .unwrap();
        assert!(matches!(f.guarantor, Party::New(_)));
        match f.medical_aid {
            Some(MedicalAidSource::New(m)) => {
                assert_eq!(m.membership_number, "M1");
                assert!(matches!(m.main_member, Party::Guarantor));
            }
            _ => panic!("expected new medical aid"),
        }
    }

    #[test]
    fn needs_one_to_twenty_patients() {
        assert!(err(json!({"patients": [], "guarantor": new_person("G")})).contains("at least one"));
        let many: Vec<Value> = (0..=MAX_PATIENTS_PER_REQUEST).map(|i| json!({"person": new_person(&format!("P{i}"))})).collect();
        assert!(err(json!({"patients": many, "guarantor": {"patient": 0}})).contains("at most"));
        let max: Vec<Value> = (0..MAX_PATIENTS_PER_REQUEST).map(|i| json!({"person": new_person(&format!("P{i}"))})).collect();
        assert!(validate(json!({"patients": max, "guarantor": {"patient": 0}})).is_ok());
    }

    #[test]
    fn guarantor_cannot_be_guarantor() {
        assert!(err(json!({"patients": [{"person": new_person("A")}], "guarantor": "guarantor"})).contains("guarantor"));
    }

    #[test]
    fn patient_indexes_must_exist() {
        assert!(err(json!({"patients": [{"person": new_person("A")}], "guarantor": {"patient": 1}})).contains("guarantor"));
        let e = err(json!({
            "patients": [{"person": new_person("A")}], "guarantor": {"patient": 0},
            "medical_aid": {"new": {"scheme_id": Uuid::nil(), "plan_id": Uuid::nil(), "membership_number": "1", "main_member": {"patient": 5}}}
        }));
        assert!(e.contains("medical_aid.main_member"), "{e}");
    }

    #[test]
    fn errors_say_where_they_came_from() {
        let e = err(json!({"patients": [{"person": new_person("A")}, {"person": new_person(" ")}], "guarantor": {"patient": 0}}));
        assert!(e.starts_with("patients[1]: first_name"), "{e}");
        let e = err(json!({"patients": [{"person": new_person("A"), "dependant_code": "1"}], "guarantor": {"patient": 0}}));
        assert!(e.starts_with("patients[0]: dependant_code"), "{e}");
        let e = err(json!({"patients": [{"person": new_person("A")}], "guarantor": {"new": {"first_name": "G", "last_name": "", "mobile": "x"}}}));
        assert!(e.starts_with("guarantor: "), "{e}");
        let e = err(json!({
            "patients": [{"person": new_person("A")}], "guarantor": {"patient": 0},
            "medical_aid": {"new": {"scheme_id": Uuid::nil(), "plan_id": Uuid::nil(), "membership_number": "#", "main_member": "guarantor"}}
        }));
        assert!(e.starts_with("medical_aid: membership_number"), "{e}");
    }

    #[test]
    fn unknown_shapes_are_rejected_by_serde() {
        let parse = |v| serde_json::from_value::<CreatePatientFileRequest>(v).is_err();
        assert!(parse(json!({"patients": [{"person": {"maybe": 1}}], "guarantor": {"patient": 0}})));
        assert!(parse(json!({"patients": [{"person": new_person("A")}], "guarantor": "patient"})));
        assert!(parse(json!({"patients": [{"person": new_person("A")}]})), "guarantor is required");
    }
}
