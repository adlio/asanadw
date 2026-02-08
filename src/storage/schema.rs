use chrono::{Datelike, Duration, NaiveDate, Weekday};
use rusqlite::params;

use crate::date_util::{last_day_of_month, quarter_of};

/// Populate dim_date from `start_year-01-01` through end of `end_year`.
/// Called on DB open; skips dates that already exist.
pub fn ensure_dim_date(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let today = chrono::Local::now().date_naive();
    let start = NaiveDate::from_ymd_opt(today.year() - 2, 1, 1).unwrap();
    // End of current year + 1 quarter
    let end_year = today.year();
    let end_quarter_month = ((today.month0() / 3) + 1) * 3 + 3; // next quarter end month
    let end = if end_quarter_month > 12 {
        NaiveDate::from_ymd_opt(end_year + 1, end_quarter_month - 12, 1)
            .unwrap()
            .pred_opt()
            .unwrap_or(NaiveDate::from_ymd_opt(end_year + 1, 3, 31).unwrap())
    } else {
        last_day_of_month(end_year, end_quarter_month)
    };

    let existing: i64 =
        conn.query_row("SELECT COUNT(*) FROM dim_date", [], |row| row.get(0))?;
    if existing > 0 {
        // Extend if needed: find max date and add new dates if end > max
        let max_str: String = conn.query_row(
            "SELECT MAX(date_key) FROM dim_date",
            [],
            |row| row.get(0),
        )?;
        let max_date = NaiveDate::parse_from_str(&max_str, "%Y-%m-%d").unwrap();
        if end <= max_date {
            return Ok(());
        }
        // Insert dates from max_date+1 to end
        insert_date_range(conn, max_date + Duration::days(1), end)?;
        return Ok(());
    }

    insert_date_range(conn, start, end)
}

fn insert_date_range(
    conn: &rusqlite::Connection,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO dim_date (
            date_key, year, quarter, month, week, day_of_week, day_of_month,
            day_of_year, is_weekend, is_first_day_of_month, is_last_day_of_month,
            is_first_day_of_quarter, is_last_day_of_quarter,
            year_key, half_key, quarter_key, month_key, week_key,
            day_of_quarter, day_of_half,
            prior_year_date_key, prior_quarter_date_key, prior_month_date_key
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23
        )",
    )?;

    let mut d = start;
    while d <= end {
        let quarter = quarter_of(d);
        let half = if quarter <= 2 { 1 } else { 2 };
        let iso_week = d.iso_week();
        let dow = d.weekday().num_days_from_monday() + 1; // 1=Mon, 7=Sun

        let quarter_start = quarter_start_date(d.year(), quarter);
        let half_start = if half == 1 {
            NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(d.year(), 7, 1).unwrap()
        };
        let day_of_quarter = (d - quarter_start).num_days() as i32;
        let day_of_half = (d - half_start).num_days() as i32;

        let last_of_month = last_day_of_month(d.year(), d.month());
        let quarter_end = quarter_end_date(d.year(), quarter);

        let prior_year_date = prior_year_date(d);
        let prior_quarter_date = prior_quarter_date(d, day_of_quarter);
        let prior_month_date = prior_month_date(d);

        stmt.execute(params![
            d.format("%Y-%m-%d").to_string(),
            d.year(),
            quarter,
            d.month(),
            iso_week.week(),
            dow,
            d.day(),
            d.ordinal(),
            if dow >= 6 { 1 } else { 0 },
            if d.day() == 1 { 1 } else { 0 },
            if d == last_of_month { 1 } else { 0 },
            if d == quarter_start { 1 } else { 0 },
            if d == quarter_end { 1 } else { 0 },
            format!("{}", d.year()),
            format!("{}-H{}", d.year(), half),
            format!("{}-Q{}", d.year(), quarter),
            format!("{}-{:02}", d.year(), d.month()),
            format!("{}-W{:02}", iso_week.year(), iso_week.week()),
            day_of_quarter,
            day_of_half,
            prior_year_date.map(|d| d.format("%Y-%m-%d").to_string()),
            prior_quarter_date.map(|d| d.format("%Y-%m-%d").to_string()),
            prior_month_date.map(|d| d.format("%Y-%m-%d").to_string()),
        ])?;

        d += Duration::days(1);
    }
    Ok(())
}

