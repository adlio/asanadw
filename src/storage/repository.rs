use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

// ── Users ──────────────────────────────────────────────────────────

pub fn upsert_user(conn: &Connection, user: &asanaclient::User) -> Result<(), rusqlite::Error> {
    let photo_url = user
        .photo
        .as_ref()
        .and_then(|p| p.image_128x128.as_deref().or(p.image_60x60.as_deref()));
    conn.execute(
        "INSERT OR REPLACE INTO dim_users (user_gid, email, name, photo_url, cached_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'))",
        params![user.gid, user.email, user.name, photo_url],
    )?;
    Ok(())
}

/// Minimal user upsert — inserts if user doesn't exist, updates email/name if they were previously NULL.
/// Use this when you only have a GID, name, and optionally email (e.g., from task assignee references).
pub fn upsert_user_minimal(
    conn: &Connection,
    user_gid: &str,
    name: Option<&str>,
) -> Result<(), rusqlite::Error> {
    upsert_user_minimal_with_email(conn, user_gid, name, None)
}

/// Like `upsert_user_minimal` but also stores the email if provided.
pub fn upsert_user_minimal_with_email(
    conn: &Connection,
    user_gid: &str,
    name: Option<&str>,
    email: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO dim_users (user_gid, name, email, cached_at)
         VALUES (?1, ?2, ?3, datetime('now'))
         ON CONFLICT(user_gid) DO UPDATE SET
           email = COALESCE(excluded.email, dim_users.email),
           name = CASE WHEN excluded.name != '' THEN excluded.name ELSE dim_users.name END",
        params![user_gid, name.unwrap_or(""), email],
    )?;
    Ok(())
}

/// Resolve a user identifier to a GID.
/// If the input is already a numeric GID, returns it as-is.
/// Otherwise, looks up the email column in dim_users.
/// Returns None if no match is found.
pub fn resolve_user_identifier(
    conn: &Connection,
    identifier: &str,
) -> Result<Option<String>, rusqlite::Error> {
    // Already a numeric GID
    if !identifier.is_empty() && identifier.chars().all(|c| c.is_ascii_digit()) {
        return Ok(Some(identifier.to_string()));
    }
    // Try email lookup
    let gid: Option<String> = conn
        .query_row(
            "SELECT user_gid FROM dim_users WHERE email = ?1",
            params![identifier],
            |row| row.get(0),
        )
        .optional()?;
    Ok(gid)
}

// ── Projects ───────────────────────────────────────────────────────

pub fn upsert_project(
    conn: &Connection,
    project: &asanaclient::Project,
) -> Result<(), rusqlite::Error> {
    let owner_gid = project.owner.as_ref().map(|o| o.gid.as_str());
    let team_gid = project.team.as_ref().map(|t| t.gid.as_str());
    let workspace_gid = project
        .workspace
        .as_ref()
        .map(|w| w.gid.as_str())
        .unwrap_or("");

    conn.execute(
        "INSERT INTO dim_projects (
            project_gid, name, owner_gid, team_gid, workspace_gid,
            is_archived, is_template, color, notes, notes_html,
            created_at, modified_at, permalink_url, cached_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, datetime('now'))
        ON CONFLICT(project_gid) DO UPDATE SET
            name=excluded.name, owner_gid=excluded.owner_gid, team_gid=excluded.team_gid,
            workspace_gid=excluded.workspace_gid, is_archived=excluded.is_archived,
            is_template=excluded.is_template, color=excluded.color, notes=excluded.notes,
            notes_html=excluded.notes_html, created_at=excluded.created_at,
            modified_at=excluded.modified_at, permalink_url=excluded.permalink_url,
            cached_at=excluded.cached_at",
        params![
            project.gid,
            project.name,
            owner_gid,
            team_gid,
            workspace_gid,
            project.archived as i32,
            project.is_template as i32,
            project.color,
            project.notes,
            project.html_notes,
            project.created_at,
            project.modified_at,
            project.permalink_url,
        ],
    )?;
    Ok(())
}

// ── Tasks ──────────────────────────────────────────────────────────

