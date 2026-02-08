use serde::Serialize;

use crate::error::Result;
use crate::storage::Database;

/// A row from a task query.
#[derive(Debug, Clone, Serialize)]
pub struct TaskRow {
    pub task_gid: String,
    pub name: String,
    pub assignee_gid: Option<String>,
    pub assignee_name: Option<String>,
    pub is_completed: bool,
    pub completed_at: Option<String>,
    pub due_on: Option<String>,
    pub created_at: String,
    pub modified_at: Option<String>,
    pub project_name: Option<String>,
    pub section_name: Option<String>,
    pub is_overdue: bool,
    pub days_to_complete: Option<i32>,
    pub num_subtasks: i32,
    pub num_likes: i32,
    pub permalink_url: Option<String>,
}

/// Builder for constructing task queries with optional filters.
#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
    project_gid: Option<String>,
    portfolio_gid: Option<String>,
    team_gid: Option<String>,
    assignee_gid: Option<String>,
    completed: Option<bool>,
    overdue: Option<bool>,
    created_after: Option<String>,
    created_before: Option<String>,
    completed_after: Option<String>,
    completed_before: Option<String>,
    due_after: Option<String>,
    due_before: Option<String>,
    has_assignee: Option<bool>,
    is_subtask: Option<bool>,
    tag_name: Option<String>,
    limit: Option<u32>,
    order_by: Option<String>,
    order_desc: bool,
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn project(mut self, gid: &str) -> Self {
        self.project_gid = Some(gid.to_string());
        self
    }

    pub fn portfolio(mut self, gid: &str) -> Self {
        self.portfolio_gid = Some(gid.to_string());
        self
    }

    pub fn team(mut self, gid: &str) -> Self {
        self.team_gid = Some(gid.to_string());
        self
    }

    pub fn assignee(mut self, gid: &str) -> Self {
        self.assignee_gid = Some(gid.to_string());
        self
    }

    pub fn completed(mut self, val: bool) -> Self {
        self.completed = Some(val);
        self
    }

    pub fn overdue(mut self, val: bool) -> Self {
        self.overdue = Some(val);
        self
    }

    pub fn created_after(mut self, date: &str) -> Self {
        self.created_after = Some(date.to_string());
        self
    }

    pub fn created_before(mut self, date: &str) -> Self {
        self.created_before = Some(date.to_string());
        self
    }

    pub fn completed_after(mut self, date: &str) -> Self {
        self.completed_after = Some(date.to_string());
        self
    }

    pub fn completed_before(mut self, date: &str) -> Self {
        self.completed_before = Some(date.to_string());
        self
    }

    pub fn due_after(mut self, date: &str) -> Self {
        self.due_after = Some(date.to_string());
        self
    }

    pub fn due_before(mut self, date: &str) -> Self {
        self.due_before = Some(date.to_string());
        self
    }

    pub fn has_assignee(mut self, val: bool) -> Self {
        self.has_assignee = Some(val);
        self
    }

    pub fn is_subtask(mut self, val: bool) -> Self {
        self.is_subtask = Some(val);
        self
    }

    pub fn tag(mut self, name: &str) -> Self {
        self.tag_name = Some(name.to_string());
        self
    }

    pub fn limit(mut self, n: u32) -> Self {
        self.limit = Some(n);
        self
    }

    pub fn order_by(mut self, field: &str) -> Self {
        self.order_by = Some(field.to_string());
        self
    }

    pub fn descending(mut self) -> Self {
        self.order_desc = true;
        self
    }

    /// Build and execute the query, returning task rows.
    pub async fn tasks(self, db: &Database) -> Result<Vec<TaskRow>> {
        let builder = self;
        db.reader()
            .call(move |conn| {
                let (sql, params) = builder.build_sql();
                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(param_refs.as_slice(), |row| {
                    Ok(TaskRow {
                        task_gid: row.get(0)?,
                        name: row.get(1)?,
                        assignee_gid: row.get(2)?,
                        assignee_name: row.get(3)?,
                        is_completed: row.get::<_, i32>(4)? != 0,
                        completed_at: row.get(5)?,
                        due_on: row.get(6)?,
                        created_at: row.get(7)?,
                        modified_at: row.get(8)?,
                        project_name: row.get(9)?,
                        section_name: row.get(10)?,
                        is_overdue: row.get::<_, i32>(11)? != 0,
                        days_to_complete: row.get(12)?,
                        num_subtasks: row.get(13)?,
                        num_likes: row.get(14)?,
                        permalink_url: row.get(15)?,
                    })
                })?;
                let result: std::result::Result<Vec<TaskRow>, _> = rows.collect();
                result
            })
            .await
            .map_err(|e| crate::error::Error::Database(e.to_string()))
    }

    /// Build and execute the query, returning a count of matching tasks.
    pub async fn count(self, db: &Database) -> Result<u64> {
        let builder = self;
        db.reader()
            .call(move |conn| {
                let (inner_sql, params) = builder.build_sql();
                let sql = format!("SELECT COUNT(*) FROM ({inner_sql})");
                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                let count: i64 = conn.query_row(&sql, param_refs.as_slice(), |row| row.get(0))?;
                Ok::<u64, rusqlite::Error>(count as u64)
            })
            .await
            .map_err(|e| crate::error::Error::Database(e.to_string()))
    }

    /// Build and execute the query, returning results as JSON.
    pub async fn to_json(self, db: &Database) -> Result<String> {
        let rows = self.tasks(db).await?;
        serde_json::to_string_pretty(&rows)
            .map_err(|e| crate::error::Error::Other(e.to_string()))
    }

    /// Build and execute the query, returning results as CSV.
    pub async fn to_csv(self, db: &Database) -> Result<String> {
        let rows = self.tasks(db).await?;
        let mut out = String::new();
        out.push_str("task_gid,name,assignee_gid,assignee_name,is_completed,completed_at,due_on,created_at,modified_at,project_name,section_name,is_overdue,days_to_complete,num_subtasks,num_likes,permalink_url\n");
        for row in &rows {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                csv_escape(&row.task_gid),
                csv_escape(&row.name),
                csv_escape(row.assignee_gid.as_deref().unwrap_or("")),
                csv_escape(row.assignee_name.as_deref().unwrap_or("")),
                row.is_completed,
                csv_escape(row.completed_at.as_deref().unwrap_or("")),
                csv_escape(row.due_on.as_deref().unwrap_or("")),
                csv_escape(&row.created_at),
                csv_escape(row.modified_at.as_deref().unwrap_or("")),
                csv_escape(row.project_name.as_deref().unwrap_or("")),
                csv_escape(row.section_name.as_deref().unwrap_or("")),
                row.is_overdue,
                row.days_to_complete.map_or(String::new(), |d| d.to_string()),
                row.num_subtasks,
                row.num_likes,
                csv_escape(row.permalink_url.as_deref().unwrap_or("")),
            ));
        }
        Ok(out)
    }

    fn build_sql(&self) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut joins = Vec::new();
        let mut wheres = Vec::new();
        let mut param_idx = 1;

        // Base query
        let select = "SELECT t.task_gid, t.name, t.assignee_gid, u.name as assignee_name,
                t.is_completed, t.completed_at, t.due_on, t.created_at, t.modified_at,
                p.name as project_name, s.name as section_name,
                t.is_overdue, t.days_to_complete, t.num_subtasks, t.num_likes,
                t.permalink_url
            FROM fact_tasks t
            LEFT JOIN dim_users u ON u.user_gid = t.assignee_gid
            LEFT JOIN bridge_task_projects btp ON btp.task_gid = t.task_gid
            LEFT JOIN dim_projects p ON p.project_gid = btp.project_gid
            LEFT JOIN dim_sections s ON s.section_gid = btp.section_gid";

        // Project filter
        if let Some(ref gid) = self.project_gid {
            wheres.push(format!("btp.project_gid = ?{param_idx}"));
            params.push(Box::new(gid.clone()));
            param_idx += 1;
        }

        // Portfolio filter (join through bridge)
        if let Some(ref gid) = self.portfolio_gid {
            joins.push(format!(
                "JOIN bridge_portfolio_projects bpp ON bpp.project_gid = btp.project_gid AND bpp.portfolio_gid = ?{param_idx}"
            ));
            params.push(Box::new(gid.clone()));
            param_idx += 1;
        }

        // Team filter (join through bridge)
        if let Some(ref gid) = self.team_gid {
            joins.push(format!(
                "JOIN bridge_team_members btm ON btm.user_gid = t.assignee_gid AND btm.team_gid = ?{param_idx}"
            ));
            params.push(Box::new(gid.clone()));
            param_idx += 1;
        }

        // Assignee filter
        if let Some(ref gid) = self.assignee_gid {
            wheres.push(format!("t.assignee_gid = ?{param_idx}"));
            params.push(Box::new(gid.clone()));
            param_idx += 1;
        }

        // Completed filter
        if let Some(completed) = self.completed {
            wheres.push(format!("t.is_completed = ?{param_idx}"));
            params.push(Box::new(completed as i32));
            param_idx += 1;
        }

        // Overdue filter
        if let Some(overdue) = self.overdue {
            wheres.push(format!("t.is_overdue = ?{param_idx}"));
            params.push(Box::new(overdue as i32));
            param_idx += 1;
        }

        // Date range filters
        if let Some(ref date) = self.created_after {
            wheres.push(format!("t.created_date_key >= ?{param_idx}"));
            params.push(Box::new(date.clone()));
            param_idx += 1;
        }
        if let Some(ref date) = self.created_before {
            wheres.push(format!("t.created_date_key <= ?{param_idx}"));
            params.push(Box::new(date.clone()));
            param_idx += 1;
        }
        if let Some(ref date) = self.completed_after {
            wheres.push(format!("t.completed_date_key >= ?{param_idx}"));
            params.push(Box::new(date.clone()));
            param_idx += 1;
        }
        if let Some(ref date) = self.completed_before {
            wheres.push(format!("t.completed_date_key <= ?{param_idx}"));
            params.push(Box::new(date.clone()));
            param_idx += 1;
        }
        if let Some(ref date) = self.due_after {
            wheres.push(format!("t.due_on >= ?{param_idx}"));
            params.push(Box::new(date.clone()));
            param_idx += 1;
        }
        if let Some(ref date) = self.due_before {
            wheres.push(format!("t.due_on <= ?{param_idx}"));
            params.push(Box::new(date.clone()));
            param_idx += 1;
        }

        // Has assignee
        if let Some(has) = self.has_assignee {
            if has {
                wheres.push("t.assignee_gid IS NOT NULL".to_string());
            } else {
                wheres.push("t.assignee_gid IS NULL".to_string());
            }
        }

        // Is subtask
        if let Some(is_sub) = self.is_subtask {
            wheres.push(format!("t.is_subtask = ?{param_idx}"));
            params.push(Box::new(is_sub as i32));
            param_idx += 1;
        }

        // Tag filter
        if let Some(ref tag) = self.tag_name {
            joins.push(format!(
                "JOIN bridge_task_tags btt ON btt.task_gid = t.task_gid AND btt.tag_name = ?{param_idx}"
            ));
            params.push(Box::new(tag.clone()));
            param_idx += 1;
        }

        // Assemble SQL
        let mut sql = select.to_string();
        for join in &joins {
            sql.push(' ');
            sql.push_str(join);
        }
        if !wheres.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&wheres.join(" AND "));
        }

        // GROUP BY to deduplicate when task is in multiple projects
        sql.push_str(" GROUP BY t.task_gid");

        // ORDER BY
        let order_field = self.order_by.as_deref().unwrap_or("t.modified_at");
        let order_dir = if self.order_desc { "DESC" } else { "ASC" };
        sql.push_str(&format!(" ORDER BY {order_field} {order_dir}"));

        // LIMIT
        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT ?{param_idx}"));
            params.push(Box::new(limit));
            // param_idx += 1; // unused after this point
        }

        (sql, params)
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_sql_default() {
        let builder = QueryBuilder::new();
        let (sql, params) = builder.build_sql();
        assert!(sql.contains("FROM fact_tasks t"));
        assert!(sql.contains("GROUP BY t.task_gid"));
        assert!(sql.contains("ORDER BY t.modified_at ASC"));
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_sql_with_filters() {
        let builder = QueryBuilder::new()
            .project("123")
            .completed(true)
            .limit(10)
            .order_by("t.created_at")
            .descending();
        let (sql, params) = builder.build_sql();
        assert!(sql.contains("btp.project_gid = ?1"));
        assert!(sql.contains("t.is_completed = ?2"));
        assert!(sql.contains("ORDER BY t.created_at DESC"));
        assert!(sql.contains("LIMIT ?3"));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_csv_escape() {
        assert_eq!(csv_escape("hello"), "hello");
        assert_eq!(csv_escape("hello,world"), "\"hello,world\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
