use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "asanadw", about = "Asana data warehouse CLI")]
struct Cli {
    /// Database path (default: ~/.asanadw/asanadw.db)
    #[arg(long)]
    db: Option<String>,

    /// Increase logging verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Page size for Asana API requests (default: API default of 100)
    #[arg(long)]
    page_size: Option<u32>,

    #[command(subcommand)]
    command: Commands,
}

/// Progress reporter that writes to stderr.
struct StderrProgress;

impl asanadw::SyncProgress for StderrProgress {
    fn on_entity_start(&self, entity_key: &str, index: usize, total: usize) {
        eprintln!("[{}/{}] Syncing {}...", index + 1, total, entity_key);
    }

    fn on_tasks_fetched(&self, _entity_key: &str, count: usize) {
        eprintln!("  Fetched {} tasks", count);
    }

    fn on_comments_progress(&self, _entity_key: &str, current: usize, total: usize) {
        if current == total {
            eprint!("\r  Fetching comments: {}/{}   \n", current, total);
        } else {
            eprint!("\r  Fetching comments: {}/{}   ", current, total);
        }
    }

    fn on_entity_complete(&self, report: &asanadw::SyncReport) {
        eprintln!("  Done: {} items synced", report.items_synced);
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Sync Asana data to the local warehouse
    Sync {
        #[command(subcommand)]
        target: SyncTarget,
    },
    /// Manage monitored entities
    Monitor {
        #[command(subcommand)]
        action: MonitorAction,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Search across all synced data
    Search {
        /// Search query
        query: String,
        /// Filter by type: task, comment, project, custom_field
        #[arg(long, value_name = "TYPE")]
        r#type: Option<String>,
        /// Filter by assignee GID or email
        #[arg(long)]
        assignee: Option<String>,
        /// Filter to tasks assigned to the current user
        #[arg(long)]
        mine: bool,
        /// Filter by project GID
        #[arg(long)]
        project: Option<String>,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: u32,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Query tasks with filters
    Query {
        /// Filter by project GID
        #[arg(long)]
        project: Option<String>,
        /// Filter by portfolio GID
        #[arg(long)]
        portfolio: Option<String>,
        /// Filter by team GID
        #[arg(long)]
        team: Option<String>,
        /// Filter by assignee GID or email
        #[arg(long)]
        assignee: Option<String>,
        /// Filter to tasks assigned to the current user
        #[arg(long)]
        mine: bool,
        /// Filter completed tasks only
        #[arg(long)]
        completed: bool,
        /// Filter incomplete tasks only
        #[arg(long)]
        incomplete: bool,
        /// Filter overdue tasks only
        #[arg(long)]
        overdue: bool,
        /// Created after date (YYYY-MM-DD)
        #[arg(long)]
        created_after: Option<String>,
        /// Created before date (YYYY-MM-DD)
        #[arg(long)]
        created_before: Option<String>,
        /// Due after date (YYYY-MM-DD)
        #[arg(long)]
        due_after: Option<String>,
        /// Due before date (YYYY-MM-DD)
        #[arg(long)]
        due_before: Option<String>,
        /// Maximum results
        #[arg(long, default_value = "100")]
        limit: u32,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Output as CSV
        #[arg(long)]
        csv: bool,
        /// Count only (no output rows)
        #[arg(long)]
        count: bool,
    },
    /// Generate LLM-powered summaries
    Summarize {
        #[command(subcommand)]
        target: SummarizeTarget,
    },
    /// Compute metrics for an entity over a period
    Metrics {
        #[command(subcommand)]
        target: MetricsTarget,
    },
    /// Show warehouse status
    Status,
}

#[derive(Subcommand)]
enum MetricsTarget {
    /// Metrics for the current user
    Me {
        /// Period (e.g. 2024-Q1, 2024-M03, ytd, rolling-30d)
        #[arg(long, default_value = "qtd")]
        period: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Metrics for a user
    User {
        /// User GID or email address
        #[arg(value_name = "USER_GID_OR_EMAIL")]
        user_gid: String,
        /// Period (e.g. 2024-Q1, 2024-M03, ytd, rolling-30d)
        #[arg(long, default_value = "qtd")]
        period: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Metrics for a project
    Project {
        /// Project GID or Asana URL
        #[arg(value_name = "PROJECT_GID_OR_URL")]
        project_gid: String,
        #[arg(long, default_value = "qtd")]
        period: String,
        #[arg(long)]
        json: bool,
    },
    /// Metrics for a portfolio
    Portfolio {
        /// Portfolio GID or Asana URL
        #[arg(value_name = "PORTFOLIO_GID_OR_URL")]
        portfolio_gid: String,
        #[arg(long, default_value = "qtd")]
        period: String,
        #[arg(long)]
        json: bool,
    },
    /// Metrics for a team
    Team {
        /// Team GID or Asana URL
        #[arg(value_name = "TEAM_GID_OR_URL")]
        team_gid: String,
        #[arg(long, default_value = "qtd")]
        period: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum SummarizeTarget {
    /// Summarize the current user's period
    Me {
        #[arg(long, default_value = "qtd")]
        period: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Summarize a single task
    Task {
        /// Task GID or Asana URL
        #[arg(value_name = "TASK_GID_OR_URL")]
        task_gid: String,
        /// Force regeneration (ignore cache)
        #[arg(long)]
        force: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Summarize a user's period
    User {
        /// User GID or email address
        #[arg(value_name = "USER_GID_OR_EMAIL")]
        user_gid: String,
        #[arg(long, default_value = "qtd")]
        period: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Summarize a project's period
    Project {
        /// Project GID or Asana URL
        #[arg(value_name = "PROJECT_GID_OR_URL")]
        project_gid: String,
        #[arg(long, default_value = "qtd")]
        period: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Summarize a portfolio's period
    Portfolio {
        /// Portfolio GID or Asana URL
        #[arg(value_name = "PORTFOLIO_GID_OR_URL")]
        portfolio_gid: String,
        #[arg(long, default_value = "qtd")]
        period: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Summarize a team's period
    Team {
        /// Team GID or Asana URL
        #[arg(value_name = "TEAM_GID_OR_URL")]
        team_gid: String,
        #[arg(long, default_value = "qtd")]
        period: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum SyncTarget {
    /// Sync a project's tasks
    Project {
        /// Project GID or Asana URL
        identifier: String,
        /// Number of days to look back
        #[arg(long)]
        days: Option<u32>,
        /// Sync data since this date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,
    },
    /// Sync a user's tasks
    User {
        /// User GID or email
        identifier: String,
        #[arg(long)]
        days: Option<u32>,
        #[arg(long)]
        since: Option<String>,
    },
    /// Sync a team's projects and members
    Team {
        /// Team GID or Asana URL
        identifier: String,
        #[arg(long)]
        days: Option<u32>,
        #[arg(long)]
        since: Option<String>,
    },
    /// Sync a portfolio's projects
    Portfolio {
        /// Portfolio GID or Asana URL
        identifier: String,
        #[arg(long)]
        days: Option<u32>,
        #[arg(long)]
        since: Option<String>,
    },
    /// Sync all monitored entities
    All {
        #[arg(long)]
        days: Option<u32>,
        #[arg(long)]
        since: Option<String>,
    },
}

#[derive(Subcommand)]
enum MonitorAction {
    /// Add an entity to monitoring
    Add {
        /// Entity type: project, user, team, portfolio
        entity_type: String,
        /// Entity GID or Asana URL
        identifier: String,
    },
    /// Add all favorited projects and portfolios to monitoring
    AddFavorites,
    /// Remove an entity from monitoring
    Remove {
        /// Entity key (e.g. project:123456)
        entity_key: String,
    },
    /// List monitored entities
    List,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Get a config value
    Get { key: String },
    /// Set a config value
    Set { key: String, value: String },
    /// List all config values
    List,
}

fn parse_since(since: Option<&str>) -> Option<chrono::NaiveDate> {
    since.and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
}

/// Resolve a user identifier (GID, email, or name) to a numeric GID via the database.
/// Falls back to returning the original identifier if no match is found in the DB.
async fn resolve_user(db: &asanadw::Database, identifier: &str) -> anyhow::Result<String> {
    let id = identifier.to_string();
    let resolved = db
        .reader()
        .call(move |conn| {
            asanadw::storage::repository::resolve_user_identifier(conn, &id)
        })
        .await?;
    match resolved {
        Some(gid) => Ok(gid),
        None => {
            log::warn!("Could not resolve user '{identifier}' in local database — using as-is");
            Ok(identifier.to_string())
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let level = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

    let db = match &cli.db {
        Some(path) => asanadw::Database::open_at(path).await?,
        None => asanadw::Database::open().await?,
    };

    match cli.command {
        Commands::Status => {
            print_status(&db).await?;
        }
        Commands::Config { action } => {
            handle_config(&db, action).await?;
        }
        Commands::Search {
            query,
            r#type,
            assignee,
            mine,
            project,
            limit,
            json,
        } => {
            let effective_assignee = if mine {
                let gid = db.reader().call(|c| asanadw::storage::repository::get_config(c, "user_gid")).await?
                    .ok_or_else(|| anyhow::anyhow!("User identity not set. Run 'asanadw sync all' first."))?;
                Some(gid)
            } else {
                assignee
            };
            handle_search(&db, &query, r#type.as_deref(), effective_assignee.as_deref(), project.as_deref(), limit, json).await?;
        }
        Commands::Query {
            project,
            portfolio,
            team,
            assignee,
            mine,
            completed,
            incomplete,
            overdue,
            created_after,
            created_before,
            due_after,
            due_before,
            limit,
            json,
            csv,
            count,
        } => {
            let effective_assignee = if mine {
                let gid = db.reader().call(|c| asanadw::storage::repository::get_config(c, "user_gid")).await?
                    .ok_or_else(|| anyhow::anyhow!("User identity not set. Run 'asanadw sync all' first."))?;
                Some(gid)
            } else {
                assignee
            };
            handle_query(
                &db, project.as_deref(), portfolio.as_deref(), team.as_deref(),
                effective_assignee.as_deref(), completed, incomplete, overdue,
                created_after.as_deref(), created_before.as_deref(),
                due_after.as_deref(), due_before.as_deref(),
                limit, json, csv, count,
            ).await?;
        }
        Commands::Summarize { target } => {
            handle_summarize(&db, target).await?;
        }
        Commands::Metrics { target } => {
            handle_metrics(&db, target).await?;
        }
        Commands::Monitor { action } => {
            let client = asanaclient::Client::from_env()?;
            let dw = asanadw::AsanaDW::new(db, client);
            handle_monitor(&dw, action).await?;
        }
        Commands::Sync { target } => {
            let mut client = asanaclient::Client::from_env()?;
            if let Some(ps) = cli.page_size {
                client = client.with_page_size(ps);
            }
            let dw = asanadw::AsanaDW::new(db, client);
            handle_sync(&dw, target).await?;
        }
    }

    Ok(())
}

async fn print_status(db: &asanadw::Database) -> anyhow::Result<()> {
    let stats = db
        .reader()
        .call(|conn| {
            let tasks: i64 =
                conn.query_row("SELECT COUNT(*) FROM fact_tasks", [], |row| row.get(0))?;
            let projects: i64 = conn.query_row("SELECT COUNT(*) FROM dim_projects", [], |row| {
                row.get(0)
            })?;
            let users: i64 =
                conn.query_row("SELECT COUNT(*) FROM dim_users", [], |row| row.get(0))?;
            let comments: i64 = conn.query_row("SELECT COUNT(*) FROM fact_comments", [], |row| {
                row.get(0)
            })?;
            let monitored: i64 = conn.query_row(
                "SELECT COUNT(*) FROM monitored_entities WHERE sync_enabled = 1",
                [],
                |row| row.get(0),
            )?;

            let last_sync: Option<String> = conn
                .query_row(
                    "SELECT MAX(completed_at) FROM sync_jobs WHERE status = 'completed'",
                    [],
                    |row| row.get(0),
                )
                .ok();

            Ok::<_, rusqlite::Error>((tasks, projects, users, comments, monitored, last_sync))
        })
        .await?;

    let (tasks, projects, users, comments, monitored, last_sync) = stats;
    println!("Warehouse Status");
    println!("  Tasks:     {tasks}");
    println!("  Projects:  {projects}");
    println!("  Users:     {users}");
    println!("  Comments:  {comments}");
    println!("  Monitored: {monitored}");
    println!(
        "  Last sync: {}",
        last_sync.unwrap_or_else(|| "never".to_string())
    );
    Ok(())
}

async fn handle_config(db: &asanadw::Database, action: ConfigAction) -> anyhow::Result<()> {
    match action {
        ConfigAction::Get { key } => {
            let val: Option<String> = db
                .reader()
                .call({
                    let key = key.clone();
                    move |conn| {
                        asanadw::storage::repository::get_config(conn, &key)
                    }
                })
                .await?;
            match val {
                Some(v) => println!("{key} = {v}"),
                None => println!("{key} is not set"),
            }
        }
        ConfigAction::Set { key, value } => {
            db.writer()
                .call(move |conn| {
                    asanadw::storage::repository::set_config(conn, &key, &value)?;
                    Ok::<(), rusqlite::Error>(())
                })
                .await?;
            println!("Config updated.");
        }
        ConfigAction::List => {
            let items: Vec<(String, String)> = db
                .reader()
                .call(|conn| {
                    asanadw::storage::repository::list_config(conn)
                })
                .await?;
            if items.is_empty() {
                println!("No configuration set.");
            } else {
                for (k, v) in items {
                    println!("{k} = {v}");
                }
            }
        }
    }
    Ok(())
}

async fn handle_monitor(dw: &asanadw::AsanaDW, action: MonitorAction) -> anyhow::Result<()> {
    match action {
        MonitorAction::Add {
            entity_type,
            identifier,
        } => {
            let key = dw.monitor_add(&entity_type, &identifier).await?;
            println!("Added: {key}");
        }
        MonitorAction::AddFavorites => {
            let keys = dw.monitor_add_favorites().await?;
            if keys.is_empty() {
                println!("No favorited projects or portfolios found.");
            } else {
                println!("Added {} favorites:", keys.len());
                for key in &keys {
                    println!("  {key}");
                }
            }
        }
        MonitorAction::Remove { entity_key } => {
            let removed = dw.monitor_remove(&entity_key).await?;
            if removed {
                println!("Removed: {entity_key}");
            } else {
                println!("Not found: {entity_key}");
            }
        }
        MonitorAction::List => {
            let entities = dw.monitor_list().await?;
            if entities.is_empty() {
                println!("No monitored entities.");
            } else {
                for e in entities {
                    let name = e.display_name.as_deref().unwrap_or("");
                    let last = e.last_sync_at.as_deref().unwrap_or("never");
                    println!("{} {} (last sync: {})", e.entity_key, name, last);
                }
            }
        }
    }
    Ok(())
}

async fn handle_sync(dw: &asanadw::AsanaDW, target: SyncTarget) -> anyhow::Result<()> {
    let progress = StderrProgress;
    match target {
        SyncTarget::Project { identifier, days, since } => {
            let options = make_sync_options(days, since.as_deref());
            let report = dw.sync_project(&identifier, &options, &progress).await?;
            print_sync_report(&report);
        }
        SyncTarget::User { identifier, days, since } => {
            let options = make_sync_options(days, since.as_deref());
            let report = dw.sync_user(&identifier, &options, &progress).await?;
            print_sync_report(&report);
        }
        SyncTarget::Team { identifier, days, since } => {
            let options = make_sync_options(days, since.as_deref());
            let report = dw.sync_team(&identifier, &options, &progress).await?;
            print_sync_report(&report);
        }
        SyncTarget::Portfolio { identifier, days, since } => {
            let options = make_sync_options(days, since.as_deref());
            let report = dw.sync_portfolio(&identifier, &options, &progress).await?;
            print_sync_report(&report);
        }
        SyncTarget::All { days, since } => {
            let options = make_sync_options(days, since.as_deref());
            let reports = dw.sync_all(&options, &progress).await?;
            for report in &reports {
                print_sync_report(report);
                println!();
            }
            if reports.is_empty() {
                println!("No monitored entities to sync. Use 'monitor add' first.");
            }
        }
    }
    Ok(())
}

fn make_sync_options(days: Option<u32>, since: Option<&str>) -> asanadw::SyncOptions {
    asanadw::SyncOptions {
        since: parse_since(since),
        days,
    }
}

async fn handle_search(
    db: &asanadw::Database,
    query: &str,
    hit_type: Option<&str>,
    assignee: Option<&str>,
    project: Option<&str>,
    limit: u32,
    json: bool,
) -> anyhow::Result<()> {
    let type_filter = match hit_type {
        Some("task") => Some(asanadw::SearchHitType::Task),
        Some("comment") => Some(asanadw::SearchHitType::Comment),
        Some("project") => Some(asanadw::SearchHitType::Project),
        Some("custom_field") => Some(asanadw::SearchHitType::CustomField),
        Some(other) => anyhow::bail!("Unknown search type: {other}. Use: task, comment, project, custom_field"),
        None => None,
    };

    let resolved_assignee = match assignee {
        Some(a) => Some(resolve_user(db, a).await?),
        None => None,
    };
    let options = asanadw::SearchOptions {
        limit: Some(limit),
        hit_type: type_filter,
        assignee_gid: resolved_assignee,
        project_gid: project.map(|s| s.to_string()),
    };

    let results = asanadw::search::search(db, query, &options).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        println!("Search: \"{}\" ({} results)", results.query, results.total);
        for hit in &results.hits {
            let type_label = match hit.hit_type {
                asanadw::SearchHitType::Task => "task",
                asanadw::SearchHitType::Comment => "comment",
                asanadw::SearchHitType::Project => "project",
                asanadw::SearchHitType::CustomField => "field",
            };
            println!("  [{type_label}] {} ({})", hit.title, hit.gid);
            println!("    {}", hit.snippet);
            if let Some(ref url) = hit.asana_url {
                println!("    {url}");
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_query(
    db: &asanadw::Database,
    project: Option<&str>,
    portfolio: Option<&str>,
    team: Option<&str>,
    assignee: Option<&str>,
    completed: bool,
    incomplete: bool,
    overdue: bool,
    created_after: Option<&str>,
    created_before: Option<&str>,
    due_after: Option<&str>,
    due_before: Option<&str>,
    limit: u32,
    json: bool,
    csv: bool,
    count: bool,
) -> anyhow::Result<()> {
    let mut builder = asanadw::QueryBuilder::new().limit(limit).order_by("t.modified_at").descending();

    if let Some(p) = project {
        builder = builder.project(p);
    }
    if let Some(p) = portfolio {
        builder = builder.portfolio(p);
    }
    if let Some(t) = team {
        builder = builder.team(t);
    }
    if let Some(a) = assignee {
        let resolved = resolve_user(db, a).await?;
        builder = builder.assignee(&resolved);
    }
    if completed {
        builder = builder.completed(true);
    }
    if incomplete {
        builder = builder.completed(false);
    }
    if overdue {
        builder = builder.overdue(true);
    }
    if let Some(d) = created_after {
        builder = builder.created_after(d);
    }
    if let Some(d) = created_before {
        builder = builder.created_before(d);
    }
    if let Some(d) = due_after {
        builder = builder.due_after(d);
    }
    if let Some(d) = due_before {
        builder = builder.due_before(d);
    }

    if count {
        let n = builder.count(db).await?;
        println!("{n}");
    } else if json {
        let output = builder.to_json(db).await?;
        println!("{output}");
    } else if csv {
        let output = builder.to_csv(db).await?;
        print!("{output}");
    } else {
        let rows = builder.tasks(db).await?;
        if rows.is_empty() {
            println!("No tasks found.");
        } else {
            for row in &rows {
                let status = if row.is_completed { "done" } else { "open" };
                let assignee = row.assignee_name.as_deref().unwrap_or("unassigned");
                let project_name = row.project_name.as_deref().unwrap_or("");
                let due = row.due_on.as_deref().unwrap_or("no due date");
                println!(
                    "[{status}] {} ({}) - {assignee} | {project_name} | due: {due}",
                    row.name, row.task_gid
                );
            }
            println!("\n{} tasks", rows.len());
        }
    }

    Ok(())
}

async fn handle_summarize(db: &asanadw::Database, target: SummarizeTarget) -> anyhow::Result<()> {
    let agent = asanadw::llm::create_agent(db).await?;

    match target {
        SummarizeTarget::Me { period, force, json } => {
            let user_gid = db.reader().call(|c| asanadw::storage::repository::get_config(c, "user_gid")).await?
                .ok_or_else(|| anyhow::anyhow!("User identity not set. Run 'asanadw sync all' first."))?;
            let p = asanadw::Period::parse(&period)?;
            let summary = asanadw::llm::agents::period::summarize_user_period(db, &agent, &user_gid, &p, force).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("{}", summary.headline);
                println!("\n{}", summary.what_changed);
                println!("\n{}", summary.why_it_matters);
                println!("\nKey accomplishments:");
                for a in &summary.key_accomplishments {
                    println!("  - {a}");
                }
                if let Some(ref notes) = summary.collaboration_notes {
                    println!("\nCollaboration: {notes}");
                }
            }
        }
        SummarizeTarget::Task { task_gid, force, json } => {
            let summary = asanadw::llm::agents::task::summarize_task(db, &agent, &task_gid, force).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("Task Summary: {}", summary.headline);
                println!("\nWhat happened: {}", summary.what_happened);
                println!("Why it matters: {}", summary.why_it_matters);
                println!("Complexity: {}", summary.complexity_signal);
                println!("Notability: {}/10", summary.notability_score);
                println!("Types: {}", summary.change_types.join(", "));
            }
        }
        SummarizeTarget::User { user_gid, period, force, json } => {
            let user_gid = resolve_user(db, &user_gid).await?;
            let p = asanadw::Period::parse(&period)?;
            let summary = asanadw::llm::agents::period::summarize_user_period(db, &agent, &user_gid, &p, force).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("{}", summary.headline);
                println!("\n{}", summary.what_changed);
                println!("\n{}", summary.why_it_matters);
                println!("\nKey accomplishments:");
                for a in &summary.key_accomplishments {
                    println!("  - {a}");
                }
                if let Some(ref notes) = summary.collaboration_notes {
                    println!("\nCollaboration: {notes}");
                }
            }
        }
        SummarizeTarget::Project { project_gid, period, force, json } => {
            let p = asanadw::Period::parse(&period)?;
            let summary = asanadw::llm::agents::period::summarize_project_period(db, &agent, &project_gid, &p, force).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("{}", summary.headline);
                println!("\n{}", summary.what_changed);
                println!("\n{}", summary.why_it_matters);
                println!("\nKey milestones:");
                for m in &summary.key_milestones {
                    println!("  - {m}");
                }
                if let Some(ref health) = summary.health_assessment {
                    println!("\nHealth: {health}");
                }
            }
        }
        SummarizeTarget::Portfolio { portfolio_gid, period, force, json } => {
            let p = asanadw::Period::parse(&period)?;
            let summary = asanadw::llm::agents::period::summarize_portfolio_period(db, &agent, &portfolio_gid, &p, force).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("{}", summary.headline);
                println!("\n{}", summary.what_changed);
                println!("\n{}", summary.why_it_matters);
                println!("\nKey milestones:");
                for m in &summary.key_milestones {
                    println!("  - {m}");
                }
                if let Some(ref health) = summary.health_assessment {
                    println!("\nHealth: {health}");
                }
            }
        }
        SummarizeTarget::Team { team_gid, period, force, json } => {
            let p = asanadw::Period::parse(&period)?;
            let summary = asanadw::llm::agents::period::summarize_team_period(db, &agent, &team_gid, &p, force).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("{}", summary.headline);
                println!("\n{}", summary.what_changed);
                println!("\n{}", summary.why_it_matters);
                println!("\nKey accomplishments:");
                for a in &summary.key_accomplishments {
                    println!("  - {a}");
                }
                if let Some(ref health) = summary.health_assessment {
                    println!("\nHealth: {health}");
                }
            }
        }
    }

    Ok(())
}

async fn handle_metrics(db: &asanadw::Database, target: MetricsTarget) -> anyhow::Result<()> {
    match target {
        MetricsTarget::Me { period, json } => {
            let user_gid = db.reader().call(|c| asanadw::storage::repository::get_config(c, "user_gid")).await?
                .ok_or_else(|| anyhow::anyhow!("User identity not set. Run 'asanadw sync all' first."))?;
            let p = asanadw::Period::parse(&period)?;
            let m = asanadw::metrics::compute_user_metrics(db, &user_gid, &p).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&m)?);
            } else {
                println!("User Metrics: {} ({})", m.user_name.as_deref().unwrap_or(&m.user_gid), m.period_key);
                print_throughput(&m.throughput);
                print_lead_time(&m.lead_time);
                print_collaboration(&m.collaboration);
            }
        }
        MetricsTarget::User { user_gid, period, json } => {
            let user_gid = resolve_user(db, &user_gid).await?;
            let p = asanadw::Period::parse(&period)?;
            let m = asanadw::metrics::compute_user_metrics(db, &user_gid, &p).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&m)?);
            } else {
                println!("User Metrics: {} ({})", m.user_name.as_deref().unwrap_or(&m.user_gid), m.period_key);
                print_throughput(&m.throughput);
                print_lead_time(&m.lead_time);
                print_collaboration(&m.collaboration);
            }
        }
        MetricsTarget::Project { project_gid, period, json } => {
            let p = asanadw::Period::parse(&period)?;
            let m = asanadw::metrics::compute_project_metrics(db, &project_gid, &p).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&m)?);
            } else {
                println!("Project Metrics: {} ({})", m.project_name.as_deref().unwrap_or(&m.project_gid), m.period_key);
                print_throughput(&m.throughput);
                print_health(&m.health);
                print_lead_time(&m.lead_time);
                print_collaboration(&m.collaboration);
            }
        }
        MetricsTarget::Portfolio { portfolio_gid, period, json } => {
            let p = asanadw::Period::parse(&period)?;
            let m = asanadw::metrics::compute_portfolio_metrics(db, &portfolio_gid, &p).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&m)?);
            } else {
                println!("Portfolio Metrics: {} ({})", m.portfolio_name.as_deref().unwrap_or(&m.portfolio_gid), m.period_key);
                println!("  Projects: {}", m.project_count);
                print_throughput(&m.throughput);
                print_health(&m.health);
                print_lead_time(&m.lead_time);
                print_collaboration(&m.collaboration);
            }
        }
        MetricsTarget::Team { team_gid, period, json } => {
            let p = asanadw::Period::parse(&period)?;
            let m = asanadw::metrics::compute_team_metrics(db, &team_gid, &p).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&m)?);
            } else {
                println!("Team Metrics: {} ({})", m.team_name.as_deref().unwrap_or(&m.team_gid), m.period_key);
                println!("  Members: {}", m.member_count);
                print_throughput(&m.throughput);
                print_health(&m.health);
                print_lead_time(&m.lead_time);
                print_collaboration(&m.collaboration);
            }
        }
    }
    Ok(())
}

