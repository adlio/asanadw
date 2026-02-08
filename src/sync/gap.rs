use chrono::{Datelike, Duration, NaiveDate};

use crate::date_util::last_day_of_month;

/// A date range [start, end] inclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

/// Given a desired range and a list of already-synced ranges, find the gaps
/// that still need syncing. Ranges are month-aligned for efficient batch queries.
pub fn find_gaps(
    desired_start: NaiveDate,
    desired_end: NaiveDate,
    synced_ranges: &[(NaiveDate, NaiveDate)],
) -> Vec<DateRange> {
    if desired_start > desired_end {
        return vec![];
    }

    // Merge and sort synced ranges
    let merged = merge_ranges(synced_ranges);

    // Find gaps between desired range and merged synced ranges
    let mut gaps = Vec::new();
    let mut cursor = desired_start;

    for range in &merged {
        if range.start > cursor {
            // Gap before this range
            let gap_end = range.start - Duration::days(1);
            if gap_end >= cursor && gap_end <= desired_end {
                gaps.push(DateRange {
                    start: cursor,
                    end: gap_end.min(desired_end),
                });
            }
        }
        cursor = range.end + Duration::days(1);
    }

    // Gap after the last synced range
    if cursor <= desired_end {
        gaps.push(DateRange {
            start: cursor,
            end: desired_end,
        });
    }

    // Align to month boundaries for efficient batching
    gaps.into_iter()
        .flat_map(|g| split_into_months(g.start, g.end))
        .collect()
}

/// Merge overlapping/adjacent date ranges.
fn merge_ranges(ranges: &[(NaiveDate, NaiveDate)]) -> Vec<DateRange> {
    if ranges.is_empty() {
        return vec![];
    }

    let mut sorted: Vec<DateRange> = ranges
        .iter()
        .map(|(s, e)| DateRange {
            start: *s,
            end: *e,
        })
        .collect();
    sorted.sort_by_key(|r| r.start);

    let mut merged = vec![sorted[0].clone()];
    for range in &sorted[1..] {
        let last = merged.last_mut().unwrap();
        // Adjacent or overlapping (1 day gap counts as adjacent)
        if range.start <= last.end + Duration::days(1) {
            last.end = last.end.max(range.end);
        } else {
            merged.push(range.clone());
        }
    }
    merged
}

/// Split a range into month-aligned batches.
fn split_into_months(start: NaiveDate, end: NaiveDate) -> Vec<DateRange> {
    let mut batches = Vec::new();
    let mut cursor = start;

    while cursor <= end {
        let month_end = last_day_of_month(cursor.year(), cursor.month());
        let batch_end = month_end.min(end);

        batches.push(DateRange {
            start: cursor,
            end: batch_end,
        });

        cursor = batch_end + Duration::days(1);
    }

    batches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn test_no_synced_ranges() {
        let gaps = find_gaps(d(2025, 1, 1), d(2025, 3, 31), &[]);
        // Should produce 3 month-aligned batches
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], DateRange { start: d(2025, 1, 1), end: d(2025, 1, 31) });
        assert_eq!(gaps[1], DateRange { start: d(2025, 2, 1), end: d(2025, 2, 28) });
        assert_eq!(gaps[2], DateRange { start: d(2025, 3, 1), end: d(2025, 3, 31) });
    }

    #[test]
    fn test_fully_synced() {
        let synced = vec![(d(2025, 1, 1), d(2025, 3, 31))];
        let gaps = find_gaps(d(2025, 1, 1), d(2025, 3, 31), &synced);
        assert!(gaps.is_empty());
    }

    #[test]
    fn test_gap_in_middle() {
        let synced = vec![
            (d(2025, 1, 1), d(2025, 1, 31)),
            (d(2025, 3, 1), d(2025, 3, 31)),
        ];
        let gaps = find_gaps(d(2025, 1, 1), d(2025, 3, 31), &synced);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0], DateRange { start: d(2025, 2, 1), end: d(2025, 2, 28) });
    }

    #[test]
    fn test_gap_at_end() {
        let synced = vec![(d(2025, 1, 1), d(2025, 2, 28))];
        let gaps = find_gaps(d(2025, 1, 1), d(2025, 3, 31), &synced);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0], DateRange { start: d(2025, 3, 1), end: d(2025, 3, 31) });
    }

    #[test]
    fn test_overlapping_synced_ranges_merged() {
        let synced = vec![
            (d(2025, 1, 1), d(2025, 1, 20)),
            (d(2025, 1, 15), d(2025, 2, 15)),
        ];
        let gaps = find_gaps(d(2025, 1, 1), d(2025, 3, 31), &synced);
        // Gap should start at Feb 16 through Mar 31
        assert!(gaps.len() >= 1);
        assert_eq!(gaps[0].start, d(2025, 2, 16));
    }

    #[test]
    fn test_split_into_months() {
        let batches = split_into_months(d(2025, 1, 15), d(2025, 3, 10));
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0], DateRange { start: d(2025, 1, 15), end: d(2025, 1, 31) });
        assert_eq!(batches[1], DateRange { start: d(2025, 2, 1), end: d(2025, 2, 28) });
        assert_eq!(batches[2], DateRange { start: d(2025, 3, 1), end: d(2025, 3, 10) });
    }
}
