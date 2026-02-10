use std::collections::HashSet;

use chrono::Duration;

use crate::error::Result;
use crate::storage::repository;
use crate::storage::Database;
use crate::sync::rate_limit::retry_api;
use crate::sync::{IncrementalSyncSummary, SyncOptions, SyncProgress, SyncReport, SyncStatus};

/// Maximum number of changed tasks before falling back to full sync.
/// If events report more changes than this, individual GETs would be slower
/// than a bulk fetch.
const INCREMENTAL_THRESHOLD: usize = 50;

/// Task fields requested during project sync (both incremental and full).
const PROJECT_TASK_FIELDS: &str = "gid,name,completed,completed_at,assignee,assignee.name,assignee.email,due_on,due_at,start_on,start_at,created_at,modified_at,notes,html_notes,parent,parent.name,num_subtasks,num_likes,memberships,memberships.project,memberships.project.name,memberships.section,memberships.section.name,tags,tags.name,custom_fields,custom_fields.gid,custom_fields.name,custom_fields.display_value,custom_fields.resource_subtype,custom_fields.text_value,custom_fields.number_value,custom_fields.enum_value,custom_fields.enum_value.gid,custom_fields.enum_value.name,custom_fields.enum_value.color,custom_fields.enum_value.enabled,custom_fields.multi_enum_values,custom_fields.multi_enum_values.gid,custom_fields.multi_enum_values.name,custom_fields.multi_enum_values.color,custom_fields.multi_enum_values.enabled,custom_fields.date_value,custom_fields.date_value.date,custom_fields.date_value.date_time,permalink_url";

/// Store status updates and their authors in the database.
///
/// Shared by full sync, incremental sync, and portfolio sync paths.
async fn upsert_status_updates(
    db: &Database,
    parent_gid: &str,
    parent_type: &str,
    statuses: &[asanaclient::types::StatusUpdate],
) -> Result<()> {
    if statuses.is_empty() {
        return Ok(());
    }
    db.writer()
        .call({
            let parent_gid = parent_gid.to_string();
            let parent_type = parent_type.to_string();
            let statuses = statuses.to_vec();
            move |conn| {
                for status in &statuses {
                    if let Some(ref author) = status.created_by {
                        repository::upsert_user_minimal(conn, &author.gid, author.name.as_deref())?;
                    }
                    repository::upsert_status_update(conn, &parent_gid, &parent_type, status)?;
                }
                Ok::<(), rusqlite::Error>(())
            }
        })
        .await?;
    Ok(())
}

/// Classified summary of Asana events by resource type.
struct EventSummary {
    changed_task_gids: HashSet<String>,
    project_changed: bool,
    sections_changed: bool,
    status_updates_changed: bool,
}

impl EventSummary {
    fn has_changes(&self) -> bool {
        !self.changed_task_gids.is_empty()
            || self.project_changed
            || self.sections_changed
            || self.status_updates_changed
    }

    fn to_progress_summary(&self) -> IncrementalSyncSummary {
        IncrementalSyncSummary {
            tasks_changed: self.changed_task_gids.len(),
            project_changed: self.project_changed,
            sections_changed: self.sections_changed,
            status_updates_changed: self.status_updates_changed,
        }
    }
}

/// Classify Asana events into changed resource categories.
fn classify_events(events: &[asanaclient::Event]) -> EventSummary {
    let mut summary = EventSummary {
        changed_task_gids: HashSet::new(),
        project_changed: false,
        sections_changed: false,
        status_updates_changed: false,
    };
    for event in events {
        let resource_type = event
            .resource
            .resource_type
            .as_deref()
            .or(event.resource_type.as_deref())
            .unwrap_or("");
        match resource_type {
            "task" => match event.action.as_str() {
                "changed" | "added" | "undeleted" => {
                    summary.changed_task_gids.insert(event.resource.gid.clone());
                }
                _ => {}
            },
            "story" => match event.action.as_str() {
                "changed" | "added" => {
                    // Re-fetch the parent task's comments
                    if let Some(ref parent) = event.parent {
                        summary.changed_task_gids.insert(parent.gid.clone());
                    }
                }
                _ => {}
            },
            "section" => {
                summary.sections_changed = true;
            }
            "project" => {
                if event.action == "changed" {
                    summary.project_changed = true;
                }
            }
            "status_update" => match event.action.as_str() {
                "changed" | "added" => {
                    summary.status_updates_changed = true;
                }
                _ => {}
            },
            _ => {}
        }
    }
    summary
}

