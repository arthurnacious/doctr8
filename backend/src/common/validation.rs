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

    #[test]
    fn emails() {
        assert_eq!(email(" Thabo@Example.co.za ").unwrap(), "thabo@example.co.za");
        assert!(email("no-at-sign").is_err());
        assert!(email("a@b").is_err());
    }
}
