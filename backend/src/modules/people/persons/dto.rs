//! No `Debug` derives on anything holding personal information.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::{AppError, AppResult},
    validation,
};

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sex {
    Female,
    Male,
    Other,
    Unknown,
}

impl Sex {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Female => "female",
            Self::Male => "male",
            Self::Other => "other",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdType {
    SaId,
    Passport,
    Other,
}

impl IdType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SaId => "sa_id",
            Self::Passport => "passport",
            Self::Other => "other",
        }
    }
}

/// Demographics as submitted, for create and full replace (PUT).
#[derive(Deserialize)]
pub struct PersonInput {
    #[serde(default)]
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub preferred_name: Option<String>,
    #[serde(default)]
    pub date_of_birth: Option<NaiveDate>,
    #[serde(default)]
    pub sex: Option<Sex>,
    /// Defaults to `sa_id` when `id_number` is given without a type.
    #[serde(default)]
    pub id_type: Option<IdType>,
    #[serde(default)]
    pub id_number: Option<String>,
    #[serde(default)]
    pub id_country: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub mobile: Option<String>,
    #[serde(default)]
    pub home_phone: Option<String>,
    #[serde(default)]
    pub address_line1: Option<String>,
    #[serde(default)]
    pub address_line2: Option<String>,
    #[serde(default)]
    pub suburb: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub postal_code: Option<String>,
}

/// Validated, normalised person ready for the database.
pub struct NewPerson {
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub sex: Option<&'static str>,
    pub id_type: Option<&'static str>,
    pub id_number: Option<String>,
    pub id_country: Option<String>,
    pub email: Option<String>,
    pub mobile: Option<String>,
    pub home_phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub suburb: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
}