/// Store project metadata (owner, team, project, sections) in the database.
///
/// Shared by full sync and incremental sync (project/section refresh) paths.
async fn upsert_project_metadata(
    db: &Database,
    project: &asanaclient::Project,
    sections: &[super::api_helpers::SectionInfo],
) -> Result<()> {
    db.writer()
        .call({
            let project = project.clone();
            let sections = sections.to_vec();
            let project_gid = project.gid.clone();
            move |conn| {
                if let Some(ref owner) = project.owner {
                    repository::upsert_user_minimal(conn, &owner.gid, owner.name.as_deref())?;
                }
                if let Some(ref team) = project.team {
                    let team_name = team.name.as_deref().unwrap_or("");
                    let workspace_gid = project
                        .workspace
                        .as_ref()
                        .map(|w| w.gid.as_str())
                        .unwrap_or("");
                    repository::upsert_team(conn, &team.gid, team_name, workspace_gid, None)?;
                }
                repository::upsert_project(conn, &project)?;
                for (i, section) in sections.iter().enumerate() {
                    repository::upsert_section(
                        conn,
                        &project_gid,
                        &section.gid,
                        &section.name,
                        i as i32,
                    )?;
                }
                Ok::<(), rusqlite::Error>(())
            }
        })
        .await?;
    Ok(())
}

/// Upsert tasks and their comments to the database.
///
/// Handles the FK constraint dance: upsert referenced users first, temporarily
/// disable FK checks for tasks (parent_gid may reference tasks not yet synced),
/// then re-enable FK checks before inserting comments.
///
/// Shared by full sync and incremental sync paths.
async fn upsert_tasks_and_comments(
    db: &Database,
    tasks: &[asanaclient::Task],
    task_comments: &[(String, Vec<asanaclient::Story>)],
) -> Result<()> {
    if tasks.is_empty() && task_comments.is_empty() {
        return Ok(());
    }
    db.writer()
        .call({
            let tasks = tasks.to_vec();
            let task_comments = task_comments.to_vec();
            move |conn| {
                // Upsert referenced users BEFORE tasks (FK constraints)
                for task in &tasks {
                    if let Some(ref assignee) = task.assignee {
                        repository::upsert_user_minimal_with_email(
                            conn,
                            &assignee.gid,
                            assignee.name.as_deref(),
                            assignee.email.as_deref(),
                        )?;
                    }
                }
                for (_task_gid, comments) in &task_comments {
                    for comment in comments {
                        if let Some(ref author) = comment.created_by {
                            repository::upsert_user_minimal(
                                conn,
                                &author.gid,
                                author.name.as_deref(),
                            )?;
                        }
                    }
                }

                // Temporarily disable FK checks — parent_gid may reference
                // tasks not yet synced
                conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

                for task in &tasks {
                    repository::upsert_task(conn, task)?;
                }

                conn.execute_batch("PRAGMA foreign_keys = ON;")?;

                // Upsert comments
                for (task_gid, comments) in &task_comments {
                    for comment in comments {
                        repository::upsert_comment(conn, task_gid, comment)?;
                    }
                }

                Ok::<(), rusqlite::Error>(())
            }
        })
        .await?;
    Ok(())
}