pub fn upsert_task(conn: &Connection, task: &asanaclient::Task) -> Result<(), rusqlite::Error> {
    let assignee_gid = task.assignee.as_ref().map(|a| a.gid.as_str());
    let parent_gid = task.parent.as_ref().map(|p| p.gid.as_str());
    let is_subtask = parent_gid.is_some();

    let created_at = task.created_at.as_deref().unwrap_or("");
    let created_date_key = date_key_from_iso(created_at);
    let completed_date_key = task.completed_at.as_deref().map(date_key_from_iso);

    let days_to_complete = compute_days_to_complete(created_at, task.completed_at.as_deref());
    let is_overdue = compute_is_overdue(task.completed, task.due_on.as_deref());

    conn.execute(
        "INSERT INTO fact_tasks (
            task_gid, name, notes, notes_html, assignee_gid,
            is_completed, completed_at, completed_date_key,
            due_on, due_at, start_on, start_at,
            created_at, created_date_key, modified_at,
            parent_gid, is_subtask, num_subtasks, num_likes,
            days_to_complete, is_overdue, permalink_url, cached_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
            ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, datetime('now')
        )
        ON CONFLICT(task_gid) DO UPDATE SET
            name=excluded.name, notes=excluded.notes, notes_html=excluded.notes_html,
            assignee_gid=excluded.assignee_gid, is_completed=excluded.is_completed,
            completed_at=excluded.completed_at, completed_date_key=excluded.completed_date_key,
            due_on=excluded.due_on, due_at=excluded.due_at, start_on=excluded.start_on,
            start_at=excluded.start_at, created_at=excluded.created_at,
            created_date_key=excluded.created_date_key, modified_at=excluded.modified_at,
            parent_gid=excluded.parent_gid, is_subtask=excluded.is_subtask,
            num_subtasks=excluded.num_subtasks, num_likes=excluded.num_likes,
            days_to_complete=excluded.days_to_complete, is_overdue=excluded.is_overdue,
            permalink_url=excluded.permalink_url, cached_at=excluded.cached_at",
        params![
            task.gid,
            task.name,
            task.notes,
            task.html_notes,
            assignee_gid,
            task.completed as i32,
            task.completed_at,
            completed_date_key,
            task.due_on,
            task.due_at,
            task.start_on,
            task.start_at,
            created_at,
            created_date_key,
            task.modified_at,
            parent_gid,
            is_subtask as i32,
            task.num_subtasks,
            task.num_likes,
            days_to_complete,
            is_overdue as i32,
            task.permalink_url,
        ],
    )?;

    // Clean stale bridge rows (previously handled by CASCADE from INSERT OR REPLACE)
    conn.execute(
        "DELETE FROM bridge_task_projects WHERE task_gid = ?1",
        params![task.gid],
    )?;
    conn.execute(
        "DELETE FROM bridge_task_tags WHERE task_gid = ?1",
        params![task.gid],
    )?;
    conn.execute(
        "DELETE FROM fact_task_custom_fields WHERE task_gid = ?1",
        params![task.gid],
    )?;

    // Insert task memberships (project associations)
    for membership in &task.memberships {
        let section_gid = membership.section.as_ref().map(|s| s.gid.as_str());
        conn.execute(
            "INSERT INTO bridge_task_projects (task_gid, project_gid, section_gid)
             VALUES (?1, ?2, ?3)",
            params![task.gid, membership.project.gid, section_gid],
        )?;
    }

    // Insert tags
    for tag in &task.tags {
        let tag_name = tag.name.as_deref().unwrap_or("");
        conn.execute(
            "INSERT INTO bridge_task_tags (task_gid, tag_gid, tag_name)
             VALUES (?1, ?2, ?3)",
            params![task.gid, tag.gid, tag_name],
        )?;
    }

    // Insert custom fields
    upsert_custom_fields(conn, &task.gid, &task.custom_fields)?;

    Ok(())
}

// ── Custom Fields ──────────────────────────────────────────────────

pub fn upsert_enum_option(
    conn: &Connection,
    field_gid: &str,
    option: &asanaclient::EnumOption,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO dim_enum_options (field_gid, option_gid, name, color, enabled, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
        params![
            field_gid,
            option.gid,
            option.name,
            option.color,
            option.enabled as i32,
        ],
    )?;
    Ok(())
}

pub fn delete_task_multi_enum_values(
    conn: &Connection,
    task_gid: &str,
    field_gid: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM bridge_task_multi_enum_values WHERE task_gid = ?1 AND field_gid = ?2",
        params![task_gid, field_gid],
    )?;
    Ok(())
}

