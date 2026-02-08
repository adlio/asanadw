use std::sync::LazyLock;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use regex::Regex;

use crate::date_util::{last_day_of_month, quarter_of};
use crate::error::{Error, Result};

static RE_HALF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{4})-H([12])$").unwrap());
static RE_QUARTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{4})-Q([1-4])$").unwrap());
static RE_WEEK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{4})-W(\d{1,2})$").unwrap());
static RE_MONTH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{4})-(\d{2})$").unwrap());

/// A time period for metrics and queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Period {
    Year(i32),
    Half(i32, u8),
    Quarter(i32, u8),
    Month(i32, u8),
    Week(i32, u8),
    Rolling(u32, NaiveDate),
    YearToDate(i32),
    HalfToDate(i32, u8),
    QuarterToDate(i32, u8),
    MonthToDate(i32, u8),
    WeekToDate(i32, u8),
}

impl Period {
    /// Parse a period string.
    ///
    /// Supported formats:
    /// - `2025` — year
    /// - `2025-H1` — half
    /// - `2025-Q1` — quarter
    /// - `2025-01` — month
    /// - `2025-W05` — ISO week
    /// - `30d` — rolling last N days
    /// - `ytd` — year to date (current year)
    /// - `htd` — half to date (current half)
    /// - `qtd` — quarter to date (current quarter)
    /// - `mtd` — month to date (current month)
    /// - `wtd` — week to date (current week)
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        let today = chrono::Local::now().date_naive();

        // To-date periods
        match s.to_lowercase().as_str() {
            "ytd" => {
                return Ok(Period::YearToDate(today.year()));
            }
            "htd" => {
                let half = if today.month() <= 6 { 1 } else { 2 };
                return Ok(Period::HalfToDate(today.year(), half));
            }
            "qtd" => {
                let q = quarter_of(today);
                return Ok(Period::QuarterToDate(today.year(), q));
            }
            "mtd" => {
                return Ok(Period::MonthToDate(today.year(), today.month() as u8));
            }
            "wtd" => {
                let iw = today.iso_week();
                return Ok(Period::WeekToDate(iw.year(), iw.week() as u8));
            }
            _ => {}
        }

        // Rolling: "30d", "7d", etc.
        if s.ends_with('d') || s.ends_with('D') {
            if let Ok(n) = s[..s.len() - 1].parse::<u32>() {
                return Ok(Period::Rolling(n, today));
            }
        }

        // Year-qualified to-date: "2025-ytd"
        if let Some(rest) = s.strip_suffix("-ytd") {
            let year: i32 = rest
                .parse()
                .map_err(|_| Error::PeriodParse(format!("invalid year: {s}")))?;
            return Ok(Period::YearToDate(year));
        }

        // Explicit half/quarter to-date: "2025-H1-td", "2025-Q1-td"
        // Not in spec, skip for now.

        // Year: "2025"
        if s.len() == 4 {
            if let Ok(year) = s.parse::<i32>() {
                return Ok(Period::Year(year));
            }
        }

        // Half: "2025-H1" or "2025-H2"
        if let Some(caps) = RE_HALF.captures(s) {
            let year: i32 = caps[1].parse().unwrap();
            let half: u8 = caps[2].parse().unwrap();
            return Ok(Period::Half(year, half));
        }

        // Quarter: "2025-Q1" through "2025-Q4"
        if let Some(caps) = RE_QUARTER.captures(s) {
            let year: i32 = caps[1].parse().unwrap();
            let q: u8 = caps[2].parse().unwrap();
            return Ok(Period::Quarter(year, q));
        }

        // Week: "2025-W05"
        if let Some(caps) = RE_WEEK.captures(s) {
            let year: i32 = caps[1].parse().unwrap();
            let week: u8 = caps[2].parse().unwrap();
            if (1..=53).contains(&week) {
                return Ok(Period::Week(year, week));
            }
        }

        // Month: "2025-01"
        if let Some(caps) = RE_MONTH.captures(s) {
            let year: i32 = caps[1].parse().unwrap();
            let month: u8 = caps[2].parse().unwrap();
            if (1..=12).contains(&month) {
                return Ok(Period::Month(year, month));
            }
        }

