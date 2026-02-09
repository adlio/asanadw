use serde::{Deserialize, Serialize};

use crate::date_util::strip_code_fences;
use crate::error::{Error, Result};
use crate::storage::Database;

const PROMPT_VERSION: &str = "task-v1";

/// Structured summary of a task from LLM analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub headline: String,
    pub what_happened: String,
    pub why_it_matters: String,
    pub complexity_signal: String,
    pub notability_score: i32,
    pub change_types: Vec<String>,
}

/// Summarize a task using the LLM. Caches results in fact_task_summaries.
pub async fn summarize_task(
    db: &Database,
    agent: &mixtape_core::Agent,
    task_gid: &str,
    force: bool,
) -> Result<TaskSummary> {
    // Check cache first
    if !force {
        let cached = get_cached_summary(db, task_gid).await?;
        if let Some(summary) = cached {
            return Ok(summary);
        }
    }

    // Gather task data
    let context = gather_task_context(db, task_gid).await?;

    let prompt = format!(
        r#"Analyze this Asana task and provide a structured summary as JSON.

Task data:
{context}

Respond with ONLY a JSON object (no markdown, no code fences) in this exact format:
{{
  "headline": "One-sentence summary of the task",
  "what_happened": "2-3 sentences describing what the task involves and its current state",
  "why_it_matters": "1-2 sentences on the significance or impact",
  "complexity_signal": "low|medium|high",
  "notability_score": <1-10 integer>,
  "change_types": ["list", "of", "relevant", "labels"]
}}

For change_types, use labels like: "feature", "bug", "design", "documentation", "infrastructure", "planning", "review", "discussion", "milestone", "blocked", "recurring"."#
    );

    let response = agent
        .run(&prompt)
        .await
        .map_err(|e| Error::Llm(e.to_string()))?;

    let text = response.text().trim();

    // Parse JSON from response (strip markdown fences if present)
    let json_str = strip_code_fences(text);
    let summary: TaskSummary = serde_json::from_str(json_str).map_err(|e| {
        Error::Llm(format!(
            "Failed to parse LLM response: {e}\nResponse: {text}"
        ))
    })?;

    // Cache the result
    store_summary(db, task_gid, &summary).await?;

    Ok(summary)
}