impl PersonInput {
    pub fn validate(self, today: NaiveDate) -> AppResult<NewPerson> {
        let opt = |field: &str, v: &Option<String>, max: usize| {
            validation::optional_text(field, v.as_deref(), max)
        };

        let mut date_of_birth = self.date_of_birth;
        if let Some(dob) = date_of_birth
            && dob > today
        {
            return Err(AppError::validation("date_of_birth cannot be in the future"));
        }

        let id_number = opt("id_number", &self.id_number, 30)?;
        let id_type = match (self.id_type, &id_number) {
            (None, None) => None,
            (Some(_), None) => return Err(AppError::validation("id_number is required when id_type is set")),
            (None, Some(_)) => Some(IdType::SaId),
            (Some(t), Some(_)) => Some(t),
        };
        let mut id_country = opt("id_country", &self.id_country, 2)?.map(|c| c.to_uppercase());

        let id_number = match (id_type, id_number) {
            (Some(IdType::SaId), Some(n)) => {
                let (n, info) = validation::sa_id_number(&n, today)?;
                match date_of_birth {
                    None => date_of_birth = Some(info.date_of_birth),
                    Some(d) if d != info.date_of_birth => {
                        return Err(AppError::validation(
                            "date_of_birth does not match the SA ID number",
                        ));
                    }
                    Some(_) => {}
                }
                id_country = id_country.or(Some("ZA".into()));
                Some(n)
            }
            (Some(IdType::Passport), Some(n)) => {
                let n: String = n.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
                if !(5..=20).contains(&n.len()) || !n.chars().all(|c| c.is_ascii_alphanumeric()) {
                    return Err(AppError::validation("passport number must be 5-20 letters or digits"));
                }
                if id_country.is_none() {
                    return Err(AppError::validation("id_country is required for passports"));
                }
                Some(n)
            }
            (_, n) => n,
        };
        if let Some(c) = &id_country
            && (c.len() != 2 || !c.chars().all(|ch| ch.is_ascii_uppercase()))
        {
            return Err(AppError::validation("id_country must be a 2-letter country code"));
        }

        Ok(NewPerson {
            title: opt("title", &self.title, 20)?,
            first_name: validation::text("first_name", &self.first_name, 100)?,
            last_name: validation::text("last_name", &self.last_name, 100)?,
            preferred_name: opt("preferred_name", &self.preferred_name, 100)?,
            date_of_birth,
            sex: self.sex.map(Sex::as_str),
            id_type: id_type.map(IdType::as_str),
            id_number,
            id_country,
            email: opt("email", &self.email, 254)?.map(|e| validation::email(&e)).transpose()?,
            mobile: opt("mobile", &self.mobile, 30)?.map(|p| validation::phone("mobile", &p)).transpose()?,
            home_phone: opt("home_phone", &self.home_phone, 30)?
                .map(|p| validation::phone("home_phone", &p))
                .transpose()?,
            address_line1: opt("address_line1", &self.address_line1, 200)?,
            address_line2: opt("address_line2", &self.address_line2, 200)?,
            suburb: opt("suburb", &self.suburb, 100)?,
            city: opt("city", &self.city, 100)?,
            postal_code: opt("postal_code", &self.postal_code, 4)?
                .map(|p| validation::postal_code(&p))
                .transpose()?,
        })
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PersonResponse {
    pub id: Uuid,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub sex: Option<String>,
    pub id_type: Option<String>,
    pub id_number: Option<String>,
    pub id_country: Option<String>,
    pub email: Option<String>,
    pub mobile: Option<String>,
    pub home_phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub suburb: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Search result: enough to pick the right person, no ID number or contacts.
#[derive(Serialize, sqlx::FromRow)]
pub struct PersonSummary {
    pub id: Uuid,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
}

#[derive(Deserialize)]
pub struct SearchPersonsRequest {
    /// Surname / first-name prefixes ("smith jo"), an ID or passport number,
    /// or an account number.
    pub query: String,
    #[serde(default)]
    pub limit: Option<i64>,
}

/// Parsed search: each part is optional; any match returns the person.
pub struct SearchTerms {
    pub id_number: Option<String>,
    pub account_no: Option<i64>,
    /// LIKE-escaped, lower-cased name prefixes.
    pub name_a: Option<String>,
    pub name_b: Option<String>,
    pub limit: i64,
}

impl SearchPersonsRequest {
    pub fn validate(self) -> AppResult<SearchTerms> {
        let q = self.query.trim();
        if q.chars().count() < 2 {
            return Err(AppError::validation("query must be at least 2 characters"));
        }
        if q.chars().count() > 100 {
            return Err(AppError::validation("query is too long"));
        }
        let compact: String = q.chars().filter(|c| !c.is_whitespace()).collect();
        let all_digits = compact.chars().all(|c| c.is_ascii_digit());
        let has_letters = q.chars().any(char::is_alphabetic);

        let mut words = q.split_whitespace().filter(|w| w.chars().any(char::is_alphabetic));
        let (name_a, name_b) = if has_letters {
            (words.next().map(like_prefix), words.next().map(like_prefix))
        } else {
            (None, None)
        };

        Ok(SearchTerms {
            id_number: compact
                .chars()
                .all(|c| c.is_ascii_alphanumeric())
                .then(|| compact.to_uppercase()),
            account_no: if all_digits { compact.parse().ok() } else { None },
            name_a,
            name_b,
            limit: self.limit.unwrap_or(20).clamp(1, 50),
        })
    }
}

/// Lower-case and escape LIKE wildcards so user input matches literally.
fn like_prefix(word: &str) -> String {
    let mut out = String::with_capacity(word.len() + 1);
    for c in word.to_lowercase().chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// One file (account) this person is a patient on.
#[derive(Serialize, sqlx::FromRow)]
pub struct PersonFile {
    pub patient_id: Uuid,
    pub account_id: Uuid,
    pub account_no: i64,
    pub dependant_code: String,
    pub account_status: String,
    pub file_mode: String,
    pub scheme_name: Option<String>,
    pub plan_name: Option<String>,
    pub membership_number: Option<String>,
    pub opened_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> PersonInput {
        serde_json::from_value(serde_json::json!({"first_name": "A", "last_name": "B"})).unwrap()
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 7).unwrap()
    }

    #[test]
    fn sa_id_fills_dob_and_country() {
        let mut p = input();
        p.id_number = Some("8001015009087".into());
        let v = p.validate(today()).unwrap();
        assert_eq!(v.id_type, Some("sa_id"));
        assert_eq!(v.date_of_birth, NaiveDate::from_ymd_opt(1980, 1, 1));
        assert_eq!(v.id_country.as_deref(), Some("ZA"));
    }

    #[test]
    fn dob_must_match_sa_id() {
        let mut p = input();
        p.id_number = Some("8001015009087".into());
        p.date_of_birth = NaiveDate::from_ymd_opt(1980, 1, 2);
        assert!(p.validate(today()).is_err());
    }

    #[test]
    fn passport_needs_country() {
        let mut p = input();
        p.id_type = Some(IdType::Passport);
        p.id_number = Some("ab12345".into());
        assert!(p.validate(today()).is_err());
    }

    #[test]
    fn search_parsing() {
        let t = SearchPersonsRequest { query: "Smith jo_".into(), limit: None }.validate().unwrap();
        assert_eq!(t.name_a.as_deref(), Some("smith"));
        assert_eq!(t.name_b.as_deref(), Some("jo\\_"));
        assert_eq!(t.account_no, None);

        let t = SearchPersonsRequest { query: "800101 5009 087".into(), limit: Some(500) }.validate().unwrap();
        assert_eq!(t.id_number.as_deref(), Some("8001015009087"));
        assert!(t.name_a.is_none());
        assert_eq!(t.limit, 50);
    }
}
