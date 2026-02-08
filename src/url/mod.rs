use crate::error::{Error, Result};

/// Parsed information from an Asana URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsanaUrlInfo {
    Task {
        task_gid: String,
        project_gid: Option<String>,
    },
    Project {
        project_gid: String,
    },
    Portfolio {
        portfolio_gid: String,
    },
    Team {
        team_gid: String,
        workspace_gid: String,
    },
}

/// Parse an Asana URL into its component identifiers.
///
/// Supported URL patterns (legacy `/0/` format):
/// - `https://app.asana.com/0/portfolio/<portfolio_gid>/list`
/// - `https://app.asana.com/0/<project_gid>/<task_gid>`
/// - `https://app.asana.com/0/<project_gid>/list` (or /board, /timeline, /calendar)
/// - `https://app.asana.com/0/<project_gid>`
///
/// Supported URL patterns (new `/1/` format):
/// - `https://app.asana.com/1/<workspace_gid>/project/<project_gid>/...`
/// - `https://app.asana.com/1/<workspace_gid>/portfolio/<portfolio_gid>/...`
/// - `https://app.asana.com/1/<workspace_gid>/task/<task_gid>/...`
///
/// If the input is not a URL (no "asana.com"), returns an error.
pub fn parse_asana_url(input: &str) -> Result<AsanaUrlInfo> {
    let url = url::Url::parse(input).map_err(|e| Error::UrlParse(e.to_string()))?;

    let host = url.host_str().unwrap_or("");
    if !host.contains("asana.com") {
        return Err(Error::UrlParse(format!("not an Asana URL: {input}")));
    }

    let segments: Vec<&str> = url
        .path_segments()
        .map(|s| s.collect())
        .unwrap_or_default();

    match segments.first().copied() {
        Some("0") => parse_legacy_url(input, &segments[1..]),
        Some("1") => parse_new_url(input, &segments[1..]),
        _ => Err(Error::UrlParse(format!("unexpected URL format: {input}"))),
    }
}

/// Parse the new Asana URL format: /1/<workspace_gid>/<entity_type>/<entity_gid>/...
fn parse_new_url(input: &str, rest: &[&str]) -> Result<AsanaUrlInfo> {
    // rest[0] = workspace_gid, rest[1] = entity_type, rest[2] = entity_gid, rest[3..] = view
    let _workspace_gid = rest
        .first()
        .filter(|s| is_gid(s))
        .ok_or_else(|| Error::UrlParse(format!("missing workspace GID in URL: {input}")))?;

    let entity_type = rest.get(1).copied().unwrap_or("");
    let entity_gid = rest.get(2).copied().unwrap_or("");

    if !is_gid(entity_gid) {
        return Err(Error::UrlParse(format!(
            "missing entity GID in URL: {input}"
        )));
    }

    match entity_type {
        "project" => Ok(AsanaUrlInfo::Project {
            project_gid: entity_gid.to_string(),
        }),
        "portfolio" => Ok(AsanaUrlInfo::Portfolio {
            portfolio_gid: entity_gid.to_string(),
        }),
        "task" => Ok(AsanaUrlInfo::Task {
            task_gid: entity_gid.to_string(),
            project_gid: None,
        }),
        "team" => Ok(AsanaUrlInfo::Team {
            team_gid: entity_gid.to_string(),
            workspace_gid: _workspace_gid.to_string(),
        }),
        _ => Err(Error::UrlParse(format!(
            "unknown entity type '{entity_type}' in URL: {input}"
        ))),
    }
}

/// Parse the legacy Asana URL format: /0/...
fn parse_legacy_url(input: &str, rest: &[&str]) -> Result<AsanaUrlInfo> {
    // Portfolio URL: /0/portfolio/<gid>/list
    if rest.first() == Some(&"portfolio") {
        let gid = rest
            .get(1)
            .filter(|s| is_gid(s))
            .ok_or_else(|| Error::UrlParse(format!("missing portfolio GID in URL: {input}")))?;
        return Ok(AsanaUrlInfo::Portfolio {
            portfolio_gid: gid.to_string(),
        });
    }

    // Remaining patterns: /0/<seg1>/<seg2>[/...]
    let seg1 = rest.first().copied().unwrap_or("");
    let seg2 = rest.get(1).copied().unwrap_or("");

    if !is_gid(seg1) {
        return Err(Error::UrlParse(format!(
            "expected GID in URL path: {input}"
        )));
    }

    // /0/<project_gid>/list|board|timeline|calendar|...
    let view_suffixes = [
        "list",
        "board",
        "timeline",
        "calendar",
        "overview",
        "messages",
        "files",
        "progress",
        "",
    ];

    if seg2.is_empty() || view_suffixes.contains(&seg2) {
        return Ok(AsanaUrlInfo::Project {
            project_gid: seg1.to_string(),
        });
    }

    // /0/<project_gid>/<task_gid>
    if is_gid(seg2) {
        return Ok(AsanaUrlInfo::Task {
            task_gid: seg2.to_string(),
            project_gid: Some(seg1.to_string()),
        });
    }

    Err(Error::UrlParse(format!(
        "could not parse Asana URL: {input}"
    )))
}

