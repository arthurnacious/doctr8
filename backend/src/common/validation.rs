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
            !local.is_empty()
                && !domain.contains('@')
                && !domain.contains("..")
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
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

/// Escape LIKE wildcards (`%`, `_`, `\\`) so user input matches literally.
pub fn like_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    for c in value.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
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

#[cfg(test)]
mod edge_cases {
    use serde::Deserialize;

    use super::*;
    use crate::common::test_support::{date, today};

    #[test]
    fn text_trims_requires_and_counts_characters_not_bytes() {
        assert_eq!(text("name", "  Thabo  ", 10).unwrap(), "Thabo");
        assert!(text("name", "   ", 10).unwrap_err().to_string().contains("name is required"));
        // 5 multi-byte characters fit a 5-character limit.
        assert_eq!(text("name", "Ngūgĩ", 5).unwrap(), "Ngūgĩ");
        assert!(text("name", "abcdef", 5).unwrap_err().to_string().contains("at most 5"));
    }

    #[test]
    fn optional_text_blank_is_none() {
        assert_eq!(optional_text("x", None, 5).unwrap(), None);
        assert_eq!(optional_text("x", Some("   "), 5).unwrap(), None);
        assert_eq!(optional_text("x", Some(" a "), 5).unwrap(), Some("a".into()));
        assert!(optional_text("x", Some("toolong"), 5).is_err());
    }

    #[test]
    fn email_rules() {
        for bad in ["", "@x.co", "a@", "a@.co", "a@co.", "a b@x.co", "a@x", "a@@x.co", "a@b@x.co", "a@x..co"] {
            assert!(email(bad).is_err(), "{bad:?}");
        }
        assert!(email(&format!("{}@x.co", "a".repeat(250))).is_err(), "over 254 chars");
        assert_eq!(email("A.B+tag@Sub.Example.CO.ZA").unwrap(), "a.b+tag@sub.example.co.za");
    }

    #[test]
    fn slug_length_bounds() {
        assert!(slug("abc").is_ok());
        assert!(slug(&"a".repeat(63)).is_ok());
        assert!(slug(&"a".repeat(64)).is_err());
        assert!(slug("ab-").is_err());
        assert!(slug("a b c").is_err());
        assert_eq!(slug("dr-1-smith").unwrap(), "dr-1-smith");
    }

    #[test]
    fn sa_id_rejects_bad_citizenship_digit() {
        // Valid date and Luhn digit but citizenship digit 3.
        let mut digits: Vec<u32> = "800101500930".chars().map(|c| c.to_digit(10).unwrap()).collect();
        let check = (0..10).find(|c| { let mut d = digits.clone(); d.push(*c); luhn_valid(&d) }).unwrap();
        digits.push(check);
        let id: String = digits.iter().map(|d| char::from_digit(*d, 10).unwrap()).collect();
        assert!(sa_id_number(&id, today()).err().unwrap().to_string().contains("citizenship"));
    }

    #[test]
    fn sa_id_leap_days() {
        let with_check = |prefix: &str| {
            let d: Vec<u32> = prefix.chars().map(|c| c.to_digit(10).unwrap()).collect();
            let c = (0..10).find(|c| { let mut x = d.clone(); x.push(*c); luhn_valid(&x) }).unwrap();
            format!("{prefix}{c}")
        };
        // 2000-02-29 existed; 2001-02-29 / 1901-02-29 did not.
        assert_eq!(sa_id_number(&with_check("000229500008"), today()).unwrap().1.date_of_birth, date(2000, 2, 29));
        assert!(sa_id_number(&with_check("010229500008"), today()).is_err());
    }

    #[test]
    fn sa_id_born_later_this_year_belongs_to_last_century() {
        // Today is 2026-11-07: an ID starting 261225 must be 1926-12-25.
        let d: Vec<u32> = "261225500008".chars().map(|c| c.to_digit(10).unwrap()).collect();
        let c = (0..10).find(|c| { let mut x = d.clone(); x.push(*c); luhn_valid(&x) }).unwrap();
        let id = format!("261225500008{c}");
        assert_eq!(sa_id_number(&id, today()).unwrap().1.date_of_birth, date(1926, 12, 25));
    }

    #[test]
    fn sa_id_messages_never_echo_the_number() {
        for bad in ["9002150123089", "9013150123088", "12345", "90021501230880"] {
            let msg = sa_id_number(bad, today()).err().unwrap().to_string();
            assert!(!msg.contains(bad) && !msg.contains("900215"), "{msg}");
        }
    }

    #[test]
    fn luhn_detects_single_digit_errors() {
        let good: Vec<u32> = "8001015009087".chars().map(|c| c.to_digit(10).unwrap()).collect();
        for i in 0..good.len() {
            let mut bad = good.clone();
            bad[i] = (bad[i] + 1) % 10;
            assert!(!luhn_valid(&bad), "change at position {i} not detected");
        }
    }

    #[test]
    fn phone_rules() {
        assert_eq!(phone("p", "0825551234").unwrap(), "0825551234");
        assert_eq!(phone("p", "  +27 82 555 1234 ").unwrap(), "+27825551234");
        assert!(phone("p", "123456").is_err(), "too short");
        assert!(phone("p", "1234567890123456").is_err(), "too long");
        assert!(phone("p", "082.555.1234").is_err());
        assert!(!phone("mobile", "082 555 12ab").unwrap_err().to_string().contains("12ab"));
    }

    #[test]
    fn postal_codes() {
        assert_eq!(postal_code(" 0181 ").unwrap(), "0181");
        for bad in ["181", "01811", "O181", ""] {
            assert!(postal_code(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn dependant_codes() {
        for ok in ["00", "01", "99", " 05 "] {
            assert!(dependant_code(ok).is_ok(), "{ok:?}");
        }
        for bad in ["0", "000", "a1", "-1", ""] {
            assert!(dependant_code(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn membership_numbers() {
        assert_eq!(membership_number("abc/12-3").unwrap(), "ABC/12-3");
        assert!(membership_number(&"1".repeat(30)).is_ok());
        assert!(membership_number(&"1".repeat(31)).is_err());
        assert!(membership_number("   ").is_err());
        assert!(membership_number("12#3").is_err());
    }

    #[test]
    fn like_escape_makes_wildcards_literal() {
        assert_eq!(like_escape("J06%"), "J06\\%");
        assert_eq!(like_escape("a_b\\c"), "a\\_b\\\\c");
        assert_eq!(like_escape("plain"), "plain");
    }

    #[test]
    fn pcns_rejects_wrong_lengths() {
        assert!(pcns_number("12345678").is_err());
        assert!(pcns_number("").is_err());
    }

    #[derive(Deserialize)]
    struct Patch {
        #[serde(default, deserialize_with = "double_option")]
        field: Option<Option<String>>,
    }

    #[test]
    fn double_option_distinguishes_absent_null_and_value() {
        let p: Patch = serde_json::from_str("{}").unwrap();
        assert_eq!(p.field, None, "absent = leave unchanged");
        let p: Patch = serde_json::from_str(r#"{"field":null}"#).unwrap();
        assert_eq!(p.field, Some(None), "null = clear");
        let p: Patch = serde_json::from_str(r#"{"field":"x"}"#).unwrap();
        assert_eq!(p.field, Some(Some("x".into())));
    }
}