pub fn upsert_task_multi_enum_value(
    conn: &Connection,
    task_gid: &str,
    field_gid: &str,
    option_gid: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO bridge_task_multi_enum_values (task_gid, field_gid, option_gid)
         VALUES (?1, ?2, ?3)",
        params![task_gid, field_gid, option_gid],
    )?;
    Ok(())
}

pub fn upsert_custom_fields(
    conn: &Connection,
    task_gid: &str,
    fields: &[asanaclient::CustomFieldValue],
) -> Result<(), rusqlite::Error> {
    for cf in fields {
        let has_enum = cf.enum_value.is_some();
        let has_multi_enum = !cf.multi_enum_values.is_empty();
        let display_value = cf.display_value.as_deref().unwrap_or("");

        // Skip fields that have no meaningful value at all
        if display_value.is_empty()
            && cf.text_value.is_none()
            && cf.number_value.is_none()
            && !has_enum
            && !has_multi_enum
        {
            continue;
        }

        let field_type = cf
            .resource_subtype
            .as_ref()
            .map(|t| format!("{t:?}").to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());

        // Upsert the field definition
        conn.execute(
            "INSERT OR REPLACE INTO dim_custom_fields (field_gid, name, field_type, cached_at)
             VALUES (?1, ?2, ?3, datetime('now'))",
            params![cf.gid, cf.name.as_deref().unwrap_or(""), field_type],
        )?;

        let enum_gid = cf.enum_value.as_ref().map(|e| e.gid.as_str());
        let date_val = cf.date_value.as_ref().and_then(|d| d.date.as_deref());

        conn.execute(
            "INSERT OR REPLACE INTO fact_task_custom_fields (
                task_gid, field_gid, text_value, number_value, date_value,
                enum_value_gid, display_value
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                task_gid,
                cf.gid,
                cf.text_value,
                cf.number_value,
                date_val,
                enum_gid,
                display_value,
            ],
        )?;

        // Upsert enum_value into dim_enum_options if present
        if let Some(ref ev) = cf.enum_value {
            upsert_enum_option(conn, &cf.gid, ev)?;
        }

        // Upsert multi_enum_values into dim_enum_options + bridge table
        if has_multi_enum {
            for option in &cf.multi_enum_values {
                upsert_enum_option(conn, &cf.gid, option)?;
            }
            // Clear stale bridge rows for this task+field, then insert current ones
            delete_task_multi_enum_values(conn, task_gid, &cf.gid)?;
            for option in &cf.multi_enum_values {
                upsert_task_multi_enum_value(conn, task_gid, &cf.gid, &option.gid)?;
            }
        }
    }
    Ok(())
}

// ── Comments / Stories ─────────────────────────────────────────────

pub fn upsert_comment(
    conn: &Connection,
    task_gid: &str,
    story: &asanaclient::Story,
) -> Result<(), rusqlite::Error> {
    let author_gid = story.created_by.as_ref().map(|u| u.gid.as_str());
    let story_type = story
        .resource_subtype
        .as_ref()
        .map(|t| format!("{t:?}").to_lowercase())
        .unwrap_or_else(|| "unknown".to_string());
    let created_at = story.created_at.as_deref().unwrap_or("");
    let created_date_key = date_key_from_iso(created_at);

    conn.execute(
        "INSERT INTO fact_comments (
            comment_gid, task_gid, author_gid, text, html_text,
            story_type, created_at, created_date_key, cached_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
        ON CONFLICT(comment_gid) DO UPDATE SET
            task_gid=excluded.task_gid, author_gid=excluded.author_gid,
            text=excluded.text, html_text=excluded.html_text,
            story_type=excluded.story_type, created_at=excluded.created_at,
            created_date_key=excluded.created_date_key, cached_at=excluded.cached_at",
        params![
            story.gid,
            task_gid,
            author_gid,
            story.text,
            story.html_text,
            story_type,
            created_at,
            created_date_key,
        ],
    )?;
    Ok(())
}

// ── Status Updates ─────────────────────────────────────────────────

pub fn upsert_status_update(
    conn: &Connection,
    parent_gid: &str,
    parent_type: &str,
    status: &asanaclient::types::StatusUpdate,
) -> Result<(), rusqlite::Error> {
    let author_gid = status.created_by.as_ref().map(|u| u.gid.as_str());
    let status_type = status
        .status_type
        .as_ref()
        .map(|s| format!("{s:?}").to_lowercase())
        .unwrap_or_else(|| "none".to_string());
    let title = status.title.as_deref().unwrap_or("");
    let created_at = status.created_at.as_deref().unwrap_or("");
    let created_date_key = date_key_from_iso(created_at);

    conn.execute(
        "INSERT OR REPLACE INTO fact_status_updates (
            status_gid, parent_gid, parent_type, author_gid,
            title, text, html_text, status_type,
            created_at, created_date_key, cached_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))",
        params![
            status.gid,
            parent_gid,
            parent_type,
            author_gid,
            title,
            status.text,
            status.html_text,
            status_type,
            created_at,
            created_date_key,
        ],
    )?;
    Ok(())
}