/// Sync a single project's tasks and metadata to the database.
///
/// Attempts incremental sync via the Asana Events API first. Falls back to
/// a full sync when: (a) no sync token exists, (b) the token has expired,
/// (c) the `--full` flag is set, or (d) incremental sync encounters an error.
pub async fn sync_project(
    db: &Database,
    client: &asanaclient::Client,
    project_gid: &str,
    options: &SyncOptions,
    progress: &dyn SyncProgress,
) -> Result<SyncReport> {
    // Ensure a monitored_entities row exists so sync tokens and timestamps
    // can be stored.  Portfolio-discovered projects won't have one otherwise,
    // causing every sync to fall back to a full fetch.
    let entity_key = format!("project:{project_gid}");
    db.writer()
        .call({
            let entity_key = entity_key.clone();
            let project_gid = project_gid.to_string();
            move |conn| {
                repository::ensure_entity_for_sync(conn, &entity_key, "project", &project_gid)
            }
        })
        .await?;

    // Try incremental sync via events
    if !options.full {
        match sync_project_incremental(db, client, project_gid, options, progress).await {
            Ok(Some(report)) => return Ok(report), // Incremental succeeded
            Ok(None) => {}                         // No token or expired, fall through
            Err(e) => log::warn!("Incremental sync failed, falling back to full: {e}"),
        }
    }

    // Full sync (existing logic)
    sync_project_full(db, client, project_gid, options, progress).await
}

