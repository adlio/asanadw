use serde::{Deserialize, Serialize};

use crate::date_util::strip_code_fences;
use crate::error::{Error, Result};
use crate::query::period::Period;
use crate::storage::Database;

const PROMPT_VERSION: &str = "period-v1";

/// Structured period summary for a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPeriodSummary {
    pub headline: String,
    pub what_changed: String,
    pub why_it_matters: String,
    pub key_accomplishments: Vec<String>,
    pub collaboration_notes: Option<String>,
}

/// Structured period summary for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPeriodSummary {
    pub headline: String,
    pub what_changed: String,
    pub why_it_matters: String,
    pub key_milestones: Vec<String>,
    pub health_assessment: Option<String>,
}

/// Structured period summary for a portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioPeriodSummary {
    pub headline: String,
    pub what_changed: String,
    pub why_it_matters: String,
    pub key_milestones: Vec<String>,
    pub health_assessment: Option<String>,
}

/// Structured period summary for a team.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamPeriodSummary {
    pub headline: String,
    pub what_changed: String,
    pub why_it_matters: String,
    pub key_accomplishments: Vec<String>,
    pub health_assessment: Option<String>,
}

// ── User period summary ────────────────────────────────────────

pub async fn summarize_user_period(
    db: &Database,
    agent: &mixtape_core::Agent,
    user_gid: &str,
    period: &Period,
    force: bool,
) -> Result<UserPeriodSummary> {
    let period_key = period.to_key();

    if !force {
        if let Some(cached) = get_cached_user_summary(db, user_gid, &period_key).await? {
            return Ok(cached);
        }
    }

    let (start, end) = period.date_range();
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();

    let context = gather_user_period_context(db, user_gid, &start_str, &end_str).await?;
    let metrics = crate::metrics::compute_user_metrics(db, user_gid, period).await?;
    let metrics_json = serde_json::to_string_pretty(&metrics).unwrap_or_default();

    let prompt = format!(
        r#"Analyze this user's work during the period {period_key} and provide a structured summary as JSON.

Activity data:
{context}

Metrics:
{metrics_json}

Respond with ONLY a JSON object (no markdown, no code fences):
{{
  "headline": "One-sentence executive summary",
  "what_changed": "2-3 sentences describing what the user worked on and accomplished",
  "why_it_matters": "1-2 sentences on impact and significance",
  "key_accomplishments": ["list of 2-5 notable accomplishments"],
  "collaboration_notes": "Optional note on collaboration patterns, or null"
}}"#
    );

    let response = agent.run(&prompt).await.map_err(|e| Error::Llm(e.to_string()))?;
    let text = response.text().trim();
    let json_str = strip_code_fences(text);
    let summary: UserPeriodSummary = serde_json::from_str(json_str)
        .map_err(|e| Error::Llm(format!("Failed to parse LLM response: {e}\nResponse: {text}")))?;

    store_user_summary(db, user_gid, &period_key, &summary).await?;
    Ok(summary)
}

// ── Project period summary ─────────────────────────────────────

pub async fn summarize_project_period(
    db: &Database,
    agent: &mixtape_core::Agent,
    project_gid: &str,
    period: &Period,
    force: bool,
) -> Result<ProjectPeriodSummary> {
    let period_key = period.to_key();

    if !force {
        if let Some(cached) = get_cached_project_summary(db, project_gid, &period_key).await? {
            return Ok(cached);
        }
    }

    let (start, end) = period.date_range();
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();

    let context = gather_project_period_context(db, project_gid, &start_str, &end_str).await?;
    let metrics = crate::metrics::compute_project_metrics(db, project_gid, period).await?;
    let metrics_json = serde_json::to_string_pretty(&metrics).unwrap_or_default();

    let prompt = format!(
        r#"Analyze this project's progress during the period {period_key} and provide a structured summary as JSON.

Activity data:
{context}

Metrics:
{metrics_json}

Respond with ONLY a JSON object (no markdown, no code fences):
{{
  "headline": "One-sentence executive summary",
  "what_changed": "2-3 sentences describing what happened in the project",
  "why_it_matters": "1-2 sentences on impact and significance",
  "key_milestones": ["list of 2-5 notable milestones or events"],
  "health_assessment": "Brief assessment of project health, or null"
}}"#
    );

    let response = agent.run(&prompt).await.map_err(|e| Error::Llm(e.to_string()))?;
    let text = response.text().trim();
    let json_str = strip_code_fences(text);
    let summary: ProjectPeriodSummary = serde_json::from_str(json_str)
        .map_err(|e| Error::Llm(format!("Failed to parse LLM response: {e}\nResponse: {text}")))?;

    store_project_summary(db, project_gid, &period_key, &summary).await?;
    Ok(summary)
}

