pub mod types;

pub use types::*;

use crate::error::Result;
use crate::query::period::Period;
use crate::storage::Database;

/// Compute metrics for a user over a period.
pub async fn compute_user_metrics(
    db: &Database,
    user_gid: &str,
    period: &Period,
) -> Result<UserMetrics> {
    let (start, end) = period.date_range();
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();
    let period_key = period.to_key();
    let user_gid = user_gid.to_string();

    db.reader()
        .call(move |conn| {
            let user_name: Option<String> = conn
                .query_row(
                    "SELECT name FROM dim_users WHERE user_gid = ?1",
                    [&user_gid],
                    |row| row.get(0),
                )
                .ok();

            let throughput =
                compute_throughput_sql(conn, Some(&user_gid), None, &start_str, &end_str)?;
            let lead_time =
                compute_lead_time_sql(conn, Some(&user_gid), None, &start_str, &end_str)?;
            let collaboration =
                compute_collaboration_sql(conn, Some(&user_gid), None, &start_str, &end_str)?;

            Ok::<UserMetrics, rusqlite::Error>(UserMetrics {
                user_gid,
                user_name,
                period_key,
                throughput,
                lead_time,
                collaboration,
            })
        })
        .await
        .map_err(|e| crate::error::Error::Database(e.to_string()))
}

/// Compute metrics for a project over a period.
pub async fn compute_project_metrics(
    db: &Database,
    project_gid: &str,
    period: &Period,
) -> Result<ProjectMetrics> {
    let (start, end) = period.date_range();
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();
    let period_key = period.to_key();
    let project_gid = project_gid.to_string();

    db.reader()
        .call(move |conn| {
            let project_name: Option<String> = conn
                .query_row(
                    "SELECT name FROM dim_projects WHERE project_gid = ?1",
                    [&project_gid],
                    |row| row.get(0),
                )
                .ok();

            let throughput =
                compute_throughput_sql(conn, None, Some(&project_gid), &start_str, &end_str)?;
            let health = compute_health_sql(conn, Some(&project_gid), &end_str)?;
            let lead_time =
                compute_lead_time_sql(conn, None, Some(&project_gid), &start_str, &end_str)?;
            let collaboration =
                compute_collaboration_sql(conn, None, Some(&project_gid), &start_str, &end_str)?;

            Ok::<ProjectMetrics, rusqlite::Error>(ProjectMetrics {
                project_gid,
                project_name,
                period_key,
                throughput,
                health,
                lead_time,
                collaboration,
            })
        })
        .await
        .map_err(|e| crate::error::Error::Database(e.to_string()))
}