async fn get_cached_summary(db: &Database, task_gid: &str) -> Result<Option<TaskSummary>> {
    let task_gid = task_gid.to_string();
    db.reader()
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT headline, what_happened, why_it_matters, complexity_signal, notability_score, change_types
                 FROM fact_task_summaries WHERE task_gid = ?1 AND prompt_version = ?2"
            )?;
            let result = stmt
                .query_row(rusqlite::params![task_gid, PROMPT_VERSION], |row| {
                    let change_types_str: String = row.get(5)?;
                    let change_types: Vec<String> =
                        serde_json::from_str(&change_types_str).unwrap_or_default();
                    Ok(TaskSummary {
                        headline: row.get(0)?,
                        what_happened: row.get(1)?,
                        why_it_matters: row.get(2)?,
                        complexity_signal: row.get(3)?,
                        notability_score: row.get(4)?,
                        change_types,
                    })
                })
                .ok();
            Ok::<Option<TaskSummary>, rusqlite::Error>(result)
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn gather_task_context(db: &Database, task_gid: &str) -> Result<String> {
    let task_gid = task_gid.to_string();
    db.reader()
        .call(move |conn| {
            let mut parts = Vec::new();

            // Task details
            #[allow(clippy::type_complexity)]
            let task: Option<(String, Option<String>, i32, Option<String>, Option<String>, Option<String>, Option<String>)> = conn
                .query_row(
                    "SELECT name, notes, is_completed, completed_at, due_on, assignee_gid, created_at FROM fact_tasks WHERE task_gid = ?1",
                    [&task_gid],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
                )
                .ok();

            if let Some((name, notes, completed, completed_at, due_on, assignee_gid, created_at)) = task {
                parts.push(format!("Name: {name}"));
                parts.push(format!("Status: {}", if completed != 0 { "completed" } else { "open" }));
                if let Some(c) = completed_at {
                    parts.push(format!("Completed: {c}"));
                }
                if let Some(d) = due_on {
                    parts.push(format!("Due: {d}"));
                }
                if let Some(c) = created_at {
                    parts.push(format!("Created: {c}"));
                }
                if let Some(ref uid) = assignee_gid {
                    let name: Option<String> = conn
                        .query_row("SELECT name FROM dim_users WHERE user_gid = ?1", [uid], |row| row.get(0))
                        .ok();
                    parts.push(format!("Assignee: {}", name.unwrap_or_else(|| uid.clone())));
                }
                if let Some(n) = notes {
                    if !n.is_empty() {
                        parts.push(format!("Notes: {}", truncate(&n, 2000)));
                    }
                }
            }

            // Custom fields
            let mut stmt = conn.prepare(
                "SELECT cf.name, tcf.display_value FROM fact_task_custom_fields tcf
                 JOIN dim_custom_fields cf ON cf.field_gid = tcf.field_gid
                 WHERE tcf.task_gid = ?1"
            )?;
            let fields: Vec<(String, String)> = stmt
                .query_map([&task_gid], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            if !fields.is_empty() {
                parts.push("Custom Fields:".to_string());
                for (name, val) in &fields {
                    parts.push(format!("  {name}: {val}"));
                }
            }

            // Comments
            let mut stmt = conn.prepare(
                "SELECT u.name, c.text, c.created_at FROM fact_comments c
                 LEFT JOIN dim_users u ON u.user_gid = c.author_gid
                 WHERE c.task_gid = ?1 AND c.story_type = 'comment'
                 ORDER BY c.created_at"
            )?;
            let comments: Vec<(Option<String>, Option<String>, String)> = stmt
                .query_map([&task_gid], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .filter_map(|r| r.ok())
                .collect();
            if !comments.is_empty() {
                parts.push(format!("\nComments ({}):", comments.len()));
                for (author, text, created_at) in &comments {
                    let a = author.as_deref().unwrap_or("unknown");
                    let t = text.as_deref().unwrap_or("");
                    parts.push(format!("  [{created_at}] {a}: {}", truncate(t, 500)));
                }
            }

            // Project membership
            let mut stmt = conn.prepare(
                "SELECT p.name, s.name FROM bridge_task_projects btp
                 JOIN dim_projects p ON p.project_gid = btp.project_gid
                 LEFT JOIN dim_sections s ON s.section_gid = btp.section_gid
                 WHERE btp.task_gid = ?1"
            )?;
            let memberships: Vec<(String, Option<String>)> = stmt
                .query_map([&task_gid], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            if !memberships.is_empty() {
                parts.push("Projects:".to_string());
                for (proj, section) in &memberships {
                    match section {
                        Some(s) => parts.push(format!("  {proj} / {s}")),
                        None => parts.push(format!("  {proj}")),
                    }
                }
            }

            Ok::<String, rusqlite::Error>(parts.join("\n"))
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn store_summary(db: &Database, task_gid: &str, summary: &TaskSummary) -> Result<()> {
    let task_gid = task_gid.to_string();
    let headline = summary.headline.clone();
    let what_happened = summary.what_happened.clone();
    let why_it_matters = summary.why_it_matters.clone();
    let complexity_signal = summary.complexity_signal.clone();
    let notability_score = summary.notability_score;
    let change_types = serde_json::to_string(&summary.change_types).unwrap_or_default();

    db.writer()
        .call(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO fact_task_summaries
                 (task_gid, headline, what_happened, why_it_matters, complexity_signal, notability_score, change_types, prompt_version, generated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
                rusqlite::params![
                    task_gid,
                    headline,
                    what_happened,
                    why_it_matters,
                    complexity_signal,
                    notability_score,
                    change_types,
                    PROMPT_VERSION
                ],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
        &s[..end]
    }
}