// ── Sections ───────────────────────────────────────────────────────

pub fn upsert_section(
    conn: &Connection,
    project_gid: &str,
    section_gid: &str,
    name: &str,
    sort_order: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO dim_sections (
            section_gid, project_gid, name, sort_order, cached_at
        ) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
        params![section_gid, project_gid, name, sort_order],
    )?;
    Ok(())
}

// ── Teams ──────────────────────────────────────────────────────────

pub fn upsert_team(
    conn: &Connection,
    team_gid: &str,
    name: &str,
    workspace_gid: &str,
    description: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO dim_teams (
            team_gid, name, workspace_gid, description, cached_at
        ) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
        params![team_gid, name, workspace_gid, description],
    )?;
    Ok(())
}

pub fn upsert_team_member(
    conn: &Connection,
    team_gid: &str,
    user_gid: &str,
    role: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO bridge_team_members (team_gid, user_gid, role)
         VALUES (?1, ?2, ?3)",
        params![team_gid, user_gid, role],
    )?;
    Ok(())
}

// ── Portfolios ─────────────────────────────────────────────────────

pub fn upsert_portfolio(
    conn: &Connection,
    portfolio: &asanaclient::Portfolio,
) -> Result<(), rusqlite::Error> {
    let owner_gid = portfolio.owner.as_ref().map(|o| o.gid.as_str());
    let workspace_gid = portfolio
        .workspace
        .as_ref()
        .map(|w| w.gid.as_str())
        .unwrap_or("");

    conn.execute(
        "INSERT OR REPLACE INTO dim_portfolios (
            portfolio_gid, name, owner_gid, workspace_gid, is_public, color, permalink_url, cached_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
        params![
            portfolio.gid,
            portfolio.name,
            owner_gid,
            workspace_gid,
            portfolio.public as i32,
            portfolio.color,
            portfolio.permalink_url,
        ],
    )?;
    Ok(())
}

pub fn upsert_portfolio_project(
    conn: &Connection,
    portfolio_gid: &str,
    project_gid: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO bridge_portfolio_projects (portfolio_gid, project_gid)
         VALUES (?1, ?2)",
        params![portfolio_gid, project_gid],
    )?;
    Ok(())
}

pub fn upsert_portfolio_portfolio(
    conn: &Connection,
    parent_portfolio_gid: &str,
    child_portfolio_gid: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO bridge_portfolio_portfolios (parent_portfolio_gid, child_portfolio_gid)
         VALUES (?1, ?2)",
        params![parent_portfolio_gid, child_portfolio_gid],
    )
    .map(|_| ())
}

// ── Monitored Entities ─────────────────────────────────────────────

pub fn add_monitored_entity(
    conn: &Connection,
    entity_key: &str,
    entity_type: &str,
    entity_gid: &str,
    display_name: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO monitored_entities (
            entity_key, entity_type, entity_gid, display_name, added_at, sync_enabled
        ) VALUES (?1, ?2, ?3, ?4, datetime('now'), 1)",
        params![entity_key, entity_type, entity_gid, display_name],
    )?;
    Ok(())
}

/// Ensure a `monitored_entities` row exists for the given entity so that
/// sync tokens and timestamps can be stored against it.  Portfolio-discovered
/// projects are not explicitly added by the user, so they may lack a row.
/// Uses `INSERT OR IGNORE` — existing rows (including user-added ones with
/// `sync_enabled = 1`) are left untouched.
pub fn ensure_entity_for_sync(
    conn: &Connection,
    entity_key: &str,
    entity_type: &str,
    entity_gid: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO monitored_entities (
            entity_key, entity_type, entity_gid, added_at, sync_enabled
        ) VALUES (?1, ?2, ?3, datetime('now'), 0)",
        params![entity_key, entity_type, entity_gid],
    )?;
    Ok(())
}