/// Compute metrics for a portfolio over a period.
pub async fn compute_portfolio_metrics(
    db: &Database,
    portfolio_gid: &str,
    period: &Period,
) -> Result<PortfolioMetrics> {
    let (start, end) = period.date_range();
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();
    let period_key = period.to_key();
    let portfolio_gid = portfolio_gid.to_string();

    db.reader()
        .call(move |conn| {
            let portfolio_name: Option<String> = conn
                .query_row(
                    "SELECT name FROM dim_portfolios WHERE portfolio_gid = ?1",
                    [&portfolio_gid],
                    |row| row.get(0),
                )
                .ok();

            // Get project GIDs in this portfolio
            let project_gids = get_portfolio_project_gids(conn, &portfolio_gid)?;
            let project_count = project_gids.len() as u64;

            let mut throughput = ThroughputMetrics::default();
            let mut health = HealthMetrics::default();
            let mut lead_time_days: Vec<i32> = Vec::new();
            let mut collaboration = CollaborationMetrics::default();

            for pgid in &project_gids {
                let t = compute_throughput_sql(conn, None, Some(pgid), &start_str, &end_str)?;
                throughput.tasks_created += t.tasks_created;
                throughput.tasks_completed += t.tasks_completed;
                throughput.net_new += t.net_new;

                let h = compute_health_sql(conn, Some(pgid), &end_str)?;
                health.overdue_count += h.overdue_count;
                health.unassigned_count += h.unassigned_count;
                health.stale_count += h.stale_count;
                health.total_open += h.total_open;

                let lt = compute_lead_time_raw(conn, None, Some(pgid), &start_str, &end_str)?;
                lead_time_days.extend(lt);

                let c = compute_collaboration_sql(conn, None, Some(pgid), &start_str, &end_str)?;
                collaboration.total_comments += c.total_comments;
                collaboration.total_likes += c.total_likes;
                // unique_commenters recalculated below
            }

            if health.total_open > 0 {
                health.overdue_pct = health.overdue_count as f64 / health.total_open as f64 * 100.0;
                health.unassigned_pct =
                    health.unassigned_count as f64 / health.total_open as f64 * 100.0;
            }

            // Aggregate unique commenters across portfolio
            let placeholders = project_gids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            if !project_gids.is_empty() {
                let sql = format!(
                    "SELECT COUNT(DISTINCT c.author_gid)
                     FROM fact_comments c
                     JOIN bridge_task_projects btp ON btp.task_gid = c.task_gid
                     WHERE btp.project_gid IN ({placeholders})
                       AND c.created_date_key >= ? AND c.created_date_key <= ?"
                );
                let mut stmt = conn.prepare(&sql)?;
                let mut idx = 1;
                for pgid in &project_gids {
                    stmt.raw_bind_parameter(idx, pgid)?;
                    idx += 1;
                }
                stmt.raw_bind_parameter(idx, &start_str)?;
                stmt.raw_bind_parameter(idx + 1, &end_str)?;
                let mut rows = stmt.raw_query();
                if let Some(row) = rows.next()? {
                    collaboration.unique_commenters = row.get::<_, i64>(0)? as u64;
                }
            }

            let lead_time = percentiles_from_days(&lead_time_days);

            Ok::<PortfolioMetrics, rusqlite::Error>(PortfolioMetrics {
                portfolio_gid,
                portfolio_name,
                period_key,
                throughput,
                health,
                lead_time,
                collaboration,
                project_count,
            })
        })
        .await
        .map_err(|e| crate::error::Error::Database(e.to_string()))
}

