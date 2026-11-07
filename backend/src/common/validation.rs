//! Small input-validation helpers used by DTOs.

use serde::{Deserialize, Deserializer};

use crate::common::error::{AppError, AppResult};

/// Required, trimmed, length-bounded text.
pub fn text(field: &str, value: &str, max_chars: usize) -> AppResult<String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(AppError::validation(format!("{field} is required")));
    }
    if v.chars().count() > max_chars {
        return Err(AppError::validation(format!(
            "{field} must be at most {max_chars} characters"
        )));
    }
    Ok(v.to_owned())
}

/// Optional text: blank becomes `None`.
pub fn optional_text(field: &str, value: Option<&str>, max_chars: usize) -> AppResult<Option<String>> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) => text(field, v, max_chars).map(Some),
    }
}

pub fn email(value: &str) -> AppResult<String> {
    let v = value.trim().to_lowercase();
    let valid = v.len() <= 254
        && v.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
        })
        && !v.chars().any(char::is_whitespace);
    if !valid {
        return Err(AppError::validation("email is not a valid email address"));
    }
    Ok(v)
}

/// URL-safe practice handle: 3–63 chars of a-z, 0-9 and inner hyphens.
pub fn slug(value: &str) -> AppResult<String> {
    let v = value.trim().to_lowercase();
    let ok = (3..=63).contains(&v.len())
        && v.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !v.starts_with('-')
        && !v.ends_with('-');
    if !ok {
        return Err(AppError::validation(
            "slug must be 3-63 characters of a-z, 0-9 and hyphens, not starting or ending with a hyphen",
        ));
    }
    Ok(v)
}

/// BHF PCNS practice number: 7 digits (spaces ignored).
pub fn pcns_number(value: &str) -> AppResult<String> {
    let v: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    if v.len() != 7 || !v.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::validation("pcns_number must be 7 digits"));
    }
    Ok(v)
}

// NOTE: messages below must never echo the submitted value — it may be
// personal information.

/// What a valid SA ID number tells us.
#[derive(Clone, Copy)]
pub struct SaIdInfo {
    pub date_of_birth: chrono::NaiveDate,
}

/// Validate a 13-digit South African ID number (YYMMDD SSSS C A Z):
/// real date of birth, citizenship digit 0/1/2, Luhn check digit.
pub fn sa_id_number(value: &str, today: chrono::NaiveDate) -> AppResult<(String, SaIdInfo)> {
    let v: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    if v.len() != 13 || !v.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::validation("SA ID number must be 13 digits"));
    }
    let d: Vec<u32> = v.chars().map(|c| c.to_digit(10).unwrap_or(0)).collect();

    let yy = (d[0] * 10 + d[1]) as i32;
    let (mm, dd) = (d[2] * 10 + d[3], d[4] * 10 + d[5]);
    // Two-digit year: pick the most recent century that isn't in the future.
    let dob = [2000 + yy, 1900 + yy]
        .into_iter()
        .filter_map(|y| chrono::NaiveDate::from_ymd_opt(y, mm, dd))
        .find(|date| *date <= today)
        .ok_or_else(|| AppError::validation("SA ID number has an invalid date of birth"))?;

    if d[10] > 2 {
        return Err(AppError::validation("SA ID number has an invalid citizenship digit"));
    }
    if !luhn_valid(&d) {
        return Err(AppError::validation("SA ID number check digit is wrong"));
    }
    Ok((v, SaIdInfo { date_of_birth: dob }))
}

fn luhn_valid(digits: &[u32]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| if i % 2 == 1 { let x = d * 2; if x > 9 { x - 9 } else { x } } else { d })
        .sum();
    sum.is_multiple_of(10)
}

/// Phone number: keeps digits and a leading +; 7–15 digits.
pub fn phone(field: &str, value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    let plus = trimmed.starts_with('+');
    if trimmed.chars().any(|c| !(c.is_ascii_digit() || " ()-+".contains(c))) {
        return Err(AppError::validation(format!("{field} is not a valid phone number")));
    }
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if !(7..=15).contains(&digits.len()) {
        return Err(AppError::validation(format!("{field} is not a valid phone number")));
    }
    Ok(if plus { format!("+{digits}") } else { digits })
}

pub fn postal_code(value: &str) -> AppResult<String> {
    let v = value.trim();
    if v.len() != 4 || !v.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::validation("postal_code must be 4 digits"));
    }
    Ok(v.to_owned())
}

/// Medical aid dependant code: two digits, 00 = main member.
pub fn dependant_code(value: &str) -> AppResult<String> {
    let v = value.trim();
    if v.len() != 2 || !v.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::validation("dependant_code must be two digits, e.g. 00 or 01"));
    }
    Ok(v.to_owned())
}

/// Medical aid membership number, normalised to upper case without spaces.
pub fn membership_number(value: &str) -> AppResult<String> {
    let v: String = value.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    let ok = (1..=30).contains(&v.len())
        && v.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '/' || c == '-');
    if !ok {
        return Err(AppError::validation(
            "membership_number must be 1-30 letters, digits, '/' or '-'",
        ));
    }
    Ok(v)
}

/// For PATCH fields that can be cleared: absent → `None`, `null` →
/// `Some(None)`, value → `Some(Some(v))`. Use with `#[serde(default)]`.
pub fn double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug(" Dr-Smith ").unwrap(), "dr-smith");
        assert!(slug("ab").is_err());
        assert!(slug("-abc").is_err());
        assert!(slug("abc_def").is_err());
    }

    #[test]
    fn pcns() {
        assert_eq!(pcns_number("012 3456").unwrap(), "0123456");
        assert!(pcns_number("123456").is_err());
        assert!(pcns_number("12345a7").is_err());
    }

    fn day(y: i32, m: u32, d: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn sa_ids() {
        let today = day(2026, 11, 7);
        let (v, info) = sa_id_number("800101 5009 087", today).unwrap();
        assert_eq!(v, "8001015009087");
        assert_eq!(info.date_of_birth, day(1980, 1, 1));
        // Wrong check digit
        assert!(sa_id_number("8001015009088", today).is_err());
        // Bad date (Feb 30)
        assert!(sa_id_number("8002305009087", today).is_err());
        // Too short
        assert!(sa_id_number("800101500908", today).is_err());
    }

    #[test]
    fn sa_id_century() {
        // 05 → 2005, not 1905; 30 → 1930 (2030 is in the future).
        let today = day(2026, 11, 7);
        assert_eq!(sa_id_number("0501015800086", today).unwrap().1.date_of_birth, day(2005, 1, 1));
        assert_eq!(sa_id_number("3001015800084", today).unwrap().1.date_of_birth, day(1930, 1, 1));
    }

    #[test]
    fn luhn_known_good() {
        let d: Vec<u32> = "8001015009087".chars().map(|c| c.to_digit(10).unwrap()).collect();
        assert!(luhn_valid(&d));
    }

    #[test]
    fn phones_and_codes() {
        assert_eq!(phone("mobile", "+27 (82) 555-1234").unwrap(), "+27825551234");
        assert!(phone("mobile", "082 555 12ab").is_err());
        assert_eq!(dependant_code("01").unwrap(), "01");
        assert!(dependant_code("1").is_err());
        assert_eq!(membership_number(" ab 12-3 ").unwrap(), "AB12-3");
    }

    #[test]
    fn emails() {
        assert_eq!(email(" Thabo@Example.co.za ").unwrap(), "thabo@example.co.za");
        assert!(email("no-at-sign").is_err());
        assert!(email("a@b").is_err());
    }
}