pub fn remove_monitored_entity(
    conn: &Connection,
    entity_key: &str,
) -> Result<bool, rusqlite::Error> {
    let count = conn.execute(
        "DELETE FROM monitored_entities WHERE entity_key = ?1",
        params![entity_key],
    )?;
    Ok(count > 0)
}

pub fn list_monitored_entities(conn: &Connection) -> Result<Vec<MonitoredEntity>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT entity_key, entity_type, entity_gid, display_name, added_at, last_sync_at, sync_enabled
         FROM monitored_entities WHERE sync_enabled = 1 ORDER BY added_at",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(MonitoredEntity {
            entity_key: row.get(0)?,
            entity_type: row.get(1)?,
            entity_gid: row.get(2)?,
            display_name: row.get(3)?,
            added_at: row.get(4)?,
            last_sync_at: row.get(5)?,
            sync_enabled: row.get(6)?,
        })
    })?;
    rows.collect()
}

pub fn get_last_sync_at(
    conn: &Connection,
    entity_key: &str,
) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT last_sync_at FROM monitored_entities WHERE entity_key = ?1",
        params![entity_key],
        |row| row.get(0),
    )
    .optional()
    .map(|opt| opt.flatten())
}

pub fn update_monitored_entity_sync_time(
    conn: &Connection,
    entity_key: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE monitored_entities SET last_sync_at = datetime('now') WHERE entity_key = ?1",
        params![entity_key],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct MonitoredEntity {
    pub entity_key: String,
    pub entity_type: String,
    pub entity_gid: String,
    pub display_name: Option<String>,
    pub added_at: String,
    pub last_sync_at: Option<String>,
    pub sync_enabled: bool,
}

// ── Event Sync Tokens ──────────────────────────────────────────────

pub fn get_event_sync_token(
    conn: &Connection,
    entity_key: &str,
) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT event_sync_token FROM monitored_entities WHERE entity_key = ?1",
        params![entity_key],
        |row| row.get(0),
    )
    .optional()
    .map(|opt| opt.flatten())
}

pub fn set_event_sync_token(
    conn: &Connection,
    entity_key: &str,
    token: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE monitored_entities SET event_sync_token = ?2 WHERE entity_key = ?1",
        params![entity_key, token],
    )?;
    Ok(())
}

// ── Config ─────────────────────────────────────────────────────────

pub fn get_config(conn: &Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT value FROM app_config WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

pub fn set_config(conn: &Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO app_config (key, value, updated_at)
         VALUES (?1, ?2, datetime('now'))",
        params![key, value],
    )?;
    Ok(())
}

pub fn list_config(conn: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT key, value FROM app_config ORDER BY key")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect()
}

// ── Sync Jobs ──────────────────────────────────────────────────────

pub fn insert_sync_job(
    conn: &Connection,
    entity_key: &str,
    range_start: Option<&str>,
    range_end: Option<&str>,
) -> Result<i64, rusqlite::Error> {
    conn.execute(
        "INSERT INTO sync_jobs (entity_key, status, started_at, sync_range_start, sync_range_end)
         VALUES (?1, 'running', datetime('now'), ?2, ?3)",
        params![entity_key, range_start, range_end],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(clippy::too_many_arguments)]
pub fn update_sync_job(
    conn: &Connection,
    job_id: i64,
    status: &str,
    synced_items: u64,
    failed_items: u64,
    batches_completed: u32,
    batches_total: u32,
    error_message: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE sync_jobs SET
            status = ?2, completed_at = datetime('now'),
            synced_items = ?3, failed_items = ?4,
            batches_completed = ?5, batches_total = ?6,
            error_message = ?7
         WHERE id = ?1",
        params![
            job_id,
            status,
            synced_items as i64,
            failed_items as i64,
            batches_completed,
            batches_total,
            error_message,
        ],
    )?;
    Ok(())
}

// ── Synced Ranges ──────────────────────────────────────────────────

pub fn insert_synced_range(
    conn: &Connection,
    entity_key: &str,
    start_date: &str,
    end_date: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO synced_ranges (entity_key, start_date, end_date, synced_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        params![entity_key, start_date, end_date],
    )?;
    Ok(())
}

pub fn get_synced_ranges(
    conn: &Connection,
    entity_key: &str,
) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT start_date, end_date FROM synced_ranges
         WHERE entity_key = ?1 ORDER BY start_date",
    )?;
    let rows = stmt.query_map(params![entity_key], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect()
}

// ── Helpers ────────────────────────────────────────────────────────