// ── Portfolio period summary ───────────────────────────────────

pub async fn summarize_portfolio_period(
    db: &Database,
    agent: &mixtape_core::Agent,
    portfolio_gid: &str,
    period: &Period,
    force: bool,
) -> Result<PortfolioPeriodSummary> {
    let period_key = period.to_key();

    if !force {
        if let Some(cached) = get_cached_portfolio_summary(db, portfolio_gid, &period_key).await? {
            return Ok(cached);
        }
    }

    let (start, end) = period.date_range();
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();

    let context = gather_portfolio_period_context(db, portfolio_gid, &start_str, &end_str).await?;
    let metrics = crate::metrics::compute_portfolio_metrics(db, portfolio_gid, period).await?;
    let metrics_json = serde_json::to_string_pretty(&metrics).unwrap_or_default();

    let prompt = format!(
        r#"Analyze this portfolio's progress during the period {period_key} and provide a structured summary as JSON.

Activity data:
{context}

Metrics:
{metrics_json}

Respond with ONLY a JSON object (no markdown, no code fences):
{{
  "headline": "One-sentence executive summary",
  "what_changed": "2-3 sentences describing what happened across the portfolio",
  "why_it_matters": "1-2 sentences on impact and significance",
  "key_milestones": ["list of 2-5 notable milestones across projects"],
  "health_assessment": "Brief assessment of portfolio health, or null"
}}"#
    );

    let response = agent.run(&prompt).await.map_err(|e| Error::Llm(e.to_string()))?;
    let text = response.text().trim();
    let json_str = strip_code_fences(text);
    let summary: PortfolioPeriodSummary = serde_json::from_str(json_str)
        .map_err(|e| Error::Llm(format!("Failed to parse LLM response: {e}\nResponse: {text}")))?;

    store_portfolio_summary(db, portfolio_gid, &period_key, &summary).await?;
    Ok(summary)
}

// ── Team period summary ────────────────────────────────────────

pub async fn summarize_team_period(
    db: &Database,
    agent: &mixtape_core::Agent,
    team_gid: &str,
    period: &Period,
    force: bool,
) -> Result<TeamPeriodSummary> {
    let period_key = period.to_key();

    if !force {
        if let Some(cached) = get_cached_team_summary(db, team_gid, &period_key).await? {
            return Ok(cached);
        }
    }

    let (start, end) = period.date_range();
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();

    let context = gather_team_period_context(db, team_gid, &start_str, &end_str).await?;
    let metrics = crate::metrics::compute_team_metrics(db, team_gid, period).await?;
    let metrics_json = serde_json::to_string_pretty(&metrics).unwrap_or_default();

    let prompt = format!(
        r#"Analyze this team's work during the period {period_key} and provide a structured summary as JSON.

Activity data:
{context}

Metrics:
{metrics_json}

Respond with ONLY a JSON object (no markdown, no code fences):
{{
  "headline": "One-sentence executive summary",
  "what_changed": "2-3 sentences describing what the team accomplished",
  "why_it_matters": "1-2 sentences on impact and significance",
  "key_accomplishments": ["list of 2-5 notable team accomplishments"],
  "health_assessment": "Brief assessment of team health and workload, or null"
}}"#
    );

    let response = agent.run(&prompt).await.map_err(|e| Error::Llm(e.to_string()))?;
    let text = response.text().trim();
    let json_str = strip_code_fences(text);
    let summary: TeamPeriodSummary = serde_json::from_str(json_str)
        .map_err(|e| Error::Llm(format!("Failed to parse LLM response: {e}\nResponse: {text}")))?;

    store_team_summary(db, team_gid, &period_key, &summary).await?;
    Ok(summary)
}