/// Populate dim_period based on dates in dim_date.
pub fn ensure_dim_period(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let existing: i64 =
        conn.query_row("SELECT COUNT(*) FROM dim_period", [], |row| row.get(0))?;
    if existing > 0 {
        return Ok(());
    }

    // Get the year range from dim_date
    let (min_year, max_year): (i32, i32) = conn.query_row(
        "SELECT MIN(year), MAX(year) FROM dim_date",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO dim_period (
            period_key, period_type, label, start_date, end_date,
            days_in_period, prior_period_key
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;

    for year in min_year..=max_year {
        // Year period
        let ys = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
        let ye = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
        stmt.execute(params![
            format!("{year}"),
            "year",
            format!("{year}"),
            ys.format("%Y-%m-%d").to_string(),
            ye.format("%Y-%m-%d").to_string(),
            (ye - ys).num_days() + 1,
            format!("{}", year - 1),
        ])?;

        // Half periods
        for h in 1..=2u8 {
            let hs = if h == 1 {
                NaiveDate::from_ymd_opt(year, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(year, 7, 1).unwrap()
            };
            let he = if h == 1 {
                NaiveDate::from_ymd_opt(year, 6, 30).unwrap()
            } else {
                NaiveDate::from_ymd_opt(year, 12, 31).unwrap()
            };
            let prior = if h == 1 {
                format!("{}-H2", year - 1)
            } else {
                format!("{year}-H1")
            };
            let label = if h == 1 {
                format!("H1 {year}")
            } else {
                format!("H2 {year}")
            };
            stmt.execute(params![
                format!("{year}-H{h}"),
                "half",
                label,
                hs.format("%Y-%m-%d").to_string(),
                he.format("%Y-%m-%d").to_string(),
                (he - hs).num_days() + 1,
                prior,
            ])?;
        }

        // Quarter periods
        for q in 1..=4u8 {
            let qs = quarter_start_date(year, q);
            let qe = quarter_end_date(year, q);
            let prior = if q == 1 {
                format!("{}-Q4", year - 1)
            } else {
                format!("{year}-Q{}", q - 1)
            };
            stmt.execute(params![
                format!("{year}-Q{q}"),
                "quarter",
                format!("Q{q} {year}"),
                qs.format("%Y-%m-%d").to_string(),
                qe.format("%Y-%m-%d").to_string(),
                (qe - qs).num_days() + 1,
                prior,
            ])?;
        }

        // Month periods
        for m in 1..=12u32 {
            let ms = NaiveDate::from_ymd_opt(year, m, 1).unwrap();
            let me = last_day_of_month(year, m);
            let prior = if m == 1 {
                format!("{}-12", year - 1)
            } else {
                format!("{year}-{:02}", m - 1)
            };
            let month_name = month_label(m);
            stmt.execute(params![
                format!("{year}-{m:02}"),
                "month",
                format!("{month_name} {year}"),
                ms.format("%Y-%m-%d").to_string(),
                me.format("%Y-%m-%d").to_string(),
                (me - ms).num_days() + 1,
                prior,
            ])?;
        }

        // Week periods — find all distinct ISO weeks for this year
        let weeks: Vec<(i32, u32)> = {
            let mut wstmt = conn.prepare(
                "SELECT DISTINCT week FROM dim_date WHERE year = ?1 ORDER BY week",
            )?;
            let rows = wstmt.query_map([year], |row| row.get::<_, u32>(0))?;
            rows.filter_map(|r| r.ok())
                .map(|w| (year, w))
                .collect()
        };

        for (y, w) in weeks {
            let ws = NaiveDate::from_isoywd_opt(y, w, Weekday::Mon);
            let we = ws.map(|d| d + Duration::days(6));
            if let (Some(ws), Some(we)) = (ws, we) {
                let prior_ws = ws - Duration::weeks(1);
                let prior_iw = prior_ws.iso_week();
                stmt.execute(params![
                    format!("{y}-W{w:02}"),
                    "week",
                    format!("Week {w}, {y}"),
                    ws.format("%Y-%m-%d").to_string(),
                    we.format("%Y-%m-%d").to_string(),
                    7,
                    format!("{}-W{:02}", prior_iw.year(), prior_iw.week()),
                ])?;
            }
        }
    }

    Ok(())
}

fn quarter_start_date(year: i32, q: u8) -> NaiveDate {
    let m = (q as u32 - 1) * 3 + 1;
    NaiveDate::from_ymd_opt(year, m, 1).unwrap()
}

fn quarter_end_date(year: i32, q: u8) -> NaiveDate {
    let m = q as u32 * 3;
    last_day_of_month(year, m)
}

fn prior_year_date(d: NaiveDate) -> Option<NaiveDate> {
    // Feb 29 -> None
    NaiveDate::from_ymd_opt(d.year() - 1, d.month(), d.day())
}

fn prior_quarter_date(d: NaiveDate, day_of_quarter: i32) -> Option<NaiveDate> {
    let q = quarter_of(d);
    let (py, pq) = if q == 1 { (d.year() - 1, 4u8) } else { (d.year(), q - 1) };
    let pqs = quarter_start_date(py, pq);
    let pqe = quarter_end_date(py, pq);
    let target = pqs + Duration::days(day_of_quarter as i64);
    if target > pqe {
        Some(pqe)
    } else {
        Some(target)
    }
}

fn prior_month_date(d: NaiveDate) -> Option<NaiveDate> {
    let (py, pm) = if d.month() == 1 {
        (d.year() - 1, 12)
    } else {
        (d.year(), d.month() - 1)
    };
    let last = last_day_of_month(py, pm);
    let day = d.day().min(last.day());
    NaiveDate::from_ymd_opt(py, pm, day)
}

fn month_label(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}