/// Attempt incremental sync for a project using the Asana Events API.
///
/// Returns:
/// - `Ok(Some(report))` if incremental sync succeeded
/// - `Ok(None)` if no token exists or the token expired (caller should do full sync)
/// - `Err(e)` if an unexpected error occurred
async fn sync_project_incremental(
    db: &Database,
    client: &asanaclient::Client,
    project_gid: &str,
    _options: &SyncOptions,
    progress: &dyn SyncProgress,
) -> Result<Option<SyncReport>> {
    let entity_key = format!("project:{project_gid}");

    // Read existing sync token
    let token: Option<String> = db
        .reader()
        .call({
            let entity_key = entity_key.clone();
            move |conn| repository::get_event_sync_token(conn, &entity_key)
        })
        .await?;

    let token = match token {
        Some(t) => t,
        None => {
            // No token — establish one, then signal full sync needed
            log::info!("No event sync token for {entity_key}, establishing...");
            match client.events().establish(project_gid).await {
                Ok(new_token) => {
                    db.writer()
                        .call({
                            let entity_key = entity_key.clone();
                            move |conn| {
                                repository::set_event_sync_token(conn, &entity_key, &new_token)
                            }
                        })
                        .await?;
                }
                Err(e) => {
                    log::warn!("Failed to establish event sync token: {e}");
                }
            }
            return Ok(None);
        }
    };

    // Fetch events since the token
    let events_response = match client.events().get_events(project_gid, &token).await {
        Ok(resp) => resp,
        Err(asanaclient::Error::SyncTokenExpired { sync }) => {
            // Token expired — store the fresh token and signal full sync
            log::info!("Event sync token expired for {entity_key}, storing fresh token");
            db.writer()
                .call({
                    let entity_key = entity_key.clone();
                    move |conn| repository::set_event_sync_token(conn, &entity_key, &sync)
                })
                .await?;
            return Ok(None);
        }
        Err(e) => return Err(e.into()),
    };

    // Classify events by resource type
    let summary = classify_events(&events_response.data);

    // If nothing changed, just update the token and return
    if !summary.has_changes() {
        let new_token = events_response.sync.clone();
        db.writer()
            .call({
                let entity_key = entity_key.clone();
                move |conn| {
                    repository::set_event_sync_token(conn, &entity_key, &new_token)?;
                    repository::update_monitored_entity_sync_time(conn, &entity_key)?;
                    Ok::<(), rusqlite::Error>(())
                }
            })
            .await?;

        progress.on_incremental_sync(&entity_key, &summary.to_progress_summary());

        return Ok(Some(SyncReport {
            entity_key,
            status: SyncStatus::Success,
            items_synced: 0,
            items_failed: 0,
            batches_completed: 1,
            batches_total: 1,
            error: None,
        }));
    }

    // If too many task changes, fall back to full sync (store the new token first)
    if summary.changed_task_gids.len() > INCREMENTAL_THRESHOLD {
        log::info!(
            "{} tasks changed for {entity_key} (threshold: {INCREMENTAL_THRESHOLD}), falling back to full sync",
            summary.changed_task_gids.len()
        );
        let new_token = events_response.sync.clone();
        db.writer()
            .call({
                let entity_key = entity_key.clone();
                move |conn| repository::set_event_sync_token(conn, &entity_key, &new_token)
            })
            .await?;
        return Ok(None);
    }

    progress.on_incremental_sync(&entity_key, &summary.to_progress_summary());

    // Fetch full task data for each changed task
    let mut tasks: Vec<asanaclient::Task> = Vec::new();
    let mut fetch_failures: u64 = 0;
    for gid in &summary.changed_task_gids {
        let path = format!("/tasks/{gid}");
        let query_params = [("opt_fields", PROJECT_TASK_FIELDS)];
        match retry_api!(client.get::<asanaclient::Task>(&path, &query_params)) {
            Ok(task) => tasks.push(task),
            Err(crate::error::Error::Api(asanaclient::Error::NotFound(_))) => {
                // Task was deleted — skip it for now; full sync handles cleanup
                log::debug!("Task {gid} not found (likely deleted), skipping");
            }
            Err(e) => {
                log::warn!("Failed to fetch task {gid}: {e}");
                fetch_failures += 1;
            }
        }
    }

    progress.on_tasks_fetched(&entity_key, tasks.len());

    // Fetch comments for each changed task
    let mut task_comments: Vec<(String, Vec<asanaclient::Story>)> = Vec::new();
    let comments_total = tasks.len();
    for (i, task) in tasks.iter().enumerate() {
        progress.on_comments_progress(&entity_key, i + 1, comments_total);
        let task_gid = task.gid.clone();
        match retry_api!(client.tasks().comments(&task_gid)) {
            Ok(comments) => {
                task_comments.push((task.gid.clone(), comments));
            }
            Err(e) => {
                log::warn!("Failed to fetch comments for task {}: {e}", task.gid);
                task_comments.push((task.gid.clone(), Vec::new()));
            }
        }
    }

    let total_synced = tasks.len() as u64;

    // Store tasks and comments
    upsert_tasks_and_comments(db, &tasks, &task_comments).await?;

    // Update sync token and timestamp
    let new_token = events_response.sync.clone();
    db.writer()
        .call({
            let entity_key = entity_key.clone();
            move |conn| {
                repository::set_event_sync_token(conn, &entity_key, &new_token)?;
                repository::update_monitored_entity_sync_time(conn, &entity_key)?;
                Ok::<(), rusqlite::Error>(())
            }
        })
        .await?;

    // Refresh project metadata and/or sections if changed
    if summary.project_changed || summary.sections_changed {
        let project = retry_api!(client.projects().get_full(project_gid))?;
        let sections = super::api_helpers::get_project_sections(client, project_gid).await?;
        upsert_project_metadata(db, &project, &sections).await?;
    }

    // Refresh status updates if changed
    if summary.status_updates_changed {
        let statuses = retry_api!(client.projects().status_updates(project_gid))?;
        progress.on_status_updates_synced(&entity_key, statuses.len());
        upsert_status_updates(db, project_gid, "project", &statuses).await?;
    }

    let status = if fetch_failures == 0 {
        SyncStatus::Success
    } else if total_synced > 0 {
        SyncStatus::PartialFailure
    } else {
        SyncStatus::Failed
    };

    Ok(Some(SyncReport {
        entity_key,
        status,
        items_synced: total_synced,
        items_failed: fetch_failures,
        batches_completed: 1,
        batches_total: 1,
        error: if fetch_failures > 0 {
            Some(format!("{fetch_failures} task fetches failed"))
        } else {
            None
        },
    }))
}