// ── Context gathering ──────────────────────────────────────────

async fn gather_user_period_context(
    db: &Database,
    user_gid: &str,
    start: &str,
    end: &str,
) -> Result<String> {
    let user_gid = user_gid.to_string();
    let start = start.to_string();
    let end = end.to_string();
    db.reader()
        .call(move |conn| {
            let mut parts = Vec::new();

            // User name
            let name: Option<String> = conn
                .query_row("SELECT name FROM dim_users WHERE user_gid = ?1", [&user_gid], |row| row.get(0))
                .ok();
            parts.push(format!("User: {}", name.unwrap_or_else(|| user_gid.clone())));

            // Tasks completed in period
            let mut stmt = conn.prepare(
                "SELECT name, completed_at, days_to_complete FROM fact_tasks
                 WHERE assignee_gid = ?1 AND is_completed = 1
                   AND completed_date_key >= ?2 AND completed_date_key <= ?3
                 ORDER BY completed_at DESC LIMIT 50"
            )?;
            let completed: Vec<(String, Option<String>, Option<i32>)> = stmt
                .query_map(rusqlite::params![user_gid, start, end], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .filter_map(|r| r.ok())
                .collect();
            if !completed.is_empty() {
                parts.push(format!("\nCompleted tasks ({}):", completed.len()));
                for (name, at, days) in &completed {
                    let d = days.map(|d| format!(" ({d}d)")).unwrap_or_default();
                    parts.push(format!("  - {name} [{}]{d}", at.as_deref().unwrap_or("?")));
                }
            }

            // Tasks created in period
            let mut stmt = conn.prepare(
                "SELECT name, due_on FROM fact_tasks
                 WHERE assignee_gid = ?1 AND created_date_key >= ?2 AND created_date_key <= ?3
                 ORDER BY created_at DESC LIMIT 30"
            )?;
            let created: Vec<(String, Option<String>)> = stmt
                .query_map(rusqlite::params![user_gid, start, end], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            if !created.is_empty() {
                parts.push(format!("\nTasks created ({}):", created.len()));
                for (name, due) in &created {
                    let d = due.as_deref().unwrap_or("no due date");
                    parts.push(format!("  - {name} (due: {d})"));
                }
            }

            Ok::<String, rusqlite::Error>(parts.join("\n"))
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn gather_project_period_context(
    db: &Database,
    project_gid: &str,
    start: &str,
    end: &str,
) -> Result<String> {
    let project_gid = project_gid.to_string();
    let start = start.to_string();
    let end = end.to_string();
    db.reader()
        .call(move |conn| {
            let mut parts = Vec::new();

            let name: Option<String> = conn
                .query_row("SELECT name FROM dim_projects WHERE project_gid = ?1", [&project_gid], |row| row.get(0))
                .ok();
            parts.push(format!("Project: {}", name.unwrap_or_else(|| project_gid.clone())));

            // Completed tasks
            let mut stmt = conn.prepare(
                "SELECT t.name, u.name, t.completed_at FROM fact_tasks t
                 JOIN bridge_task_projects btp ON btp.task_gid = t.task_gid
                 LEFT JOIN dim_users u ON u.user_gid = t.assignee_gid
                 WHERE btp.project_gid = ?1 AND t.is_completed = 1
                   AND t.completed_date_key >= ?2 AND t.completed_date_key <= ?3
                 ORDER BY t.completed_at DESC LIMIT 50"
            )?;
            let completed: Vec<(String, Option<String>, Option<String>)> = stmt
                .query_map(rusqlite::params![project_gid, start, end], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .filter_map(|r| r.ok())
                .collect();
            if !completed.is_empty() {
                parts.push(format!("\nCompleted tasks ({}):", completed.len()));
                for (name, assignee, at) in &completed {
                    let a = assignee.as_deref().unwrap_or("unassigned");
                    parts.push(format!("  - {name} ({a}) [{}]", at.as_deref().unwrap_or("?")));
                }
            }

            // Open tasks
            let mut stmt = conn.prepare(
                "SELECT t.name, u.name, t.due_on, t.is_overdue FROM fact_tasks t
                 JOIN bridge_task_projects btp ON btp.task_gid = t.task_gid
                 LEFT JOIN dim_users u ON u.user_gid = t.assignee_gid
                 WHERE btp.project_gid = ?1 AND t.is_completed = 0
                 ORDER BY t.due_on ASC LIMIT 30"
            )?;
            let open: Vec<(String, Option<String>, Option<String>, i32)> = stmt
                .query_map([&project_gid], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
                .filter_map(|r| r.ok())
                .collect();
            if !open.is_empty() {
                parts.push(format!("\nOpen tasks ({}):", open.len()));
                for (name, assignee, due, overdue) in &open {
                    let a = assignee.as_deref().unwrap_or("unassigned");
                    let d = due.as_deref().unwrap_or("no due date");
                    let flag = if *overdue != 0 { " [OVERDUE]" } else { "" };
                    parts.push(format!("  - {name} ({a}) due: {d}{flag}"));
                }
            }

            Ok::<String, rusqlite::Error>(parts.join("\n"))
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn gather_portfolio_period_context(
    db: &Database,
    portfolio_gid: &str,
    start: &str,
    end: &str,
) -> Result<String> {
    let portfolio_gid = portfolio_gid.to_string();
    let start = start.to_string();
    let end = end.to_string();
    db.reader()
        .call(move |conn| {
            let mut parts = Vec::new();

            let name: Option<String> = conn
                .query_row("SELECT name FROM dim_portfolios WHERE portfolio_gid = ?1", [&portfolio_gid], |row| row.get(0))
                .ok();
            parts.push(format!("Portfolio: {}", name.unwrap_or_else(|| portfolio_gid.clone())));

            // List projects with summary stats
            let mut stmt = conn.prepare(
                "SELECT p.project_gid, p.name,
                        (SELECT COUNT(*) FROM fact_tasks t JOIN bridge_task_projects btp ON btp.task_gid = t.task_gid WHERE btp.project_gid = p.project_gid AND t.is_completed = 1 AND t.completed_date_key >= ?2 AND t.completed_date_key <= ?3),
                        (SELECT COUNT(*) FROM fact_tasks t JOIN bridge_task_projects btp ON btp.task_gid = t.task_gid WHERE btp.project_gid = p.project_gid AND t.is_completed = 0)
                 FROM bridge_portfolio_projects bpp
                 JOIN dim_projects p ON p.project_gid = bpp.project_gid
                 WHERE bpp.portfolio_gid = ?1"
            )?;
            let projects: Vec<(String, String, i64, i64)> = stmt
                .query_map(rusqlite::params![portfolio_gid, start, end], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
                .filter_map(|r| r.ok())
                .collect();
            if !projects.is_empty() {
                parts.push(format!("\nProjects ({}):", projects.len()));
                for (_gid, name, completed, open) in &projects {
                    parts.push(format!("  - {name}: {completed} completed, {open} open"));
                }
            }

            Ok::<String, rusqlite::Error>(parts.join("\n"))
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn gather_team_period_context(
    db: &Database,
    team_gid: &str,
    start: &str,
    end: &str,
) -> Result<String> {
    let team_gid = team_gid.to_string();
    let start = start.to_string();
    let end = end.to_string();
    db.reader()
        .call(move |conn| {
            let mut parts = Vec::new();

            let name: Option<String> = conn
                .query_row("SELECT name FROM dim_teams WHERE team_gid = ?1", [&team_gid], |row| row.get(0))
                .ok();
            parts.push(format!("Team: {}", name.unwrap_or_else(|| team_gid.clone())));

            // Members with task counts
            let mut stmt = conn.prepare(
                "SELECT u.name, u.user_gid,
                        (SELECT COUNT(*) FROM fact_tasks t WHERE t.assignee_gid = u.user_gid AND t.is_completed = 1 AND t.completed_date_key >= ?2 AND t.completed_date_key <= ?3),
                        (SELECT COUNT(*) FROM fact_tasks t WHERE t.assignee_gid = u.user_gid AND t.is_completed = 0)
                 FROM bridge_team_members btm
                 JOIN dim_users u ON u.user_gid = btm.user_gid
                 WHERE btm.team_gid = ?1"
            )?;
            let members: Vec<(Option<String>, String, i64, i64)> = stmt
                .query_map(rusqlite::params![team_gid, start, end], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
                .filter_map(|r| r.ok())
                .collect();
            if !members.is_empty() {
                parts.push(format!("\nMembers ({}):", members.len()));
                for (name, _gid, completed, open) in &members {
                    let n = name.as_deref().unwrap_or("unknown");
                    parts.push(format!("  - {n}: {completed} completed, {open} open"));
                }
            }

            Ok::<String, rusqlite::Error>(parts.join("\n"))
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

// ── Cache operations ───────────────────────────────────────────

async fn get_cached_user_summary(db: &Database, user_gid: &str, period_key: &str) -> Result<Option<UserPeriodSummary>> {
    let user_gid = user_gid.to_string();
    let period_key = period_key.to_string();
    db.reader()
        .call(move |conn| {
            let result = conn
                .query_row(
                    "SELECT headline, what_changed, why_it_matters, key_accomplishments, collaboration_notes
                     FROM fact_user_period_summaries WHERE user_gid = ?1 AND period_key = ?2 AND prompt_version = ?3",
                    rusqlite::params![user_gid, period_key, PROMPT_VERSION],
                    |row| {
                        let accomplishments_str: String = row.get(3)?;
                        Ok(UserPeriodSummary {
                            headline: row.get(0)?,
                            what_changed: row.get(1)?,
                            why_it_matters: row.get(2)?,
                            key_accomplishments: serde_json::from_str(&accomplishments_str).unwrap_or_default(),
                            collaboration_notes: row.get(4)?,
                        })
                    },
                )
                .ok();
            Ok::<Option<UserPeriodSummary>, rusqlite::Error>(result)
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn store_user_summary(db: &Database, user_gid: &str, period_key: &str, summary: &UserPeriodSummary) -> Result<()> {
    let user_gid = user_gid.to_string();
    let period_key = period_key.to_string();
    let headline = summary.headline.clone();
    let what_changed = summary.what_changed.clone();
    let why_it_matters = summary.why_it_matters.clone();
    let key_accomplishments = serde_json::to_string(&summary.key_accomplishments).unwrap_or_default();
    let collaboration_notes = summary.collaboration_notes.clone();

    db.writer()
        .call(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO fact_user_period_summaries
                 (user_gid, period_key, headline, what_changed, why_it_matters, key_accomplishments, collaboration_notes, prompt_version, generated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
                rusqlite::params![user_gid, period_key, headline, what_changed, why_it_matters, key_accomplishments, collaboration_notes, PROMPT_VERSION],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn get_cached_project_summary(db: &Database, project_gid: &str, period_key: &str) -> Result<Option<ProjectPeriodSummary>> {
    let project_gid = project_gid.to_string();
    let period_key = period_key.to_string();
    db.reader()
        .call(move |conn| {
            let result = conn
                .query_row(
                    "SELECT headline, what_changed, why_it_matters, key_milestones, health_assessment
                     FROM fact_project_period_summaries WHERE project_gid = ?1 AND period_key = ?2 AND prompt_version = ?3",
                    rusqlite::params![project_gid, period_key, PROMPT_VERSION],
                    |row| {
                        let milestones_str: String = row.get(3)?;
                        Ok(ProjectPeriodSummary {
                            headline: row.get(0)?,
                            what_changed: row.get(1)?,
                            why_it_matters: row.get(2)?,
                            key_milestones: serde_json::from_str(&milestones_str).unwrap_or_default(),
                            health_assessment: row.get(4)?,
                        })
                    },
                )
                .ok();
            Ok::<Option<ProjectPeriodSummary>, rusqlite::Error>(result)
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn store_project_summary(db: &Database, project_gid: &str, period_key: &str, summary: &ProjectPeriodSummary) -> Result<()> {
    let project_gid = project_gid.to_string();
    let period_key = period_key.to_string();
    let headline = summary.headline.clone();
    let what_changed = summary.what_changed.clone();
    let why_it_matters = summary.why_it_matters.clone();
    let key_milestones = serde_json::to_string(&summary.key_milestones).unwrap_or_default();
    let health_assessment = summary.health_assessment.clone();

    db.writer()
        .call(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO fact_project_period_summaries
                 (project_gid, period_key, headline, what_changed, why_it_matters, key_milestones, health_assessment, prompt_version, generated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
                rusqlite::params![project_gid, period_key, headline, what_changed, why_it_matters, key_milestones, health_assessment, PROMPT_VERSION],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn get_cached_portfolio_summary(db: &Database, portfolio_gid: &str, period_key: &str) -> Result<Option<PortfolioPeriodSummary>> {
    let portfolio_gid = portfolio_gid.to_string();
    let period_key = period_key.to_string();
    db.reader()
        .call(move |conn| {
            let result = conn
                .query_row(
                    "SELECT headline, what_changed, why_it_matters, key_milestones, health_assessment
                     FROM fact_portfolio_period_summaries WHERE portfolio_gid = ?1 AND period_key = ?2 AND prompt_version = ?3",
                    rusqlite::params![portfolio_gid, period_key, PROMPT_VERSION],
                    |row| {
                        let milestones_str: String = row.get(3)?;
                        Ok(PortfolioPeriodSummary {
                            headline: row.get(0)?,
                            what_changed: row.get(1)?,
                            why_it_matters: row.get(2)?,
                            key_milestones: serde_json::from_str(&milestones_str).unwrap_or_default(),
                            health_assessment: row.get(4)?,
                        })
                    },
                )
                .ok();
            Ok::<Option<PortfolioPeriodSummary>, rusqlite::Error>(result)
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn store_portfolio_summary(db: &Database, portfolio_gid: &str, period_key: &str, summary: &PortfolioPeriodSummary) -> Result<()> {
    let portfolio_gid = portfolio_gid.to_string();
    let period_key = period_key.to_string();
    let headline = summary.headline.clone();
    let what_changed = summary.what_changed.clone();
    let why_it_matters = summary.why_it_matters.clone();
    let key_milestones = serde_json::to_string(&summary.key_milestones).unwrap_or_default();
    let health_assessment = summary.health_assessment.clone();

    db.writer()
        .call(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO fact_portfolio_period_summaries
                 (portfolio_gid, period_key, headline, what_changed, why_it_matters, key_milestones, health_assessment, prompt_version, generated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
                rusqlite::params![portfolio_gid, period_key, headline, what_changed, why_it_matters, key_milestones, health_assessment, PROMPT_VERSION],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn get_cached_team_summary(db: &Database, team_gid: &str, period_key: &str) -> Result<Option<TeamPeriodSummary>> {
    let team_gid = team_gid.to_string();
    let period_key = period_key.to_string();
    db.reader()
        .call(move |conn| {
            let result = conn
                .query_row(
                    "SELECT headline, what_changed, why_it_matters, key_accomplishments, health_assessment
                     FROM fact_team_period_summaries WHERE team_gid = ?1 AND period_key = ?2 AND prompt_version = ?3",
                    rusqlite::params![team_gid, period_key, PROMPT_VERSION],
                    |row| {
                        let accomplishments_str: String = row.get(3)?;
                        Ok(TeamPeriodSummary {
                            headline: row.get(0)?,
                            what_changed: row.get(1)?,
                            why_it_matters: row.get(2)?,
                            key_accomplishments: serde_json::from_str(&accomplishments_str).unwrap_or_default(),
                            health_assessment: row.get(4)?,
                        })
                    },
                )
                .ok();
            Ok::<Option<TeamPeriodSummary>, rusqlite::Error>(result)
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn store_team_summary(db: &Database, team_gid: &str, period_key: &str, summary: &TeamPeriodSummary) -> Result<()> {
    let team_gid = team_gid.to_string();
    let period_key = period_key.to_string();
    let headline = summary.headline.clone();
    let what_changed = summary.what_changed.clone();
    let why_it_matters = summary.why_it_matters.clone();
    let key_accomplishments = serde_json::to_string(&summary.key_accomplishments).unwrap_or_default();
    let health_assessment = summary.health_assessment.clone();

    db.writer()
        .call(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO fact_team_period_summaries
                 (team_gid, period_key, headline, what_changed, why_it_matters, key_accomplishments, health_assessment, prompt_version, generated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
                rusqlite::params![team_gid, period_key, headline, what_changed, why_it_matters, key_accomplishments, health_assessment, PROMPT_VERSION],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

