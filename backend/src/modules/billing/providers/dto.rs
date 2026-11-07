use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::{AppError, AppResult},
    validation,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// The practice's own treating doctors.
    Practice,
    /// Referring doctors, surgical assistants and other outside doctors.
    External,
}

impl ProviderKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Practice => "practice",
            Self::External => "external",
        }
    }
}

fn yes() -> bool {
    true
}

/// Create and full replace (PUT).
#[derive(Deserialize)]
pub struct ProviderInput {
    pub kind: ProviderKind,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub first_name: Option<String>,
    pub last_name: String,
    #[serde(default)]
    pub practice_number: Option<String>,
    #[serde(default)]
    pub hpcsa_number: Option<String>,
    #[serde(default)]
    pub speciality: Option<String>,
    /// Link to the practice member this provider signs in as.
    #[serde(default)]
    pub user_id: Option<Uuid>,
    #[serde(default = "yes")]
    pub is_active: bool,
}

pub struct NewProvider {
    pub kind: ProviderKind,
    pub title: Option<String>,
    pub first_name: Option<String>,
    pub last_name: String,
    pub practice_number: Option<String>,
    pub hpcsa_number: Option<String>,
    pub speciality: Option<String>,
    pub user_id: Option<Uuid>,
    pub is_active: bool,
}

impl ProviderInput {
    pub fn validate(self) -> AppResult<NewProvider> {
        if self.kind == ProviderKind::External && self.user_id.is_some() {
            return Err(AppError::validation("Only the practice's own providers can be linked to a user"));
        }
        let practice_number = validation::optional_text("practice_number", self.practice_number.as_deref(), 20)?
            .map(|n| validation::pcns_number(&n))
            .transpose()
            .map_err(|_| AppError::validation("practice_number must be a 7-digit BHF practice number"))?;
        let hpcsa_number = validation::optional_text("hpcsa_number", self.hpcsa_number.as_deref(), 20)?
            .map(|n| hpcsa_number(&n))
            .transpose()?;
        if self.kind == ProviderKind::External && practice_number.is_none() && hpcsa_number.is_none() {
            return Err(AppError::validation(
                "An external provider needs a practice number or HPCSA number so they can be identified on claims",
            ));
        }
        Ok(NewProvider {
            kind: self.kind,
            title: validation::optional_text("title", self.title.as_deref(), 20)?,
            first_name: validation::optional_text("first_name", self.first_name.as_deref(), 100)?,
            last_name: validation::text("last_name", &self.last_name, 100)?,
            practice_number,
            hpcsa_number,
            speciality: validation::optional_text("speciality", self.speciality.as_deref(), 100)?,
            user_id: self.user_id,
            is_active: self.is_active,
        })
    }
}

/// HPCSA registration number, e.g. "MP 0123456" → "MP0123456".
pub fn hpcsa_number(value: &str) -> AppResult<String> {
    let v: String = value.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    let letters = v.chars().take_while(|c| c.is_ascii_uppercase()).count();
    let digits = v.len() - letters;
    let ok = (2..=4).contains(&letters) && (4..=8).contains(&digits) && v[letters..].chars().all(|c| c.is_ascii_digit());
    if !ok {
        return Err(AppError::validation("hpcsa_number must look like MP0123456"));
    }
    Ok(v)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ProviderResponse {
    pub id: Uuid,
    pub kind: String,
    pub title: Option<String>,
    pub first_name: Option<String>,
    pub last_name: String,
    pub practice_number: Option<String>,
    pub hpcsa_number: Option<String>,
    pub speciality: Option<String>,
    pub user_id: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ProviderResponse {
    /// "Dr A Nkosi" style display name for invoices and statements.
    pub fn display_name(&self) -> String {
        display_name(self.title.as_deref(), self.first_name.as_deref(), &self.last_name)
    }
}

pub fn display_name(title: Option<&str>, first_name: Option<&str>, last_name: &str) -> String {
    let initial = first_name.and_then(|f| f.chars().next()).map(|c| c.to_uppercase().to_string());
    [title.map(str::to_owned), initial, Some(last_name.to_owned())]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Deserialize)]
pub struct ProviderFilter {
    #[serde(default)]
    pub kind: Option<ProviderKind>,
    #[serde(default)]
    pub include_inactive: bool,
}

// --- service centres -----------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CentreKind {
    Rooms,
    Hospital,
    DayClinic,
    Other,
}

impl CentreKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rooms => "rooms",
            Self::Hospital => "hospital",
            Self::DayClinic => "day_clinic",
            Self::Other => "other",
        }
    }

    /// Usual place-of-service code (11 office, 21 inpatient hospital,
    /// 24 ambulatory surgical centre).
    pub const fn default_place_of_service(self) -> Option<&'static str> {
        match self {
            Self::Rooms => Some("11"),
            Self::Hospital => Some("21"),
            Self::DayClinic => Some("24"),
            Self::Other => None,
        }
    }
}

#[derive(Deserialize)]
pub struct ServiceCentreInput {
    pub kind: CentreKind,
    pub name: String,
    #[serde(default)]
    pub practice_number: Option<String>,
    #[serde(default)]
    pub default_place_of_service: Option<String>,
    #[serde(default = "yes")]
    pub is_active: bool,
}