/// Generate an Asana URL for a given entity type and GID.
pub fn generate_asana_url(entity_type: &str, gid: &str) -> String {
    match entity_type {
        "task" => format!("https://app.asana.com/0/0/{gid}"),
        "project" => format!("https://app.asana.com/0/{gid}"),
        "portfolio" => format!("https://app.asana.com/0/portfolio/{gid}/list"),
        _ => format!("https://app.asana.com/0/0/{gid}"),
    }
}

/// Check if a string looks like an Asana GID (all digits).
pub fn is_gid(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Extract a GID from either a raw GID or an Asana URL.
/// Returns the GID string.
pub fn resolve_gid(input: &str) -> Result<String> {
    if is_gid(input) {
        return Ok(input.to_string());
    }
    if input.contains("asana.com") {
        let info = parse_asana_url(input)?;
        match info {
            AsanaUrlInfo::Task { task_gid, .. } => Ok(task_gid),
            AsanaUrlInfo::Project { project_gid } => Ok(project_gid),
            AsanaUrlInfo::Portfolio { portfolio_gid } => Ok(portfolio_gid),
            AsanaUrlInfo::Team { team_gid, .. } => Ok(team_gid),
        }
    } else {
        // Might be a name or email — return as-is for later resolution
        Ok(input.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portfolio_url() {
        let info =
            parse_asana_url("https://app.asana.com/0/portfolio/1208241409266353/list").unwrap();
        assert_eq!(
            info,
            AsanaUrlInfo::Portfolio {
                portfolio_gid: "1208241409266353".to_string()
            }
        );
    }

    #[test]
    fn test_project_url_with_list() {
        let info = parse_asana_url("https://app.asana.com/0/1234567890/list").unwrap();
        assert_eq!(
            info,
            AsanaUrlInfo::Project {
                project_gid: "1234567890".to_string()
            }
        );
    }

    #[test]
    fn test_project_url_with_board() {
        let info = parse_asana_url("https://app.asana.com/0/1234567890/board").unwrap();
        assert_eq!(
            info,
            AsanaUrlInfo::Project {
                project_gid: "1234567890".to_string()
            }
        );
    }

    #[test]
    fn test_project_url_bare() {
        let info = parse_asana_url("https://app.asana.com/0/1234567890").unwrap();
        assert_eq!(
            info,
            AsanaUrlInfo::Project {
                project_gid: "1234567890".to_string()
            }
        );
    }

    #[test]
    fn test_task_url() {
        let info =
            parse_asana_url("https://app.asana.com/0/1234567890/9876543210").unwrap();
        assert_eq!(
            info,
            AsanaUrlInfo::Task {
                task_gid: "9876543210".to_string(),
                project_gid: Some("1234567890".to_string()),
            }
        );
    }

    #[test]
    fn test_new_format_project_url() {
        let info = parse_asana_url(
            "https://app.asana.com/1/1209759542559920/project/1209759542987106/list/1209759322889760",
        )
        .unwrap();
        assert_eq!(
            info,
            AsanaUrlInfo::Project {
                project_gid: "1209759542987106".to_string()
            }
        );
    }

    #[test]
    fn test_new_format_portfolio_url() {
        let info = parse_asana_url(
            "https://app.asana.com/1/1209759542559920/portfolio/1208241409266353/list",
        )
        .unwrap();
        assert_eq!(
            info,
            AsanaUrlInfo::Portfolio {
                portfolio_gid: "1208241409266353".to_string()
            }
        );
    }

    #[test]
    fn test_new_format_task_url() {
        let info = parse_asana_url(
            "https://app.asana.com/1/1209759542559920/task/9876543210",
        )
        .unwrap();
        assert_eq!(
            info,
            AsanaUrlInfo::Task {
                task_gid: "9876543210".to_string(),
                project_gid: None,
            }
        );
    }

    #[test]
    fn test_not_asana_url() {
        assert!(parse_asana_url("https://google.com/foo").is_err());
    }

    #[test]
    fn test_resolve_gid_raw() {
        assert_eq!(resolve_gid("1234567890").unwrap(), "1234567890");
    }

    #[test]
    fn test_resolve_gid_from_url() {
        assert_eq!(
            resolve_gid("https://app.asana.com/0/portfolio/1234567890/list").unwrap(),
            "1234567890"
        );
    }

    #[test]
    fn test_resolve_gid_email() {
        // Emails pass through as-is for later API resolution
        assert_eq!(
            resolve_gid("user@example.com").unwrap(),
            "user@example.com"
        );
    }

    #[test]
    fn test_is_gid() {
        assert!(is_gid("1234567890"));
        assert!(!is_gid(""));
        assert!(!is_gid("abc"));
        assert!(!is_gid("123abc"));
    }
}
