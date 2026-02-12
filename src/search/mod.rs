use serde::Serialize;

use crate::error::Result;
use crate::storage::Database;

/// What kind of entity matched the search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SearchHitType {
    Task,
    Comment,
    Project,
    Portfolio,
    CustomField,
}

/// A single search result.
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub hit_type: SearchHitType,
    /// GID of the matched entity (task_gid, comment_gid, or project_gid).
    pub gid: String,
    /// For comments and custom fields, the parent task GID.
    pub task_gid: Option<String>,
    /// Display name / title.
    pub title: String,
    /// Highlighted snippet from the matching text.
    pub snippet: String,
    /// FTS5 rank score (lower = more relevant).
    pub rank: f64,
    /// Asana URL, if reconstructable.
    pub asana_url: Option<String>,
}

/// Options controlling a search operation.
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Maximum number of results to return.
    pub limit: Option<u32>,
    /// Restrict search to a specific hit type.
    pub hit_type: Option<SearchHitType>,
    /// Filter to tasks assigned to this user GID.
    pub assignee_gid: Option<String>,
    /// Filter to tasks in this project GID.
    pub project_gid: Option<String>,
}

/// Search results container.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResults {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub total: usize,
}

/// Search across all FTS-indexed content.
pub async fn search(db: &Database, query: &str, options: &SearchOptions) -> Result<SearchResults> {
    if query.trim().is_empty() {
        return Ok(SearchResults {
            query: query.to_string(),
            hits: Vec::new(),
            total: 0,
        });
    }
    let query_owned = query.to_string();
    let options_limit = options.limit.unwrap_or(50);
    let hit_type_filter = options.hit_type.clone();
    let assignee_filter = options.assignee_gid.clone();
    let project_filter = options.project_gid.clone();

    let hits: SearchResults = db
        .reader()
        .call(move |conn| {
            let mut all_hits: Vec<SearchHit> = Vec::new();

            // Search tasks
            if hit_type_filter.is_none() || hit_type_filter == Some(SearchHitType::Task) {
                let mut sql = String::from(
                    "SELECT t.task_gid, t.name, snippet(tasks_fts, 2, '<b>', '</b>', '...', 32) as snip, tasks_fts.rank, t.permalink_url
                     FROM tasks_fts
                     JOIN fact_tasks t ON t.id = tasks_fts.rowid
                     WHERE tasks_fts MATCH ?1",
                );
                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
                    vec![Box::new(query_owned.clone())];

                if let Some(ref assignee) = assignee_filter {
                    sql.push_str(" AND t.assignee_gid = ?2");
                    params.push(Box::new(assignee.clone()));
                }
                if let Some(ref project) = project_filter {
                    let param_idx = params.len() + 1;
                    sql.push_str(&format!(
                        " AND t.task_gid IN (SELECT task_gid FROM bridge_task_projects WHERE project_gid = ?{param_idx})"
                    ));
                    params.push(Box::new(project.clone()));
                }
                sql.push_str(" ORDER BY rank LIMIT ?");
                let limit_idx = params.len() + 1;
                sql = sql.replace("LIMIT ?", &format!("LIMIT ?{limit_idx}"));
                params.push(Box::new(options_limit));

                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(param_refs.as_slice(), |row| {
                    let gid: String = row.get(0)?;
                    let snippet: Option<String> = row.get(2)?;
                    let stored_url: Option<String> = row.get(4)?;
                    Ok(SearchHit {
                        hit_type: SearchHitType::Task,
                        gid: gid.clone(),
                        task_gid: None,
                        title: row.get(1)?,
                        snippet: snippet.unwrap_or_default(),
                        rank: row.get(3)?,
                        asana_url: stored_url.or_else(|| Some(format!("https://app.asana.com/0/0/{gid}"))),
                    })
                })?;
                for row in rows {
                    all_hits.push(row?);
                }
            }

            // Search comments
            if hit_type_filter.is_none() || hit_type_filter == Some(SearchHitType::Comment) {
                let mut sql = String::from(
                    "SELECT c.comment_gid, c.task_gid, t.name, snippet(comments_fts, 2, '<b>', '</b>', '...', 32) as snip, comments_fts.rank, t.permalink_url
                     FROM comments_fts
                     JOIN fact_comments c ON c.id = comments_fts.rowid
                     LEFT JOIN fact_tasks t ON t.task_gid = c.task_gid
                     WHERE comments_fts MATCH ?1",
                );
                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
                    vec![Box::new(query_owned.clone())];

                if let Some(ref assignee) = assignee_filter {
                    sql.push_str(" AND t.assignee_gid = ?2");
                    params.push(Box::new(assignee.clone()));
                }
                if let Some(ref project) = project_filter {
                    let param_idx = params.len() + 1;
                    sql.push_str(&format!(
                        " AND c.task_gid IN (SELECT task_gid FROM bridge_task_projects WHERE project_gid = ?{param_idx})"
                    ));
                    params.push(Box::new(project.clone()));
                }
                sql.push_str(&format!(" ORDER BY rank LIMIT ?{}", params.len() + 1));
                params.push(Box::new(options_limit));

                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(param_refs.as_slice(), |row| {
                    let task_gid: String = row.get(1)?;
                    let task_name: Option<String> = row.get(2)?;
                    let snippet: Option<String> = row.get(3)?;
                    let stored_url: Option<String> = row.get(5)?;
                    Ok(SearchHit {
                        hit_type: SearchHitType::Comment,
                        gid: row.get(0)?,
                        task_gid: Some(task_gid.clone()),
                        title: format!("Comment on: {}", task_name.as_deref().unwrap_or("(unknown task)")),
                        snippet: snippet.unwrap_or_default(),
                        rank: row.get(4)?,
                        asana_url: stored_url.or_else(|| Some(format!("https://app.asana.com/0/0/{task_gid}"))),
                    })
                })?;
                for row in rows {
                    all_hits.push(row?);
                }
            }

            // Search projects
            if hit_type_filter.is_none() || hit_type_filter == Some(SearchHitType::Project) {
                let sql =
                    "SELECT p.project_gid, p.name, snippet(projects_fts, 1, '<b>', '</b>', '...', 32) as snip, projects_fts.rank, p.permalink_url
                     FROM projects_fts
                     JOIN dim_projects p ON p.id = projects_fts.rowid
                     WHERE projects_fts MATCH ?1
                     ORDER BY rank LIMIT ?2";
                let mut stmt = conn.prepare(sql)?;
                let rows = stmt.query_map(rusqlite::params![query_owned, options_limit], |row| {
                    let gid: String = row.get(0)?;
                    let snippet: Option<String> = row.get(2)?;
                    let stored_url: Option<String> = row.get(4)?;
                    Ok(SearchHit {
                        hit_type: SearchHitType::Project,
                        gid: gid.clone(),
                        task_gid: None,
                        title: row.get(1)?,
                        snippet: snippet.unwrap_or_default(),
                        rank: row.get(3)?,
                        asana_url: stored_url.or_else(|| Some(format!("https://app.asana.com/0/{gid}"))),
                    })
                })?;
                for row in rows {
                    all_hits.push(row?);
                }
            }

            // Search portfolios
            if hit_type_filter.is_none() || hit_type_filter == Some(SearchHitType::Portfolio) {
                let sql =
                    "SELECT p.portfolio_gid, p.name, snippet(portfolios_fts, 1, '<b>', '</b>', '...', 32) as snip, portfolios_fts.rank, p.permalink_url
                     FROM portfolios_fts
                     JOIN dim_portfolios p ON p.rowid = portfolios_fts.rowid
                     WHERE portfolios_fts MATCH ?1
                     ORDER BY rank LIMIT ?2";
                let mut stmt = conn.prepare(sql)?;
                let rows = stmt.query_map(rusqlite::params![query_owned, options_limit], |row| {
                    let gid: String = row.get(0)?;
                    let snippet: Option<String> = row.get(2)?;
                    let stored_url: Option<String> = row.get(4)?;
                    Ok(SearchHit {
                        hit_type: SearchHitType::Portfolio,
                        gid: gid.clone(),
                        task_gid: None,
                        title: row.get(1)?,
                        snippet: snippet.unwrap_or_default(),
                        rank: row.get(3)?,
                        asana_url: stored_url,
                    })
                })?;
                for row in rows {
                    all_hits.push(row?);
                }
            }

            // Search custom fields
            if hit_type_filter.is_none() || hit_type_filter == Some(SearchHitType::CustomField) {
                let mut sql = String::from(
                    "SELECT cff.task_gid, t.name, cff.field_name, cff.display_value, cff.rank, t.permalink_url
                     FROM custom_fields_fts cff
                     LEFT JOIN fact_tasks t ON t.task_gid = cff.task_gid
                     WHERE custom_fields_fts MATCH ?1",
                );
                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
                    vec![Box::new(query_owned.clone())];

                if let Some(ref assignee) = assignee_filter {
                    let param_idx = params.len() + 1;
                    sql.push_str(&format!(" AND t.assignee_gid = ?{param_idx}"));
                    params.push(Box::new(assignee.clone()));
                }
                if let Some(ref project) = project_filter {
                    let param_idx = params.len() + 1;
                    sql.push_str(&format!(
                        " AND cff.task_gid IN (SELECT task_gid FROM bridge_task_projects WHERE project_gid = ?{param_idx})"
                    ));
                    params.push(Box::new(project.clone()));
                }
                sql.push_str(&format!(" ORDER BY rank LIMIT ?{}", params.len() + 1));
                params.push(Box::new(options_limit));

                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(param_refs.as_slice(), |row| {
                    let task_gid: String = row.get(0)?;
                    let task_name: Option<String> = row.get(1)?;
                    let field_name: String = row.get(2)?;
                    let display_value: String = row.get(3)?;
                    let stored_url: Option<String> = row.get(5)?;
                    Ok(SearchHit {
                        hit_type: SearchHitType::CustomField,
                        gid: task_gid.clone(),
                        task_gid: Some(task_gid.clone()),
                        title: format!(
                            "{}: {} = {}",
                            task_name.as_deref().unwrap_or("(unknown task)"),
                            field_name,
                            display_value
                        ),
                        snippet: display_value,
                        rank: row.get(4)?,
                        asana_url: stored_url.or_else(|| Some(format!("https://app.asana.com/0/0/{task_gid}"))),
                    })
                })?;
                for row in rows {
                    all_hits.push(row?);
                }
            }

            // Sort all hits by rank (lower = more relevant in FTS5)
            all_hits.sort_by(|a, b| a.rank.partial_cmp(&b.rank).unwrap_or(std::cmp::Ordering::Equal));

            // Trim to overall limit
            all_hits.truncate(options_limit as usize);

            let total = all_hits.len();
            Ok::<SearchResults, rusqlite::Error>(SearchResults {
                query: query_owned,
                hits: all_hits,
                total,
            })
        })
        .await?;

    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    #[tokio::test]
    async fn test_search_tasks() {
        let db = Database::open_memory().await.unwrap();

        // Insert a task with content to search
        db.writer()
            .call(|conn| {
                conn.execute(
                    "INSERT INTO dim_projects (project_gid, name, workspace_gid, cached_at)
                     VALUES ('p1', 'My Project', 'w1', datetime('now'))",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO dim_users (user_gid, name, cached_at)
                     VALUES ('u1', 'Alice', datetime('now'))",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO fact_tasks (task_gid, name, notes, assignee_gid, is_completed, created_at, created_date_key, modified_at, is_subtask, is_overdue, cached_at)
                     VALUES ('t1', 'Fix login bug', 'The login page crashes when clicking submit', 'u1', 0, '2025-01-01', '2025-01-01', '2025-01-01', 0, 0, datetime('now'))",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO bridge_task_projects (task_gid, project_gid) VALUES ('t1', 'p1')",
                    [],
                )?;
                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();

        let options = SearchOptions {
            limit: Some(10),
            hit_type: None,
            assignee_gid: None,
            project_gid: None,
        };

        let results = search(&db, "login", &options).await.unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].gid, "t1");
        assert_eq!(results.hits[0].hit_type, SearchHitType::Task);
    }

    #[tokio::test]
    async fn test_search_no_results() {
        let db = Database::open_memory().await.unwrap();

        let options = SearchOptions {
            limit: Some(10),
            hit_type: None,
            assignee_gid: None,
            project_gid: None,
        };

        let results = search(&db, "nonexistent", &options).await.unwrap();
        assert_eq!(results.total, 0);
        assert!(results.hits.is_empty());
    }

    #[tokio::test]
    async fn test_search_filter_by_type() {
        let db = Database::open_memory().await.unwrap();

        db.writer()
            .call(|conn| {
                conn.execute(
                    "INSERT INTO fact_tasks (task_gid, name, is_completed, created_at, created_date_key, modified_at, is_subtask, is_overdue, cached_at)
                     VALUES ('t1', 'Test widget feature', 0, '2025-01-01', '2025-01-01', '2025-01-01', 0, 0, datetime('now'))",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO dim_projects (project_gid, name, workspace_gid, notes, cached_at)
                     VALUES ('p1', 'Widget Project', 'w1', 'Build widget features', datetime('now'))",
                    [],
                )?;
                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();

        // Search for "widget" filtered to tasks only
        let options = SearchOptions {
            limit: Some(10),
            hit_type: Some(SearchHitType::Task),
            assignee_gid: None,
            project_gid: None,
        };
        let results = search(&db, "widget", &options).await.unwrap();
        assert!(results
            .hits
            .iter()
            .all(|h| h.hit_type == SearchHitType::Task));

        // Search for "widget" filtered to projects only
        let options = SearchOptions {
            limit: Some(10),
            hit_type: Some(SearchHitType::Project),
            assignee_gid: None,
            project_gid: None,
        };
        let results = search(&db, "widget", &options).await.unwrap();
        assert!(results
            .hits
            .iter()
            .all(|h| h.hit_type == SearchHitType::Project));
    }
}
