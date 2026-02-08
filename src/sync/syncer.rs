use chrono::Duration;

use crate::error::Result;
use crate::storage::repository;
use crate::storage::Database;
use crate::sync::rate_limit::retry_api;
use crate::sync::{SyncOptions, SyncReport, SyncStatus};

/// Sync a single project's tasks and metadata to the database.
///
/// The Asana `/projects/{gid}/tasks` endpoint does NOT support `modified_since`.
/// It only supports `completed_since` (to exclude old completed tasks) and `opt_fields`.
/// We fetch all tasks in one paginated call, using `completed_since` to skip tasks
/// that were completed before our lookback window.
pub async fn sync_project(
    db: &Database,
    client: &asanaclient::Client,
    project_gid: &str,
    options: &SyncOptions,
) -> Result<SyncReport> {
    let entity_key = format!("project:{project_gid}");

    // Fetch project details and upsert
    let project = retry_api!(client.projects().get_full(project_gid))?;

    // Fetch sections
    let sections = super::api_helpers::get_project_sections(client, project_gid).await?;

    db.writer()
        .call({
            let project = project.clone();
            let project_gid = project_gid.to_string();
            let sections = sections.clone();
            move |conn| {
                // Insert referenced entities before the project (FK constraints)
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
                repository::insert_sync_job(
                    conn,
                    &entity_key,
                    Some(&range_start),
                    Some(&range_end),
                )
            }
        })
        .await?;

    // Fetch all tasks from the project.
    // `completed_since` returns all incomplete tasks PLUS tasks completed after the given time.
    let fields = "gid,name,completed,completed_at,assignee,assignee.name,assignee.email,due_on,due_at,start_on,start_at,created_at,modified_at,notes,html_notes,parent,parent.name,num_subtasks,num_likes,memberships,memberships.project,memberships.project.name,memberships.section,memberships.section.name,tags,tags.name,custom_fields,custom_fields.gid,custom_fields.name,custom_fields.display_value,custom_fields.resource_subtype,custom_fields.text_value,custom_fields.number_value,custom_fields.enum_value,custom_fields.enum_value.gid,custom_fields.enum_value.name,custom_fields.enum_value.color,custom_fields.enum_value.enabled,custom_fields.multi_enum_values,custom_fields.multi_enum_values.gid,custom_fields.multi_enum_values.name,custom_fields.multi_enum_values.color,custom_fields.multi_enum_values.enabled,custom_fields.date_value,custom_fields.date_value.date,custom_fields.date_value.date_time,permalink_url";
    let completed_since = format!("{}T00:00:00.000Z", since);
    let path = format!("/projects/{project_gid}/tasks");
    let query_params = [
        ("opt_fields", fields),
        ("completed_since", completed_since.as_str()),
    ];
    let tasks: Vec<asanaclient::Task> = retry_api!(client.get_all(&path, &query_params))?;

    // Fetch comments for each task
    let mut task_comments: Vec<(String, Vec<asanaclient::Story>)> = Vec::new();
    for task in &tasks {
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

    // Write all tasks and comments to DB
    db.writer()
        .call({
            let tasks = tasks.clone();
            move |conn| {
                // Upsert all referenced users BEFORE tasks (FK: assignee_gid → dim_users)
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
                            repository::upsert_user_minimal(conn, &author.gid, author.name.as_deref())?;
                        }
                    }
                }

                // Temporarily disable FK checks for tasks — parent_gid may reference
                // tasks not yet synced, and created_date_key may be outside dim_date range
                conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

                for task in &tasks {
                    repository::upsert_task(conn, task)?;
                }

                // Re-enable FK checks before inserting comments (which have valid FKs)
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
                    conn, job_id, &status_str, total_synced, 0, 1, 1, None,
                )?;
                repository::update_monitored_entity_sync_time(conn, &entity_key)?;
                Ok::<(), rusqlite::Error>(())
            }
        })
        .await?;

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
            move |conn| {
                repository::insert_sync_job(conn, &entity_key, Some(&start), Some(&end))
            }
        })
        .await?;

    let tasks = super::api_helpers::search_workspace_tasks(
        client,
        workspace_gid,
        Some(&modified_since),
        Some(user_gid),
    )
    .await?;

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
        match sync_project(db, client, &project_ref.gid, options).await {
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
            match sync_project(db, client, gid, options).await {
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