/// Full sync for a project: re-fetch all tasks and comments.
///
/// This is the original sync logic, used as fallback when incremental sync
/// is not possible (first run, expired token, --full flag).
async fn sync_project_full(
    db: &Database,
    client: &asanaclient::Client,
    project_gid: &str,
    options: &SyncOptions,
    progress: &dyn SyncProgress,
) -> Result<SyncReport> {
    let entity_key = format!("project:{project_gid}");

    // Check when we last synced this entity (used to skip unchanged tasks' comments)
    let last_sync_at: Option<String> = db
        .reader()
        .call({
            let entity_key = entity_key.clone();
            move |conn| repository::get_last_sync_at(conn, &entity_key)
        })
        .await?;

    // Fetch and store project metadata + sections
    let project = retry_api!(client.projects().get_full(project_gid))?;
    let sections = super::api_helpers::get_project_sections(client, project_gid).await?;
    upsert_project_metadata(db, &project, &sections).await?;

    // Create sync job record
    let today = chrono::Local::now().date_naive();
    let since = options.since_date().unwrap_or(today - Duration::days(90));
    let job_id = db
        .writer()
        .call({
            let entity_key = entity_key.clone();
            let range_start = since.format("%Y-%m-%d").to_string();
            let range_end = today.format("%Y-%m-%d").to_string();
            move |conn| {
                repository::insert_sync_job(conn, &entity_key, Some(&range_start), Some(&range_end))
            }
        })
        .await?;

    // Fetch all tasks from the project.
    // `completed_since` returns all incomplete tasks PLUS tasks completed after the given time.
    let completed_since = format!("{}T00:00:00.000Z", since);
    let path = format!("/projects/{project_gid}/tasks");
    let query_params = [
        ("opt_fields", PROJECT_TASK_FIELDS),
        ("completed_since", completed_since.as_str()),
    ];
    let tasks: Vec<asanaclient::Task> = retry_api!(client.get_all(&path, &query_params))?;

    progress.on_tasks_fetched(&entity_key, tasks.len());

    // Fetch comments only for tasks modified since our last sync.
    // Tasks whose modified_at predates last_sync_at already have their comments stored.
    let total_tasks = tasks.len();
    let mut tasks_needing_comments: Vec<&asanaclient::Task> = Vec::new();
    for task in &tasks {
        let needs_fetch = match (&task.modified_at, &last_sync_at) {
            (Some(modified), Some(synced)) => modified.as_str() > synced.as_str(),
            _ => true, // No modified_at or never synced → fetch
        };
        if needs_fetch {
            tasks_needing_comments.push(task);
        }
    }
    let skipped = total_tasks - tasks_needing_comments.len();
    if skipped > 0 {
        progress.on_comments_skipped(&entity_key, skipped, total_tasks);
    }

    let mut task_comments: Vec<(String, Vec<asanaclient::Story>)> = Vec::new();
    let comments_total = tasks_needing_comments.len();
    for (i, task) in tasks_needing_comments.iter().enumerate() {
        progress.on_comments_progress(&entity_key, i + 1, comments_total);
        let task_gid = task.gid.clone();
        match retry_api!(client.tasks().comments(&task_gid)) {
            Ok(comments) => {
                task_comments.push((task.gid.clone(), comments));
            }
            Err(e) => {
                log::warn!("Failed to fetch comments for task {}: {e}", task.gid);
                task_comments.push((task.gid.clone(), Vec::new()));
            }
        }
    }

    let total_synced = tasks.len() as u64;

    // Store tasks and comments
    upsert_tasks_and_comments(db, &tasks, &task_comments).await?;

    // Fetch and store status updates for the project
    let statuses = retry_api!(client.projects().status_updates(project_gid))?;
    progress.on_status_updates_synced(&entity_key, statuses.len());
    upsert_status_updates(db, project_gid, "project", &statuses).await?;

    let status = if total_synced > 0 || tasks.is_empty() {
        SyncStatus::Success
    } else {
        SyncStatus::Failed
    };
    let status_str = match &status {
        SyncStatus::Success => "completed",
        SyncStatus::PartialFailure => "partial_failure",
        SyncStatus::Failed => "failed",
    }
    .to_string();

    // Update sync job
    db.writer()
        .call({
            let entity_key = entity_key.clone();
            move |conn| {
                repository::update_sync_job(
                    conn,
                    job_id,
                    &status_str,
                    total_synced,
                    0,
                    1,
                    1,
                    None,
                )?;
                repository::update_monitored_entity_sync_time(conn, &entity_key)?;
                Ok::<(), rusqlite::Error>(())
            }
        })
        .await?;

    // Establish a fresh event sync token so the next sync can be incremental
    match client.events().establish(project_gid).await {
        Ok(new_token) => {
            db.writer()
                .call({
                    let entity_key = entity_key.clone();
                    move |conn| repository::set_event_sync_token(conn, &entity_key, &new_token)
                })
                .await?;
        }
        Err(e) => {
            log::warn!("Failed to establish event sync token after full sync: {e}");
        }
    }

    Ok(SyncReport {
        entity_key,
        status,
        items_synced: total_synced,
        items_failed: 0,
        batches_completed: 1,
        batches_total: 1,
        error: None,
    })
}