/// Extract YYYY-MM-DD from an ISO datetime string.
fn date_key_from_iso(iso: &str) -> String {
    if iso.len() >= 10 {
        iso[..10].to_string()
    } else {
        iso.to_string()
    }
}

fn compute_days_to_complete(created_at: &str, completed_at: Option<&str>) -> Option<i32> {
    let completed = completed_at?;
    let created = NaiveDate::parse_from_str(&date_key_from_iso(created_at), "%Y-%m-%d").ok()?;
    let done = NaiveDate::parse_from_str(&date_key_from_iso(completed), "%Y-%m-%d").ok()?;
    Some((done - created).num_days() as i32)
}

fn compute_is_overdue(completed: bool, due_on: Option<&str>) -> bool {
    if completed {
        return false;
    }
    let due = match due_on {
        Some(d) => d,
        None => return false,
    };
    let due_date = match NaiveDate::parse_from_str(due, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return false,
    };
    let today = chrono::Local::now().date_naive();
    today > due_date
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    #[tokio::test]
    async fn test_config_round_trip() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                set_config(conn, "workspace_gid", "12345")?;
                let val = get_config(conn, "workspace_gid")?;
                assert_eq!(val, Some("12345".to_string()));

                let missing = get_config(conn, "nonexistent")?;
                assert_eq!(missing, None);
                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_user_round_trip() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                let user = asanaclient::User {
                    gid: "u1".to_string(),
                    name: "Test User".to_string(),
                    email: Some("test@example.com".to_string()),
                    photo: None,
                };
                upsert_user(conn, &user)?;

                let name: String = conn.query_row(
                    "SELECT name FROM dim_users WHERE user_gid = 'u1'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(name, "Test User");
                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_monitored_entity_crud() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                add_monitored_entity(conn, "project:123", "project", "123", Some("My Project"))?;
                add_monitored_entity(conn, "user:456", "user", "456", Some("Alice"))?;

                let entities = list_monitored_entities(conn)?;
                assert_eq!(entities.len(), 2);

                let removed = remove_monitored_entity(conn, "user:456")?;
                assert!(removed);

                let entities = list_monitored_entities(conn)?;
                assert_eq!(entities.len(), 1);
                assert_eq!(entities[0].entity_key, "project:123");

                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_sync_job_round_trip() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                let job_id =
                    insert_sync_job(conn, "project:123", Some("2025-01-01"), Some("2025-01-31"))?;
                assert!(job_id > 0);

                update_sync_job(conn, job_id, "completed", 42, 0, 1, 1, None)?;

                let status: String = conn.query_row(
                    "SELECT status FROM sync_jobs WHERE id = ?1",
                    params![job_id],
                    |row| row.get(0),
                )?;
                assert_eq!(status, "completed");

                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_upsert_user_minimal_does_not_overwrite() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                // Full upsert first
                let user = asanaclient::User {
                    gid: "u1".to_string(),
                    name: "Full Name".to_string(),
                    email: Some("user@example.com".to_string()),
                    photo: None,
                };
                upsert_user(conn, &user)?;

                // Minimal upsert updates name but preserves email
                upsert_user_minimal(conn, "u1", Some("Short Name"))?;

                let (name, email): (String, Option<String>) = conn.query_row(
                    "SELECT name, email FROM dim_users WHERE user_gid = 'u1'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                // Name updated, email preserved (excluded.email is NULL → keeps existing)
                assert_eq!(name, "Short Name");
                assert_eq!(email, Some("user@example.com".to_string()));

                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_upsert_user_minimal_inserts_new() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                upsert_user_minimal(conn, "u1", Some("Bob"))?;

                let name: String = conn.query_row(
                    "SELECT name FROM dim_users WHERE user_gid = 'u1'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(name, "Bob");
                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[test]
    fn test_date_key_from_iso() {
        assert_eq!(date_key_from_iso("2025-01-15T10:30:00.000Z"), "2025-01-15");
        assert_eq!(date_key_from_iso("2025-01-15"), "2025-01-15");
    }

    #[test]
    fn test_compute_is_overdue() {
        // Completed task is never overdue
        assert!(!compute_is_overdue(true, Some("2020-01-01")));
        // No due date means not overdue
        assert!(!compute_is_overdue(false, None));
        // Future due date means not overdue
        assert!(!compute_is_overdue(false, Some("2099-12-31")));
        // Past due date means overdue
        assert!(compute_is_overdue(false, Some("2020-01-01")));
    }

    #[test]
    fn test_compute_days_to_complete() {
        assert_eq!(
            compute_days_to_complete("2025-01-01T00:00:00Z", Some("2025-01-11T00:00:00Z")),
            Some(10)
        );
        assert_eq!(compute_days_to_complete("2025-01-01", None), None);
    }

    #[tokio::test]
    async fn test_resolve_user_identifier() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                let user = asanaclient::User {
                    gid: "12345".to_string(),
                    name: "Alice".to_string(),
                    email: Some("alice@example.com".to_string()),
                    photo: None,
                };
                upsert_user(conn, &user)?;

                // Numeric GID resolves directly (no DB lookup)
                assert_eq!(
                    resolve_user_identifier(conn, "12345")?,
                    Some("12345".to_string())
                );

                // Email resolves to GID
                assert_eq!(
                    resolve_user_identifier(conn, "alice@example.com")?,
                    Some("12345".to_string())
                );

                // Unknown email returns None
                assert_eq!(resolve_user_identifier(conn, "nobody@example.com")?, None);

                // Non-GID, non-email string returns None
                assert_eq!(resolve_user_identifier(conn, "some-name")?, None);

                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_event_sync_token_round_trip() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                // Add a monitored entity first
                add_monitored_entity(conn, "project:123", "project", "123", Some("Test Project"))?;

                // Initially token should be None
                let token = get_event_sync_token(conn, "project:123")?;
                assert_eq!(token, None);

                // Set a token
                set_event_sync_token(conn, "project:123", "token_abc_123")?;

                // Retrieve the token
                let token = get_event_sync_token(conn, "project:123")?;
                assert_eq!(token, Some("token_abc_123".to_string()));

                // Update the token
                set_event_sync_token(conn, "project:123", "token_xyz_456")?;

                // Verify it was updated
                let token = get_event_sync_token(conn, "project:123")?;
                assert_eq!(token, Some("token_xyz_456".to_string()));

                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_event_sync_token_nonexistent_entity() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                // Get token for entity that doesn't exist
                let token = get_event_sync_token(conn, "project:999")?;
                assert_eq!(token, None);

                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_event_sync_token_multiple_entities() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                // Add multiple monitored entities
                add_monitored_entity(conn, "project:100", "project", "100", Some("Project 1"))?;
                add_monitored_entity(conn, "project:200", "project", "200", Some("Project 2"))?;

                // Set different tokens for each
                set_event_sync_token(conn, "project:100", "token_for_100")?;
                set_event_sync_token(conn, "project:200", "token_for_200")?;

                // Verify each entity has its own token
                assert_eq!(
                    get_event_sync_token(conn, "project:100")?,
                    Some("token_for_100".to_string())
                );
                assert_eq!(
                    get_event_sync_token(conn, "project:200")?,
                    Some("token_for_200".to_string())
                );

                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_ensure_entity_for_sync_enables_token_storage() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                // Portfolio-discovered project — no monitored_entities row yet
                ensure_entity_for_sync(conn, "project:500", "project", "500")?;

                // Token should now be storable and retrievable
                set_event_sync_token(conn, "project:500", "tok_abc")?;
                let token = get_event_sync_token(conn, "project:500")?;
                assert_eq!(token, Some("tok_abc".to_string()));

                // Row should have sync_enabled = 0 (not a user-monitored entity)
                let entities = list_monitored_entities(conn)?;
                assert!(
                    entities.iter().all(|e| e.entity_key != "project:500"),
                    "ensure_entity_for_sync rows should not appear in list_monitored_entities"
                );

                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_ensure_entity_for_sync_does_not_clobber_existing() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                // User explicitly adds a project (sync_enabled = 1, has display name)
                add_monitored_entity(conn, "project:600", "project", "600", Some("My Project"))?;
                set_event_sync_token(conn, "project:600", "existing_token")?;

                // Now ensure_entity_for_sync is called (e.g. via portfolio sync)
                ensure_entity_for_sync(conn, "project:600", "project", "600")?;

                // Existing row should be untouched
                let token = get_event_sync_token(conn, "project:600")?;
                assert_eq!(token, Some("existing_token".to_string()));

                // Still appears in list (sync_enabled = 1 preserved)
                let entities = list_monitored_entities(conn)?;
                assert!(entities.iter().any(|e| e.entity_key == "project:600"));

                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();
    }
}
