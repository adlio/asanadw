use chrono::{Datelike, Duration, NaiveDate};

/// Get the last day of a given month.
pub fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap() - Duration::days(1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap() - Duration::days(1)
    }
}

/// Get the quarter (1-4) for a given date.
pub fn quarter_of(d: NaiveDate) -> u8 {
    ((d.month() - 1) / 3 + 1) as u8
}

/// Strip markdown code fences from LLM responses.
pub fn strip_code_fences(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        rest.strip_suffix("```").unwrap_or(rest).trim()
    } else if let Some(rest) = s.strip_prefix("```") {
        rest.strip_suffix("```").unwrap_or(rest).trim()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_day_of_month() {
        assert_eq!(
            last_day_of_month(2025, 1),
            NaiveDate::from_ymd_opt(2025, 1, 31).unwrap()
        );
        assert_eq!(
            last_day_of_month(2025, 2),
            NaiveDate::from_ymd_opt(2025, 2, 28).unwrap()
        );
        assert_eq!(
            last_day_of_month(2024, 2),
            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
        ); // Leap year
        assert_eq!(
            last_day_of_month(2025, 12),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()
        );
    }

    #[test]
    fn test_quarter_of() {
        assert_eq!(quarter_of(NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()), 1);
        assert_eq!(quarter_of(NaiveDate::from_ymd_opt(2025, 3, 31).unwrap()), 1);
        assert_eq!(quarter_of(NaiveDate::from_ymd_opt(2025, 4, 1).unwrap()), 2);
        assert_eq!(quarter_of(NaiveDate::from_ymd_opt(2025, 6, 30).unwrap()), 2);
        assert_eq!(quarter_of(NaiveDate::from_ymd_opt(2025, 7, 1).unwrap()), 3);
        assert_eq!(
            quarter_of(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()),
            4
        );
    }

    #[test]
    fn test_strip_code_fences_json() {
        assert_eq!(
            strip_code_fences("```json\n{\"key\": \"value\"}\n```"),
            "{\"key\": \"value\"}"
        );
    }

    #[test]
    fn test_strip_code_fences_plain() {
        assert_eq!(
            strip_code_fences("```\n{\"key\": \"value\"}\n```"),
            "{\"key\": \"value\"}"
        );
    }

    #[test]
    fn test_strip_code_fences_none() {
        assert_eq!(
            strip_code_fences("{\"key\": \"value\"}"),
            "{\"key\": \"value\"}"
        );
    }

    #[test]
    fn test_strip_code_fences_whitespace() {
        assert_eq!(strip_code_fences("  ```json\n{}\n```  "), "{}");
    }
}
