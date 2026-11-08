use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::{AppError, AppResult},
    validation,
};

pub const DEFAULT_CODE_SET: &str = "CCSA";

#[derive(Serialize, sqlx::FromRow)]
pub struct Icd10Code {
    pub code: String,
    pub description: String,
    pub valid_as_primary: bool,
    /// Only for this sex ("female" | "male"), if limited.
    pub sex: Option<String>,
    pub min_age_years: Option<i16>,
    pub max_age_years: Option<i16>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TariffCode {
    pub id: Uuid,
    pub code_set: String,
    pub code: String,
    pub description: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct NappiCode {
    pub code: String,
    pub description: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Modifier {
    pub code: String,
    pub description: String,
}

/// `?q=J06&limit=20` — code prefix or words in the description. Codes are
/// not personal data, so a query string is fine here.
#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub code_set: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

pub struct Search {
    /// Upper-cased, LIKE-escaped code prefix.
    pub code_prefix: Option<String>,
    /// LIKE-escaped description fragment.
    pub text: Option<String>,
    pub code_set: String,
    pub limit: i64,
}

impl SearchQuery {
    pub fn validate(self) -> AppResult<Search> {
        let q = self.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
        if q.is_some_and(|q| q.chars().count() > 100) {
            return Err(AppError::validation("q is too long"));
        }
        let code_set = self.code_set.unwrap_or_else(|| DEFAULT_CODE_SET.into()).to_uppercase();
        Ok(Search {
            code_prefix: q.map(|q| validation::like_escape(&q.to_uppercase())),
            text: q.map(validation::like_escape),
            code_set,
            limit: self.limit.unwrap_or(25).clamp(1, 100),
        })
    }
}

/// `?code=0190&code_set=CCSA&plan_id=…&date=2026-11-07`: the rate for a
/// tariff code on a medical aid plan (omit plan_id for private) in the
/// year of the service date.
#[derive(Deserialize)]
pub struct TariffPriceQuery {
    pub code: String,
    #[serde(default)]
    pub code_set: Option<String>,
    #[serde(default)]
    pub plan_id: Option<Uuid>,
    pub date: NaiveDate,
}

pub struct TariffPriceLookup {
    pub code_set: String,
    pub code: String,
    pub plan_id: Option<Uuid>,
    pub year: i16,
}

impl TariffPriceQuery {
    pub fn validate(self) -> AppResult<TariffPriceLookup> {
        let code = self.code.trim().to_uppercase();
        if !(4..=6).contains(&code.len()) || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(AppError::validation("code must be 4-6 letters or digits"));
        }
        let year = i16::try_from(self.date.year()).map_err(|_| AppError::validation("date is out of range"))?;
        Ok(TariffPriceLookup {
            code_set: self.code_set.unwrap_or_else(|| DEFAULT_CODE_SET.into()).trim().to_uppercase(),
            code,
            plan_id: self.plan_id,
            year,
        })
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct TariffPrice {
    pub code_set: String,
    pub code: String,
    pub year: i16,
    pub price_cents: i64,
    /// "plan": the plan's own rate; "default": the general rate (private
    /// patients, or a plan without its own rate for this code).
    pub source: &'static str,
}

/// Normalise an ICD-10 code as typed: upper case, no spaces, dot after
/// the category ("j069" → "J06.9").
pub fn normalise_icd10(code: &str) -> String {
    let c: String = code.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    if c.is_ascii() && c.len() > 3 && !c.contains('.') {
        format!("{}.{}", &c[..3], &c[3..])
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_defaults_and_bounds() {
        let s = SearchQuery { q: None, code_set: None, limit: None }.validate().unwrap();
        assert!(s.code_prefix.is_none() && s.text.is_none());
        assert_eq!((s.code_set.as_str(), s.limit), ("CCSA", 25));
        let s = SearchQuery { q: Some("  ".into()), code_set: Some("nhrpl".into()), limit: Some(1000) }.validate().unwrap();
        assert!(s.text.is_none());
        assert_eq!((s.code_set.as_str(), s.limit), ("NHRPL", 100));
        assert!(SearchQuery { q: Some("x".repeat(101)), code_set: None, limit: None }.validate().is_err());
    }

    #[test]
    fn search_escapes_input() {
        let s = SearchQuery { q: Some("j0_%".into()), code_set: None, limit: None }.validate().unwrap();
        assert_eq!(s.code_prefix.as_deref(), Some("J0\\_\\%"));
        assert_eq!(s.text.as_deref(), Some("j0\\_\\%"));
    }

    #[test]
    fn tariff_price_query() {
        let q = |code: &str| TariffPriceQuery {
            code: code.into(),
            code_set: Some(" ccsa ".into()),
            plan_id: None,
            date: NaiveDate::from_ymd_opt(2026, 11, 7).unwrap(),
        }
        .validate();
        let l = q(" 0190 ").unwrap();
        assert_eq!((l.code.as_str(), l.code_set.as_str(), l.year), ("0190", "CCSA", 2026));
        assert!(q("01").is_err());
        assert!(q("01-90").is_err());
    }

    #[test]
    fn icd10_normalisation() {
        assert_eq!(normalise_icd10("j069"), "J06.9");
        assert_eq!(normalise_icd10(" J06.9 "), "J06.9");
        assert_eq!(normalise_icd10("z00"), "Z00");
        assert_eq!(normalise_icd10("s5250"), "S52.50");
        assert_eq!(normalise_icd10("ÉÉÉÉ"), "ÉÉÉÉ", "non-ASCII is left alone (and rejected later), never panics");
    }
}