fn print_throughput(t: &asanadw::metrics::ThroughputMetrics) {
    println!("  Throughput:");
    println!("    Created:   {}", t.tasks_created);
    println!("    Completed: {}", t.tasks_completed);
    println!("    Net new:   {}", t.net_new);
}

fn print_health(h: &asanadw::metrics::HealthMetrics) {
    println!("  Health:");
    println!("    Open tasks:  {}", h.total_open);
    println!("    Overdue:     {} ({:.1}%)", h.overdue_count, h.overdue_pct);
    println!("    Unassigned:  {} ({:.1}%)", h.unassigned_count, h.unassigned_pct);
    println!("    Stale (14d): {}", h.stale_count);
}

fn print_lead_time(lt: &asanadw::metrics::LeadTimeMetrics) {
    println!("  Lead Time:");
    match lt.avg_days_to_complete {
        Some(avg) => {
            println!("    Average: {avg:.1} days");
            println!("    Median:  {:.1} days", lt.median_days_to_complete.unwrap_or(0.0));
            println!("    P90:     {:.1} days", lt.p90_days_to_complete.unwrap_or(0.0));
            println!("    Range:   {}-{} days", lt.min_days_to_complete.unwrap_or(0), lt.max_days_to_complete.unwrap_or(0));
        }
        None => println!("    No completed tasks in period"),
    }
}

fn print_collaboration(c: &asanadw::metrics::CollaborationMetrics) {
    println!("  Collaboration:");
    println!("    Comments:    {}", c.total_comments);
    println!("    Commenters:  {}", c.unique_commenters);
    println!("    Likes:       {}", c.total_likes);
}

fn print_sync_report(report: &asanadw::SyncReport) {
    println!("Sync: {}", report.entity_key);
    println!("  Status:  {:?}", report.status);
    println!("  Synced:  {} items", report.items_synced);
    println!("  Failed:  {} items", report.items_failed);
    println!(
        "  Batches: {}/{}",
        report.batches_completed, report.batches_total
    );
    if let Some(ref err) = report.error {
        println!("  Error:   {err}");
    }
}