pub struct NewServiceCentre {
    pub kind: CentreKind,
    pub name: String,
    pub practice_number: Option<String>,
    pub default_place_of_service: Option<String>,
    pub is_active: bool,
}

impl ServiceCentreInput {
    pub fn validate(self) -> AppResult<NewServiceCentre> {
        let pos = match validation::optional_text("default_place_of_service", self.default_place_of_service.as_deref(), 2)? {
            Some(p) => Some(place_of_service(&p)?),
            None => self.kind.default_place_of_service().map(str::to_owned),
        };
        Ok(NewServiceCentre {
            kind: self.kind,
            name: validation::text("name", &self.name, 120)?,
            practice_number: validation::optional_text("practice_number", self.practice_number.as_deref(), 20)?
                .map(|n| validation::pcns_number(&n))
                .transpose()?,
            default_place_of_service: pos,
            is_active: self.is_active,
        })
    }
}

pub fn place_of_service(value: &str) -> AppResult<String> {
    let v = value.trim();
    if v.len() != 2 || !v.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::validation("place_of_service must be a 2-digit code such as 11 or 21"));
    }
    Ok(v.to_owned())
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ServiceCentreResponse {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub practice_number: Option<String>,
    pub default_place_of_service: Option<String>,
    pub is_active: bool,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn provider(v: serde_json::Value) -> AppResult<NewProvider> {
        serde_json::from_value::<ProviderInput>(v).unwrap().validate()
    }

    #[test]
    fn practice_provider_minimal() {
        let p = provider(json!({"kind": "practice", "last_name": " Nkosi ", "first_name": "Ayanda", "title": "Dr"})).unwrap();
        assert_eq!(p.last_name, "Nkosi");
        assert!(p.is_active, "active by default");
        assert!(p.practice_number.is_none());
    }

    #[test]
    fn numbers_are_normalised() {
        let p = provider(json!({"kind": "external", "last_name": "X", "practice_number": "012 3456", "hpcsa_number": "mp 0123456"})).unwrap();
        assert_eq!(p.practice_number.as_deref(), Some("0123456"));
        assert_eq!(p.hpcsa_number.as_deref(), Some("MP0123456"));
    }

    #[test]
    fn external_needs_an_identifier_and_no_user() {
        assert!(provider(json!({"kind": "external", "last_name": "X"})).is_err());
        assert!(provider(json!({"kind": "external", "last_name": "X", "hpcsa_number": "MP0123456", "user_id": Uuid::nil()})).is_err());
        assert!(provider(json!({"kind": "practice", "last_name": "X", "user_id": Uuid::nil()})).is_ok());
    }

    #[test]
    fn hpcsa_formats() {
        for ok in ["MP0123456", "DP 12345", "psy1234", "ABCD12345678"] {
            assert!(hpcsa_number(ok).is_ok(), "{ok}");
        }
        for bad in ["0123456", "M0123456", "ABCDE1234", "MP123", "MP0123456789", "MP01234X6", "MP-0123456"] {
            assert!(hpcsa_number(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn bad_practice_number_is_rejected() {
        assert!(provider(json!({"kind": "practice", "last_name": "X", "practice_number": "12345"})).is_err());
    }

    #[test]
    fn display_names() {
        assert_eq!(display_name(Some("Dr"), Some("ayanda"), "Nkosi"), "Dr A Nkosi");
        assert_eq!(display_name(None, None, "Nkosi"), "Nkosi");
        assert_eq!(display_name(Some("Prof"), Some(""), "Mokoena"), "Prof Mokoena");
    }

    #[test]
    fn centre_defaults_place_of_service_by_kind() {
        let c = |v| serde_json::from_value::<ServiceCentreInput>(v).unwrap().validate().unwrap();
        assert_eq!(c(json!({"kind": "hospital", "name": "Sunninghill"})).default_place_of_service.as_deref(), Some("21"));
        assert_eq!(c(json!({"kind": "rooms", "name": "Rooms 2"})).default_place_of_service.as_deref(), Some("11"));
        assert_eq!(c(json!({"kind": "day_clinic", "name": "DC"})).default_place_of_service.as_deref(), Some("24"));
        assert_eq!(c(json!({"kind": "other", "name": "Home"})).default_place_of_service, None);
        assert_eq!(c(json!({"kind": "hospital", "name": "H", "default_place_of_service": "22"})).default_place_of_service.as_deref(), Some("22"));
    }

    #[test]
    fn centre_rejects_bad_codes() {
        let c = |v| serde_json::from_value::<ServiceCentreInput>(v).unwrap().validate();
        assert!(c(json!({"kind": "hospital", "name": "H", "default_place_of_service": "2"})).is_err());
        assert!(c(json!({"kind": "hospital", "name": "", })).is_err());
        assert!(c(json!({"kind": "hospital", "name": "H", "practice_number": "abc"})).is_err());
    }

    #[test]
    fn place_of_service_codes() {
        assert_eq!(place_of_service(" 11 ").unwrap(), "11");
        assert!(place_of_service("1a").is_err());
        assert!(place_of_service("111").is_err());
    }
}