/// Compute metrics for a team over a period.
pub async fn compute_team_metrics(
    db: &Database,
    team_gid: &str,
    period: &Period,
) -> Result<TeamMetrics> {
    let (start, end) = period.date_range();
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();
    let period_key = period.to_key();
    let team_gid = team_gid.to_string();

    db.reader()
        .call(move |conn| {
            let team_name: Option<String> = conn
                .query_row(
                    "SELECT name FROM dim_teams WHERE team_gid = ?1",
                    [&team_gid],
                    |row| row.get(0),
                )
                .ok();

            // Get member GIDs
            let mut stmt =
                conn.prepare("SELECT user_gid FROM bridge_team_members WHERE team_gid = ?1")?;
            let member_gids: Vec<String> = stmt
                .query_map([&team_gid], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            let member_count = member_gids.len() as u64;

            let mut throughput = ThroughputMetrics::default();
            let mut health = HealthMetrics::default();
            let mut lead_time_days: Vec<i32> = Vec::new();
            let mut collaboration = CollaborationMetrics::default();

            for uid in &member_gids {
                let t = compute_throughput_sql(conn, Some(uid), None, &start_str, &end_str)?;
                throughput.tasks_created += t.tasks_created;
                throughput.tasks_completed += t.tasks_completed;
                throughput.net_new += t.net_new;

                let lt = compute_lead_time_raw(conn, Some(uid), None, &start_str, &end_str)?;
                lead_time_days.extend(lt);

                let c = compute_collaboration_sql(conn, Some(uid), None, &start_str, &end_str)?;
                collaboration.total_comments += c.total_comments;
                collaboration.total_likes += c.total_likes;
            }

            // Health across team's tasks (all open assigned to team members)
            if !member_gids.is_empty() {
                let placeholders = member_gids
                    .iter()
                    .map(|_| "?")
                    .collect::<Vec<_>>()
                    .join(",");
                let sql = format!(
                    "SELECT
                        SUM(CASE WHEN is_overdue = 1 THEN 1 ELSE 0 END),
                        SUM(CASE WHEN assignee_gid IS NULL THEN 1 ELSE 0 END),
                        SUM(CASE WHEN modified_at < date('now', '-14 days') THEN 1 ELSE 0 END),
                        COUNT(*)
                     FROM fact_tasks
                     WHERE is_completed = 0
                       AND assignee_gid IN ({placeholders})"
                );
                let mut stmt = conn.prepare(&sql)?;
                for (i, uid) in member_gids.iter().enumerate() {
                    stmt.raw_bind_parameter(i + 1, uid)?;
                }
                let mut rows = stmt.raw_query();
                if let Some(row) = rows.next()? {
                    health.overdue_count = row.get::<_, Option<i64>>(0)?.unwrap_or(0) as u64;
                    health.unassigned_count = row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u64;
                    health.stale_count = row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64;
                    health.total_open = row.get::<_, i64>(3)? as u64;
                }
                if health.total_open > 0 {
                    health.overdue_pct =
                        health.overdue_count as f64 / health.total_open as f64 * 100.0;
                    health.unassigned_pct =
                        health.unassigned_count as f64 / health.total_open as f64 * 100.0;
                }

                // Unique commenters across team
                let sql = format!(
                    "SELECT COUNT(DISTINCT c.author_gid)
                     FROM fact_comments c
                     JOIN fact_tasks t ON t.task_gid = c.task_gid
                     WHERE t.assignee_gid IN ({placeholders})
                       AND c.created_date_key >= ? AND c.created_date_key <= ?"
                );
                let mut stmt = conn.prepare(&sql)?;
                for (i, uid) in member_gids.iter().enumerate() {
                    stmt.raw_bind_parameter(i + 1, uid)?;
                }
                stmt.raw_bind_parameter(member_gids.len() + 1, &start_str)?;
                stmt.raw_bind_parameter(member_gids.len() + 2, &end_str)?;
                let mut rows = stmt.raw_query();
                if let Some(row) = rows.next()? {
                    collaboration.unique_commenters = row.get::<_, i64>(0)? as u64;
                }
            }

            let lead_time = percentiles_from_days(&lead_time_days);

            Ok::<TeamMetrics, rusqlite::Error>(TeamMetrics {
                team_gid,
                team_name,
                period_key,
                throughput,
                health,
                lead_time,
                collaboration,
                member_count,
            })
        })
        .await
        .map_err(|e| crate::error::Error::Database(e.to_string()))
}

// ── Internal SQL helpers ───────────────────────────────────────────

fn compute_throughput_sql(
    conn: &rusqlite::Connection,
    user_gid: Option<&str>,
    project_gid: Option<&str>,
    start: &str,
    end: &str,
) -> std::result::Result<ThroughputMetrics, rusqlite::Error> {
    #[allow(clippy::type_complexity)]
    let (where_clause, join_clause, bind_fn): (
        String,
        String,
        Box<dyn Fn(&mut rusqlite::Statement, usize) -> rusqlite::Result<()> + '_>,
    ) = build_entity_filter(user_gid, project_gid);

    // Tasks created in period
    let sql = format!(
        "SELECT COUNT(*) FROM fact_tasks t {join_clause} WHERE t.created_date_key >= ?1 AND t.created_date_key <= ?2 {where_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    stmt.raw_bind_parameter(1, start)?;
    stmt.raw_bind_parameter(2, end)?;
    bind_fn(&mut stmt, 3)?;
    let created: i64 = stmt.raw_query().next()?.unwrap().get(0)?;

    // Tasks completed in period
    let sql = format!(
        "SELECT COUNT(*) FROM fact_tasks t {join_clause} WHERE t.completed_date_key >= ?1 AND t.completed_date_key <= ?2 AND t.is_completed = 1 {where_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    stmt.raw_bind_parameter(1, start)?;
    stmt.raw_bind_parameter(2, end)?;
    bind_fn(&mut stmt, 3)?;
    let completed: i64 = stmt.raw_query().next()?.unwrap().get(0)?;

    Ok(ThroughputMetrics {
        tasks_created: created as u64,
        tasks_completed: completed as u64,
        net_new: created - completed,
    })
}

fn compute_health_sql(
    conn: &rusqlite::Connection,
    project_gid: Option<&str>,
    _end: &str,
) -> std::result::Result<HealthMetrics, rusqlite::Error> {
    let (join, where_extra) = if project_gid.is_some() {
        (
            "JOIN bridge_task_projects btp ON btp.task_gid = t.task_gid",
            " AND btp.project_gid = ?1",
        )
    } else {
        ("", "")
    };

    let sql = format!(
        "SELECT
            SUM(CASE WHEN t.due_on < date('now') AND t.due_on IS NOT NULL THEN 1 ELSE 0 END),
            SUM(CASE WHEN t.assignee_gid IS NULL THEN 1 ELSE 0 END),
            SUM(CASE WHEN t.modified_at < date('now', '-14 days') THEN 1 ELSE 0 END),
            COUNT(*)
         FROM fact_tasks t
         {join}
         WHERE t.is_completed = 0{where_extra}"
    );
    let mut stmt = conn.prepare(&sql)?;
    if let Some(pgid) = project_gid {
        stmt.raw_bind_parameter(1, pgid)?;
    }
    let mut rows = stmt.raw_query();
    let row = rows.next()?.unwrap();

    let overdue = row.get::<_, Option<i64>>(0)?.unwrap_or(0) as u64;
    let unassigned = row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u64;
    let stale = row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64;
    let total_open = row.get::<_, i64>(3)? as u64;

    Ok(HealthMetrics {
        overdue_count: overdue,
        unassigned_count: unassigned,
        stale_count: stale,
        total_open,
        overdue_pct: if total_open > 0 {
            overdue as f64 / total_open as f64 * 100.0
        } else {
            0.0
        },
        unassigned_pct: if total_open > 0 {
            unassigned as f64 / total_open as f64 * 100.0
        } else {
            0.0
        },
    })
}

fn compute_lead_time_raw(
    conn: &rusqlite::Connection,
    user_gid: Option<&str>,
    project_gid: Option<&str>,
    start: &str,
    end: &str,
) -> std::result::Result<Vec<i32>, rusqlite::Error> {
    #[allow(clippy::type_complexity)]
    let (where_clause, join_clause, bind_fn): (
        String,
        String,
        Box<dyn Fn(&mut rusqlite::Statement, usize) -> rusqlite::Result<()> + '_>,
    ) = build_entity_filter(user_gid, project_gid);

    let sql = format!(
        "SELECT t.days_to_complete FROM fact_tasks t {join_clause}
         WHERE t.is_completed = 1 AND t.days_to_complete IS NOT NULL
           AND t.completed_date_key >= ?1 AND t.completed_date_key <= ?2 {where_clause}
         ORDER BY t.days_to_complete"
    );
    let mut stmt = conn.prepare(&sql)?;
    stmt.raw_bind_parameter(1, start)?;
    stmt.raw_bind_parameter(2, end)?;
    bind_fn(&mut stmt, 3)?;

    let mut days = Vec::new();
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next()? {
        days.push(row.get::<_, i32>(0)?);
    }
    Ok(days)
}

fn compute_lead_time_sql(
    conn: &rusqlite::Connection,
    user_gid: Option<&str>,
    project_gid: Option<&str>,
    start: &str,
    end: &str,
) -> std::result::Result<LeadTimeMetrics, rusqlite::Error> {
    let days = compute_lead_time_raw(conn, user_gid, project_gid, start, end)?;
    Ok(percentiles_from_days(&days))
}

fn compute_collaboration_sql(
    conn: &rusqlite::Connection,
    user_gid: Option<&str>,
    project_gid: Option<&str>,
    start: &str,
    end: &str,
) -> std::result::Result<CollaborationMetrics, rusqlite::Error> {
    // Comments — use parameterized query for entity filter
    let (task_join, task_where, entity_val): (&str, &str, Option<&str>) =
        if let Some(pgid) = project_gid {
            (
                "JOIN bridge_task_projects btp ON btp.task_gid = c.task_gid",
                " AND btp.project_gid = ?3",
                Some(pgid),
            )
        } else if let Some(uid) = user_gid {
            (
                "JOIN fact_tasks t ON t.task_gid = c.task_gid",
                " AND t.assignee_gid = ?3",
                Some(uid),
            )
        } else {
            ("", "", None)
        };

    let sql = format!(
        "SELECT COUNT(*), COUNT(DISTINCT c.author_gid)
         FROM fact_comments c {task_join}
         WHERE c.created_date_key >= ?1 AND c.created_date_key <= ?2{task_where}"
    );
    let total_comments: i64;
    let unique_commenters: i64;
    {
        let mut stmt = conn.prepare(&sql)?;
        stmt.raw_bind_parameter(1, start)?;
        stmt.raw_bind_parameter(2, end)?;
        if let Some(val) = entity_val {
            stmt.raw_bind_parameter(3, val)?;
        }
        let mut rows = stmt.raw_query();
        let row = rows.next()?.unwrap();
        total_comments = row.get(0)?;
        unique_commenters = row.get(1)?;
    }

    // Likes
    let (like_join, like_where, like_val): (&str, &str, Option<&str>) =
        if let Some(pgid) = project_gid {
            (
                "JOIN bridge_task_projects btp ON btp.task_gid = t.task_gid",
                " AND btp.project_gid = ?3",
                Some(pgid),
            )
        } else if let Some(uid) = user_gid {
            ("", " AND t.assignee_gid = ?3", Some(uid))
        } else {
            ("", "", None)
        };

    let sql = format!(
        "SELECT COALESCE(SUM(t.num_likes), 0) FROM fact_tasks t {like_join}
         WHERE t.created_date_key >= ?1 AND t.created_date_key <= ?2{like_where}"
    );
    let mut stmt = conn.prepare(&sql)?;
    stmt.raw_bind_parameter(1, start)?;
    stmt.raw_bind_parameter(2, end)?;
    if let Some(val) = like_val {
        stmt.raw_bind_parameter(3, val)?;
    }
    let total_likes: i64 = stmt.raw_query().next()?.unwrap().get(0)?;

    Ok(CollaborationMetrics {
        total_comments: total_comments as u64,
        unique_commenters: unique_commenters as u64,
        total_likes: total_likes as u64,
    })
}

#[allow(clippy::type_complexity)]
fn build_entity_filter<'a>(
    user_gid: Option<&'a str>,
    project_gid: Option<&'a str>,
) -> (
    String,
    String,
    Box<dyn Fn(&mut rusqlite::Statement<'_>, usize) -> rusqlite::Result<()> + 'a>,
) {
    if let Some(pgid) = project_gid {
        (
            " AND btp.project_gid = ?3".to_string(),
            "JOIN bridge_task_projects btp ON btp.task_gid = t.task_gid".to_string(),
            Box::new(move |stmt: &mut rusqlite::Statement<'_>, idx: usize| {
                stmt.raw_bind_parameter(idx, pgid)?;
                Ok(())
            }),
        )
    } else if let Some(uid) = user_gid {
        (
            " AND t.assignee_gid = ?3".to_string(),
            String::new(),
            Box::new(move |stmt: &mut rusqlite::Statement<'_>, idx: usize| {
                stmt.raw_bind_parameter(idx, uid)?;
                Ok(())
            }),
        )
    } else {
        (
            String::new(),
            String::new(),
            Box::new(|_stmt: &mut rusqlite::Statement<'_>, _idx: usize| Ok(())),
        )
    }
}

fn get_portfolio_project_gids(
    conn: &rusqlite::Connection,
    portfolio_gid: &str,
) -> std::result::Result<Vec<String>, rusqlite::Error> {
    let mut stmt =
        conn.prepare("SELECT project_gid FROM bridge_portfolio_projects WHERE portfolio_gid = ?1")?;
    let gids: Vec<String> = stmt
        .query_map([portfolio_gid], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(gids)
}

fn percentiles_from_days(days: &[i32]) -> LeadTimeMetrics {
    if days.is_empty() {
        return LeadTimeMetrics::default();
    }

    let sum: i64 = days.iter().map(|&d| d as i64).sum();
    let avg = sum as f64 / days.len() as f64;

    #[allow(clippy::manual_is_multiple_of)]
    let median = if days.len() % 2 == 0 {
        let mid = days.len() / 2;
        (days[mid - 1] as f64 + days[mid] as f64) / 2.0
    } else {
        days[days.len() / 2] as f64
    };

    let p90_idx = ((days.len() as f64) * 0.9).ceil() as usize;
    let p90_idx = p90_idx.min(days.len()).max(1) - 1;
    let p90 = days[p90_idx] as f64;

    LeadTimeMetrics {
        avg_days_to_complete: Some(avg),
        median_days_to_complete: Some(median),
        p90_days_to_complete: Some(p90),
        min_days_to_complete: days.first().copied(),
        max_days_to_complete: days.last().copied(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    #[test]
    fn test_percentiles_empty() {
        let lt = percentiles_from_days(&[]);
        assert!(lt.avg_days_to_complete.is_none());
        assert!(lt.median_days_to_complete.is_none());
        assert!(lt.p90_days_to_complete.is_none());
    }

    #[test]
    fn test_percentiles_single_element() {
        // This was a panic bug — single element would underflow
        let lt = percentiles_from_days(&[5]);
        assert_eq!(lt.avg_days_to_complete, Some(5.0));
        assert_eq!(lt.median_days_to_complete, Some(5.0));
        assert_eq!(lt.p90_days_to_complete, Some(5.0));
        assert_eq!(lt.min_days_to_complete, Some(5));
        assert_eq!(lt.max_days_to_complete, Some(5));
    }

    #[test]
    fn test_percentiles_two_elements() {
        let lt = percentiles_from_days(&[3, 7]);
        assert_eq!(lt.avg_days_to_complete, Some(5.0));
        assert_eq!(lt.median_days_to_complete, Some(5.0)); // (3+7)/2
        assert_eq!(lt.min_days_to_complete, Some(3));
        assert_eq!(lt.max_days_to_complete, Some(7));
    }

    #[test]
    fn test_percentiles_many_elements() {
        let days: Vec<i32> = (1..=100).collect();
        let lt = percentiles_from_days(&days);
        assert_eq!(lt.avg_days_to_complete, Some(50.5));
        assert_eq!(lt.median_days_to_complete, Some(50.5)); // (50+51)/2
        assert_eq!(lt.p90_days_to_complete, Some(90.0));
        assert_eq!(lt.min_days_to_complete, Some(1));
        assert_eq!(lt.max_days_to_complete, Some(100));
    }

    #[test]
    fn test_percentiles_odd_count() {
        let lt = percentiles_from_days(&[1, 3, 5, 7, 9]);
        assert_eq!(lt.avg_days_to_complete, Some(5.0));
        assert_eq!(lt.median_days_to_complete, Some(5.0));
        assert_eq!(lt.min_days_to_complete, Some(1));
        assert_eq!(lt.max_days_to_complete, Some(9));
    }

    #[tokio::test]
    async fn test_compute_throughput_and_health() {
        let db = Database::open_memory().await.unwrap();

        // Insert a project and some tasks
        db.writer()
            .call(|conn| {
                conn.execute(
                    "INSERT INTO dim_projects (project_gid, name, workspace_gid, cached_at)
                     VALUES ('p1', 'Test Project', 'w1', datetime('now'))",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO dim_users (user_gid, name, cached_at)
                     VALUES ('u1', 'Test User', datetime('now'))",
                    [],
                )?;

                // Task created and completed in period
                conn.execute(
                    "INSERT INTO fact_tasks (task_gid, name, assignee_gid, is_completed, completed_at, completed_date_key, created_at, created_date_key, modified_at, is_subtask, days_to_complete, is_overdue, cached_at)
                     VALUES ('t1', 'Task 1', 'u1', 1, '2025-01-10', '2025-01-10', '2025-01-01', '2025-01-01', '2025-01-10', 0, 9, 0, datetime('now'))",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO bridge_task_projects (task_gid, project_gid) VALUES ('t1', 'p1')",
                    [],
                )?;

                // Open task with no assignee
                conn.execute(
                    "INSERT INTO fact_tasks (task_gid, name, is_completed, created_at, created_date_key, modified_at, is_subtask, is_overdue, cached_at)
                     VALUES ('t2', 'Task 2', 0, '2025-01-05', '2025-01-05', '2025-01-05', 0, 0, datetime('now'))",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO bridge_task_projects (task_gid, project_gid) VALUES ('t2', 'p1')",
                    [],
                )?;

                // Open overdue task
                conn.execute(
                    "INSERT INTO fact_tasks (task_gid, name, assignee_gid, is_completed, due_on, created_at, created_date_key, modified_at, is_subtask, is_overdue, cached_at)
                     VALUES ('t3', 'Task 3', 'u1', 0, '2024-12-01', '2025-01-01', '2025-01-01', '2025-01-01', 0, 1, datetime('now'))",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO bridge_task_projects (task_gid, project_gid) VALUES ('t3', 'p1')",
                    [],
                )?;

                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();

        let period = Period::Month(2025, 1);
        let metrics = compute_project_metrics(&db, "p1", &period).await.unwrap();

        // Throughput
        assert_eq!(metrics.throughput.tasks_completed, 1);
        assert_eq!(metrics.throughput.tasks_created, 3);

        // Health - 2 open tasks, 1 overdue, 1 unassigned
        assert_eq!(metrics.health.total_open, 2);
        assert_eq!(metrics.health.overdue_count, 1);
        assert_eq!(metrics.health.unassigned_count, 1);

        // Lead time - 1 completed task with 9 days
        assert_eq!(metrics.lead_time.avg_days_to_complete, Some(9.0));
    }

    #[tokio::test]
    async fn test_compute_user_metrics() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                conn.execute(
                    "INSERT INTO dim_users (user_gid, name, cached_at) VALUES ('u1', 'Alice', datetime('now'))",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO fact_tasks (task_gid, name, assignee_gid, is_completed, completed_at, completed_date_key, created_at, created_date_key, modified_at, is_subtask, days_to_complete, is_overdue, cached_at)
                     VALUES ('t1', 'Task 1', 'u1', 1, '2025-01-15', '2025-01-15', '2025-01-01', '2025-01-01', '2025-01-15', 0, 14, 0, datetime('now'))",
                    [],
                )?;
                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();

        let period = Period::Month(2025, 1);
        let metrics = compute_user_metrics(&db, "u1", &period).await.unwrap();
        assert_eq!(metrics.user_name, Some("Alice".to_string()));
        assert_eq!(metrics.throughput.tasks_completed, 1);
        assert_eq!(metrics.lead_time.avg_days_to_complete, Some(14.0));
    }
}
