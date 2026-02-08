use asanaclient::Client;

use crate::error::Result;

/// Search for tasks in a workspace, optionally filtered by date and assignee.
/// Uses the workspace task search API endpoint.
pub async fn search_workspace_tasks(
    client: &Client,
    workspace_gid: &str,
    modified_since: Option<&str>,
    assignee_gid: Option<&str>,
) -> Result<Vec<asanaclient::Task>> {
    let mut query = vec![
        ("opt_fields", "gid,name,completed,completed_at,assignee,assignee.name,due_on,due_at,start_on,created_at,modified_at,notes,html_notes,parent,num_subtasks,num_likes,memberships,memberships.project,memberships.project.name,memberships.section,memberships.section.name,tags,tags.name,custom_fields,custom_fields.gid,custom_fields.name,custom_fields.display_value,custom_fields.resource_subtype,custom_fields.text_value,custom_fields.number_value,custom_fields.enum_value,custom_fields.enum_value.gid,custom_fields.date_value,custom_fields.date_value.date,custom_fields.date_value.date_time,permalink_url"),
    ];

    if let Some(since) = modified_since {
        query.push(("modified_since", since));
    }
    if let Some(assignee) = assignee_gid {
        query.push(("assignee.any", assignee));
    }

    let path = format!("/workspaces/{workspace_gid}/tasks/search");
    let tasks: Vec<asanaclient::Task> = client.get_all(&path, &query).await?;
    Ok(tasks)
}

/// Get sections for a project.
pub async fn get_project_sections(
    client: &Client,
    project_gid: &str,
) -> Result<Vec<SectionInfo>> {
    let path = format!("/projects/{project_gid}/sections");
    let query = [("opt_fields", "gid,name")];
    let sections: Vec<SectionInfo> = client.get_all(&path, &query).await?;
    Ok(sections)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SectionInfo {
    pub gid: String,
    pub name: String,
}

/// Get members of a team.
pub async fn get_team_members(
    client: &Client,
    team_gid: &str,
) -> Result<Vec<TeamMemberInfo>> {
    let path = format!("/teams/{team_gid}/users");
    let query = [("opt_fields", "gid,name,email")];
    let members: Vec<TeamMemberInfo> = client.get_all(&path, &query).await?;
    Ok(members)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TeamMemberInfo {
    pub gid: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// Get projects belonging to a team.
pub async fn get_team_projects(
    client: &Client,
    team_gid: &str,
) -> Result<Vec<ProjectRef>> {
    let path = format!("/teams/{team_gid}/projects");
    let query = [("opt_fields", "gid,name,archived")];
    let projects: Vec<ProjectRef> = client.get_all(&path, &query).await?;
    Ok(projects)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProjectRef {
    pub gid: String,
    pub name: String,
    #[serde(default)]
    pub archived: bool,
}