        Err(Error::PeriodParse(format!("unrecognized period: {s}")))
    }

    /// Convert to a canonical key string for storage/lookup.
    pub fn to_key(&self) -> String {
        match self {
            Period::Year(y) => format!("{y}"),
            Period::Half(y, h) => format!("{y}-H{h}"),
            Period::Quarter(y, q) => format!("{y}-Q{q}"),
            Period::Month(y, m) => format!("{y}-{m:02}"),
            Period::Week(y, w) => format!("{y}-W{w:02}"),
            Period::Rolling(n, _) => format!("{n}d"),
            Period::YearToDate(y) => format!("{y}-ytd"),
            Period::HalfToDate(y, h) => format!("{y}-H{h}-td"),
            Period::QuarterToDate(y, q) => format!("{y}-Q{q}-td"),
            Period::MonthToDate(y, m) => format!("{y}-{m:02}-td"),
            Period::WeekToDate(y, w) => format!("{y}-W{w:02}-td"),
        }
    }

    /// Get the date range (inclusive start, inclusive end) for this period.
    pub fn date_range(&self) -> (NaiveDate, NaiveDate) {
        let today = chrono::Local::now().date_naive();
        match self {
            Period::Year(y) => (
                NaiveDate::from_ymd_opt(*y, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(*y, 12, 31).unwrap(),
            ),
            Period::Half(y, h) => {
                if *h == 1 {
                    (
                        NaiveDate::from_ymd_opt(*y, 1, 1).unwrap(),
                        NaiveDate::from_ymd_opt(*y, 6, 30).unwrap(),
                    )
                } else {
                    (
                        NaiveDate::from_ymd_opt(*y, 7, 1).unwrap(),
                        NaiveDate::from_ymd_opt(*y, 12, 31).unwrap(),
                    )
                }
            }
            Period::Quarter(y, q) => {
                let start_month = (*q as u32 - 1) * 3 + 1;
                let end_month = *q as u32 * 3;
                (
                    NaiveDate::from_ymd_opt(*y, start_month, 1).unwrap(),
                    last_day_of_month(*y, end_month),
                )
            }
            Period::Month(y, m) => (
                NaiveDate::from_ymd_opt(*y, *m as u32, 1).unwrap(),
                last_day_of_month(*y, *m as u32),
            ),
            Period::Week(y, w) => {
                let start =
                    NaiveDate::from_isoywd_opt(*y, *w as u32, Weekday::Mon).unwrap();
                (start, start + Duration::days(6))
            }
            Period::Rolling(n, as_of) => (*as_of - Duration::days(*n as i64 - 1), *as_of),
            Period::YearToDate(y) => (NaiveDate::from_ymd_opt(*y, 1, 1).unwrap(), today),
            Period::HalfToDate(y, h) => {
                let start = if *h == 1 {
                    NaiveDate::from_ymd_opt(*y, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(*y, 7, 1).unwrap()
                };
                (start, today)
            }
            Period::QuarterToDate(y, q) => {
                let start_month = (*q as u32 - 1) * 3 + 1;
                (
                    NaiveDate::from_ymd_opt(*y, start_month, 1).unwrap(),
                    today,
                )
            }
            Period::MonthToDate(y, m) => {
                (NaiveDate::from_ymd_opt(*y, *m as u32, 1).unwrap(), today)
            }
            Period::WeekToDate(y, w) => {
                let start =
                    NaiveDate::from_isoywd_opt(*y, *w as u32, Weekday::Mon).unwrap();
                (start, today)
            }
        }
    }

    /// Get the previous period of the same type.
    pub fn previous(&self) -> Self {
        match self {
            Period::Year(y) => Period::Year(y - 1),
            Period::Half(y, h) => {
                if *h == 1 {
                    Period::Half(y - 1, 2)
                } else {
                    Period::Half(*y, 1)
                }
            }
            Period::Quarter(y, q) => {
                if *q == 1 {
                    Period::Quarter(y - 1, 4)
                } else {
                    Period::Quarter(*y, q - 1)
                }
            }
            Period::Month(y, m) => {
                if *m == 1 {
                    Period::Month(y - 1, 12)
                } else {
                    Period::Month(*y, m - 1)
                }
            }
            Period::Week(y, w) => {
                if *w == 1 {
                    // Last week of previous year — approximate
                    Period::Week(y - 1, 52)
                } else {
                    Period::Week(*y, w - 1)
                }
            }
            Period::Rolling(n, as_of) => {
                Period::Rolling(*n, *as_of - Duration::days(*n as i64))
            }
            Period::YearToDate(y) => Period::YearToDate(y - 1),
            Period::HalfToDate(y, h) => {
                if *h == 1 {
                    Period::HalfToDate(y - 1, 2)
                } else {
                    Period::HalfToDate(*y, 1)
                }
            }
            Period::QuarterToDate(y, q) => {
                if *q == 1 {
                    Period::QuarterToDate(y - 1, 4)
                } else {
                    Period::QuarterToDate(*y, q - 1)
                }
            }
            Period::MonthToDate(y, m) => {
                if *m == 1 {
                    Period::MonthToDate(y - 1, 12)
                } else {
                    Period::MonthToDate(*y, m - 1)
                }
            }
            Period::WeekToDate(y, w) => {
                if *w == 1 {
                    Period::WeekToDate(y - 1, 52)
                } else {
                    Period::WeekToDate(*y, w - 1)
                }
            }
        }
    }

    /// For period-over-period comparisons: returns the equivalent to-date
    /// range in the prior period. E.g., if this is Q1 2026 and as_of is
    /// Feb 7, returns the prior Q1 clamped to the same day offset.
    pub fn prior_period_to_date(&self, as_of: NaiveDate) -> Self {
        let (start, _end) = self.date_range();
        let offset = (as_of - start).num_days();

        let prev = self.previous();
        let (prev_start, prev_end) = prev.date_range();
        let target = prev_start + Duration::days(offset);
        let clamped = if target > prev_end {
            prev_end
        } else {
            target
        };

        // Return as a Rolling period covering the prior period's equivalent range
        let days = (clamped - prev_start).num_days() + 1;
        Period::Rolling(days as u32, clamped)
    }

    /// Returns true if this period contains today.
    pub fn is_current(&self) -> bool {
        let today = chrono::Local::now().date_naive();
        let (start, end) = self.date_range();
        today >= start && today <= end
    }
}

impl std::fmt::Display for Period {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_year() {
        assert_eq!(Period::parse("2025").unwrap(), Period::Year(2025));
    }

    #[test]
    fn test_parse_half() {
        assert_eq!(Period::parse("2025-H1").unwrap(), Period::Half(2025, 1));
        assert_eq!(Period::parse("2025-H2").unwrap(), Period::Half(2025, 2));
    }

    #[test]
    fn test_parse_quarter() {
        assert_eq!(Period::parse("2025-Q1").unwrap(), Period::Quarter(2025, 1));
        assert_eq!(Period::parse("2025-Q4").unwrap(), Period::Quarter(2025, 4));
    }

    #[test]
    fn test_parse_month() {
        assert_eq!(Period::parse("2025-01").unwrap(), Period::Month(2025, 1));
        assert_eq!(Period::parse("2025-12").unwrap(), Period::Month(2025, 12));
    }

    #[test]
    fn test_parse_week() {
        assert_eq!(Period::parse("2025-W05").unwrap(), Period::Week(2025, 5));
        assert_eq!(Period::parse("2025-W1").unwrap(), Period::Week(2025, 1));
    }

    #[test]
    fn test_parse_rolling() {
        let p = Period::parse("30d").unwrap();
        match p {
            Period::Rolling(30, _) => {}
            _ => panic!("expected Rolling(30, _), got {p:?}"),
        }
    }

    #[test]
    fn test_parse_to_date() {
        let today = chrono::Local::now().date_naive();

        match Period::parse("ytd").unwrap() {
            Period::YearToDate(y) => assert_eq!(y, today.year()),
            p => panic!("expected YearToDate, got {p:?}"),
        }

        match Period::parse("qtd").unwrap() {
            Period::QuarterToDate(y, _) => assert_eq!(y, today.year()),
            p => panic!("expected QuarterToDate, got {p:?}"),
        }

        match Period::parse("mtd").unwrap() {
            Period::MonthToDate(y, m) => {
                assert_eq!(y, today.year());
                assert_eq!(m, today.month() as u8);
            }
            p => panic!("expected MonthToDate, got {p:?}"),
        }
    }

    #[test]
    fn test_parse_invalid() {
        assert!(Period::parse("garbage").is_err());
        assert!(Period::parse("2025-Q5").is_err());
        assert!(Period::parse("2025-13").is_err());
    }

    #[test]
    fn test_to_key() {
        assert_eq!(Period::Year(2025).to_key(), "2025");
        assert_eq!(Period::Half(2025, 1).to_key(), "2025-H1");
        assert_eq!(Period::Quarter(2025, 1).to_key(), "2025-Q1");
        assert_eq!(Period::Month(2025, 1).to_key(), "2025-01");
        assert_eq!(Period::Week(2025, 5).to_key(), "2025-W05");
    }

    #[test]
    fn test_date_range_year() {
        let (s, e) = Period::Year(2025).date_range();
        assert_eq!(s, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        assert_eq!(e, NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
    }

    #[test]
    fn test_date_range_quarter() {
        let (s, e) = Period::Quarter(2025, 1).date_range();
        assert_eq!(s, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        assert_eq!(e, NaiveDate::from_ymd_opt(2025, 3, 31).unwrap());

        let (s, e) = Period::Quarter(2025, 2).date_range();
        assert_eq!(s, NaiveDate::from_ymd_opt(2025, 4, 1).unwrap());
        assert_eq!(e, NaiveDate::from_ymd_opt(2025, 6, 30).unwrap());
    }

    #[test]
    fn test_date_range_month() {
        let (s, e) = Period::Month(2025, 2).date_range();
        assert_eq!(s, NaiveDate::from_ymd_opt(2025, 2, 1).unwrap());
        assert_eq!(e, NaiveDate::from_ymd_opt(2025, 2, 28).unwrap());
    }

    #[test]
    fn test_date_range_week() {
        let (s, e) = Period::Week(2025, 1).date_range();
        assert_eq!(s.weekday(), Weekday::Mon);
        assert_eq!((e - s).num_days(), 6);
    }

    #[test]
    fn test_previous() {
        assert_eq!(Period::Year(2025).previous(), Period::Year(2024));
        assert_eq!(Period::Half(2025, 1).previous(), Period::Half(2024, 2));
        assert_eq!(Period::Half(2025, 2).previous(), Period::Half(2025, 1));
        assert_eq!(Period::Quarter(2025, 1).previous(), Period::Quarter(2024, 4));
        assert_eq!(Period::Quarter(2025, 3).previous(), Period::Quarter(2025, 2));
        assert_eq!(Period::Month(2025, 1).previous(), Period::Month(2024, 12));
        assert_eq!(Period::Month(2025, 6).previous(), Period::Month(2025, 5));
    }

    #[test]
    fn test_prior_period_to_date() {
        // Q1 2026, as of Feb 7 = day 37 of the quarter
        let as_of = NaiveDate::from_ymd_opt(2026, 2, 7).unwrap();
        let period = Period::Quarter(2026, 1);
        let prior = period.prior_period_to_date(as_of);

        // Should be a Rolling period covering Q4 2025 from Oct 1 through Nov 7
        // Feb 7 is offset 37 from Jan 1; Oct 1 + 37 = Nov 7
        let (ps, pe) = prior.date_range();
        assert_eq!(ps, NaiveDate::from_ymd_opt(2025, 10, 1).unwrap());
        assert_eq!(pe, NaiveDate::from_ymd_opt(2025, 11, 7).unwrap());
    }

    #[test]
    fn test_is_current() {
        let today = chrono::Local::now().date_naive();
        let current_year = Period::Year(today.year());
        assert!(current_year.is_current());

        let past_year = Period::Year(today.year() - 2);
        assert!(!past_year.is_current());
    }
}