/// Sync a user's tasks across the workspace.
pub async fn sync_user(
    db: &Database,
    client: &asanaclient::Client,
    workspace_gid: &str,
    user_gid: &str,
    options: &SyncOptions,
    progress: &dyn SyncProgress,
) -> Result<SyncReport> {
    let entity_key = format!("user:{user_gid}");
    let today = chrono::Local::now().date_naive();
    let since = options.since_date().unwrap_or(today - Duration::days(90));

    let modified_since = format!("{}T00:00:00Z", since);

    let job_id = db
        .writer()
        .call({
            let entity_key = entity_key.clone();
            let start = since.format("%Y-%m-%d").to_string();
            let end = today.format("%Y-%m-%d").to_string();
            move |conn| repository::insert_sync_job(conn, &entity_key, Some(&start), Some(&end))
        })
        .await?;

    let tasks = super::api_helpers::search_workspace_tasks(
        client,
        workspace_gid,
        Some(&modified_since),
        Some(user_gid),
    )
    .await?;

    progress.on_tasks_fetched(&entity_key, tasks.len());

    let task_count = tasks.len() as u64;

    for task in &tasks {
        db.writer()
            .call({
                let task = task.clone();
                move |conn| {
                    repository::upsert_task(conn, &task)?;
                    Ok::<(), rusqlite::Error>(())
                }
            })
            .await?;
    }

    db.writer()
        .call({
            let entity_key = entity_key.clone();
            move |conn| {
                repository::update_sync_job(conn, job_id, "completed", task_count, 0, 1, 1, None)?;
                repository::update_monitored_entity_sync_time(conn, &entity_key)?;
                Ok::<(), rusqlite::Error>(())
            }
        })
        .await?;

    Ok(SyncReport {
        entity_key,
        status: SyncStatus::Success,
        items_synced: task_count,
        items_failed: 0,
        batches_completed: 1,
        batches_total: 1,
        error: None,
    })
}

/// Sync a team: fetch members, projects, and sync each project.
pub async fn sync_team(
    db: &Database,
    client: &asanaclient::Client,
    _workspace_gid: &str,
    team_gid: &str,
    options: &SyncOptions,
    progress: &dyn SyncProgress,
) -> Result<SyncReport> {
    let entity_key = format!("team:{team_gid}");

    // Fetch team members
    let members = super::api_helpers::get_team_members(client, team_gid).await?;

    db.writer()
        .call({
            let team_gid = team_gid.to_string();
            let members = members.clone();
            move |conn| {
                for member in &members {
                    repository::upsert_user_minimal(conn, &member.gid, member.name.as_deref())?;
                    repository::upsert_team_member(conn, &team_gid, &member.gid, None)?;
                }
                Ok::<(), rusqlite::Error>(())
            }
        })
        .await?;

    // Fetch and sync team projects
    let projects = super::api_helpers::get_team_projects(client, team_gid).await?;
    let mut total_synced: u64 = 0;
    let mut total_failed: u64 = 0;
    let total = projects.len() as u32;

    for project_ref in &projects {
        if project_ref.archived {
            continue;
        }
        match sync_project(db, client, &project_ref.gid, options, progress).await {
            Ok(report) => {
                total_synced += report.items_synced;
            }
            Err(e) => {
                log::error!(
                    "Failed to sync project {} ({}): {e}",
                    project_ref.name,
                    project_ref.gid
                );
                total_failed += 1;
            }
        }
    }

    Ok(SyncReport::from_counts(
        entity_key,
        total_synced,
        total_failed,
        total.saturating_sub(total_failed as u32),
        total,
    ))
}

/// Sync a portfolio: fetch items and sync each project.
pub async fn sync_portfolio(
    db: &Database,
    client: &asanaclient::Client,
    portfolio_gid: &str,
    options: &SyncOptions,
    progress: &dyn SyncProgress,
) -> Result<SyncReport> {
    let entity_key = format!("portfolio:{portfolio_gid}");

    let portfolio = retry_api!(client.portfolios().get(portfolio_gid))?;
    db.writer()
        .call({
            let portfolio = portfolio.clone();
            move |conn| {
                // Insert referenced owner before the portfolio (FK constraints)
                if let Some(ref owner) = portfolio.owner {
                    repository::upsert_user_minimal(conn, &owner.gid, owner.name.as_deref())?;
                }
                repository::upsert_portfolio(conn, &portfolio)?;
                Ok::<(), rusqlite::Error>(())
            }
        })
        .await?;

    // Fetch and store status updates for the portfolio
    let statuses = retry_api!(client.portfolios().status_updates(portfolio_gid))?;
    progress.on_status_updates_synced(&entity_key, statuses.len());
    upsert_status_updates(db, portfolio_gid, "portfolio", &statuses).await?;

    // Fetch portfolio items (projects)
    let items = retry_api!(client.portfolios().items(portfolio_gid))?;

    let mut total_synced: u64 = 0;
    let mut total_failed: u64 = 0;
    let mut project_count: u32 = 0;

    for item in &items {
        let gid = &item.gid;
        let resource_type = item.resource_type.as_str();

        if resource_type == "project" {
            project_count += 1;
            match sync_project(db, client, gid, options, progress).await {
                Ok(report) => {
                    total_synced += report.items_synced;
                    // Link portfolio to project only after project exists
                    db.writer()
                        .call({
                            let portfolio_gid = portfolio_gid.to_string();
                            let project_gid = gid.clone();
                            move |conn| {
                                repository::upsert_portfolio_project(
                                    conn,
                                    &portfolio_gid,
                                    &project_gid,
                                )?;
                                Ok::<(), rusqlite::Error>(())
                            }
                        })
                        .await?;
                }
                Err(e) => {
                    log::error!("Failed to sync project {gid} in portfolio: {e}");
                    total_failed += 1;
                }
            }
        }
    }

    Ok(SyncReport::from_counts(
        entity_key,
        total_synced,
        total_failed,
        project_count.saturating_sub(total_failed as u32),
        project_count,
    ))
}
