# asanadw Technical Design

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      CLI (clap)                              │
├─────────────────────────────────────────────────────────────┤
│                    AsanaDW Library                           │
├──────────┬──────────┬──────────┬──────────┬─────────────────┤
│  Sync    │  Query   │  Search  │ Metrics  │      LLM        │
│  Engine  │  Builder │  (FTS5)  │  Engine  │    Agents       │
├──────────┴──────────┴──────────┴──────────┴─────────────────┤
│                   Storage Layer (SQLite)                     │
├─────────────────────────────────────────────────────────────┤
│                   asanaclient (Asana API)                    │
└─────────────────────────────────────────────────────────────┘
```

## Database Schema

### Connection Pragmas

Every connection must set the following pragmas at open time. The storage layer enforces this automatically.

```sql
PRAGMA journal_mode = WAL;      -- Write-ahead logging for concurrent reads
PRAGMA foreign_keys = ON;       -- Required for ON DELETE CASCADE
PRAGMA busy_timeout = 5000;     -- Wait up to 5s for locks instead of failing
```

### Dimension Tables

```sql
-- User directory
CREATE TABLE dim_users (
    user_gid TEXT PRIMARY KEY,
    email TEXT,
    name TEXT NOT NULL,
    photo_url TEXT,
    cached_at TEXT NOT NULL
);
CREATE INDEX idx_users_email ON dim_users(email);

-- Project metadata
CREATE TABLE dim_projects (
    id INTEGER PRIMARY KEY,         -- Stable rowid for FTS content sync
    project_gid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    owner_gid TEXT,
    team_gid TEXT,
    workspace_gid TEXT NOT NULL,
    is_archived INTEGER DEFAULT 0,
    is_template INTEGER DEFAULT 0,
    color TEXT,
    notes TEXT,
    notes_html TEXT,
    created_at TEXT,
    modified_at TEXT,
    cached_at TEXT NOT NULL,
    FOREIGN KEY (owner_gid) REFERENCES dim_users(user_gid),
    FOREIGN KEY (team_gid) REFERENCES dim_teams(team_gid)
);

CREATE INDEX idx_projects_gid ON dim_projects(project_gid);

-- Portfolio metadata
CREATE TABLE dim_portfolios (
    portfolio_gid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    owner_gid TEXT,
    workspace_gid TEXT NOT NULL,
    is_public INTEGER DEFAULT 1,
    color TEXT,
    cached_at TEXT NOT NULL,
    FOREIGN KEY (owner_gid) REFERENCES dim_users(user_gid)
);

-- Team metadata
CREATE TABLE dim_teams (
    team_gid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    workspace_gid TEXT NOT NULL,
    description TEXT,
    cached_at TEXT NOT NULL
);

-- Section metadata (within projects)
CREATE TABLE dim_sections (
    section_gid TEXT PRIMARY KEY,
    project_gid TEXT NOT NULL,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    cached_at TEXT NOT NULL,
    FOREIGN KEY (project_gid) REFERENCES dim_projects(project_gid) ON DELETE CASCADE
);

CREATE INDEX idx_sections_project ON dim_sections(project_gid);

-- Calendar dimension
-- Populated on init from the earliest record date through end of current quarter.
-- Extended automatically when new data arrives outside the current range.
CREATE TABLE dim_date (
    date_key TEXT PRIMARY KEY,      -- YYYY-MM-DD
    year INTEGER NOT NULL,
    quarter INTEGER NOT NULL,       -- 1-4
    month INTEGER NOT NULL,         -- 1-12
    week INTEGER NOT NULL,          -- ISO week number
    day_of_week INTEGER NOT NULL,   -- 1=Monday, 7=Sunday (ISO)
    day_of_month INTEGER NOT NULL,
    day_of_year INTEGER NOT NULL,
    is_weekend INTEGER NOT NULL,
    is_first_day_of_month INTEGER NOT NULL,
    is_last_day_of_month INTEGER NOT NULL,
    is_first_day_of_quarter INTEGER NOT NULL,
    is_last_day_of_quarter INTEGER NOT NULL,

    -- Period keys for joining to dim_period
    year_key TEXT NOT NULL,         -- "2025"
    half_key TEXT NOT NULL,         -- "2025-H1"
    quarter_key TEXT NOT NULL,      -- "2025-Q1"
    month_key TEXT NOT NULL,        -- "2025-01"
    week_key TEXT NOT NULL,         -- "2025-W05"

    -- Fiscal offsets (relative position within period, 0-based)
    day_of_quarter INTEGER NOT NULL,
    day_of_half INTEGER NOT NULL,

    -- Prior-period same-day keys (for period-over-period to-date comparisons)
    prior_year_date_key TEXT,       -- same month/day last year (NULL if Feb 29)
    prior_quarter_date_key TEXT,    -- same day-of-quarter offset in prior quarter
    prior_month_date_key TEXT       -- same day-of-month in prior month (clamped)
);

-- Period definitions
-- Pre-populated alongside dim_date for all year/half/quarter/month/week periods.
CREATE TABLE dim_period (
    period_key TEXT PRIMARY KEY,
    period_type TEXT NOT NULL,      -- year, half, quarter, month, week
    label TEXT NOT NULL,            -- Human-readable: "Q1 2025", "January 2025", etc.
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    days_in_period INTEGER NOT NULL,
    prior_period_key TEXT           -- Key of the equivalent prior period
);
CREATE INDEX idx_period_type ON dim_period(period_type, start_date);
```

### Fact Tables

```sql
-- Task records (primary fact table)
CREATE TABLE fact_tasks (
    id INTEGER PRIMARY KEY,         -- Stable rowid for FTS content sync
    task_gid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    notes TEXT,
    notes_html TEXT,
    assignee_gid TEXT,
    is_completed INTEGER DEFAULT 0,
    completed_at TEXT,
    completed_date_key TEXT,
    due_on TEXT,
    due_at TEXT,
    start_on TEXT,
    start_at TEXT,
    created_at TEXT NOT NULL,
    created_date_key TEXT NOT NULL,
    modified_at TEXT,
    parent_gid TEXT,                -- Parent task for subtasks
    is_subtask INTEGER DEFAULT 0,
    num_subtasks INTEGER DEFAULT 0,
    num_likes INTEGER DEFAULT 0,

    -- Computed metrics
    days_to_complete INTEGER,       -- NULL if not completed
    is_overdue INTEGER DEFAULT 0,

    -- Sync metadata
    cached_at TEXT NOT NULL,

    FOREIGN KEY (assignee_gid) REFERENCES dim_users(user_gid),
    FOREIGN KEY (parent_gid) REFERENCES fact_tasks(task_gid),
    FOREIGN KEY (created_date_key) REFERENCES dim_date(date_key),
    FOREIGN KEY (completed_date_key) REFERENCES dim_date(date_key)
);

CREATE INDEX idx_tasks_gid ON fact_tasks(task_gid);
CREATE INDEX idx_tasks_assignee ON fact_tasks(assignee_gid);
CREATE INDEX idx_tasks_completed ON fact_tasks(is_completed, completed_date_key);
CREATE INDEX idx_tasks_created ON fact_tasks(created_date_key);
CREATE INDEX idx_tasks_parent ON fact_tasks(parent_gid);
CREATE INDEX idx_tasks_due ON fact_tasks(due_on);
CREATE INDEX idx_tasks_modified ON fact_tasks(modified_at);

-- Custom field definitions (discovered during sync)
CREATE TABLE dim_custom_fields (
    field_gid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    field_type TEXT NOT NULL,        -- text, number, enum, multi_enum, date, people
    enum_options TEXT,               -- JSON array of {gid, name, color} for enum types
    cached_at TEXT NOT NULL
);

-- Custom field values (normalized, one row per task per field)
CREATE TABLE fact_task_custom_fields (
    task_gid TEXT NOT NULL,
    field_gid TEXT NOT NULL,
    text_value TEXT,                 -- For text, enum display name, people name
    number_value REAL,               -- For number fields
    date_value TEXT,                 -- For date fields (YYYY-MM-DD)
    enum_value_gid TEXT,             -- For enum fields (raw GID)
    display_value TEXT NOT NULL,     -- Human-readable representation (always populated)
    PRIMARY KEY (task_gid, field_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (field_gid) REFERENCES dim_custom_fields(field_gid)
);

CREATE INDEX idx_tcf_field ON fact_task_custom_fields(field_gid);
CREATE INDEX idx_tcf_text ON fact_task_custom_fields(field_gid, text_value);
CREATE INDEX idx_tcf_number ON fact_task_custom_fields(field_gid, number_value);

-- Comments and activity
CREATE TABLE fact_comments (
    id INTEGER PRIMARY KEY,         -- Stable rowid for FTS content sync
    comment_gid TEXT NOT NULL UNIQUE,
    task_gid TEXT NOT NULL,
    author_gid TEXT,
    text TEXT,
    html_text TEXT,
    story_type TEXT NOT NULL,       -- comment, system, etc.
    created_at TEXT NOT NULL,
    created_date_key TEXT NOT NULL,
    cached_at TEXT NOT NULL,
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (author_gid) REFERENCES dim_users(user_gid)
);

CREATE INDEX idx_comments_task ON fact_comments(task_gid);
CREATE INDEX idx_comments_author ON fact_comments(author_gid);

-- Status updates for projects and portfolios
CREATE TABLE fact_status_updates (
    status_gid TEXT PRIMARY KEY,
    parent_gid TEXT NOT NULL,       -- Project or portfolio GID
    parent_type TEXT NOT NULL,      -- 'project' or 'portfolio'
    author_gid TEXT,
    title TEXT NOT NULL,
    text TEXT,
    html_text TEXT,
    status_type TEXT NOT NULL,      -- on_track, at_risk, off_track, on_hold
    created_at TEXT NOT NULL,
    created_date_key TEXT NOT NULL,
    cached_at TEXT NOT NULL,
    FOREIGN KEY (author_gid) REFERENCES dim_users(user_gid)
);

CREATE INDEX idx_status_parent ON fact_status_updates(parent_gid, parent_type);
```

### Bridge Tables

```sql
-- Task to project membership
CREATE TABLE bridge_task_projects (
    task_gid TEXT NOT NULL,
    project_gid TEXT NOT NULL,
    section_gid TEXT,
    PRIMARY KEY (task_gid, project_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (project_gid) REFERENCES dim_projects(project_gid) ON DELETE CASCADE,
    FOREIGN KEY (section_gid) REFERENCES dim_sections(section_gid)
);

CREATE INDEX idx_btp_project ON bridge_task_projects(project_gid);

-- Portfolio to project membership
CREATE TABLE bridge_portfolio_projects (
    portfolio_gid TEXT NOT NULL,
    project_gid TEXT NOT NULL,
    PRIMARY KEY (portfolio_gid, project_gid),
    FOREIGN KEY (portfolio_gid) REFERENCES dim_portfolios(portfolio_gid) ON DELETE CASCADE,
    FOREIGN KEY (project_gid) REFERENCES dim_projects(project_gid) ON DELETE CASCADE
);

-- Task tags
CREATE TABLE bridge_task_tags (
    task_gid TEXT NOT NULL,
    tag_gid TEXT NOT NULL,
    tag_name TEXT NOT NULL,
    PRIMARY KEY (task_gid, tag_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE
);

-- Task dependencies
CREATE TABLE bridge_task_dependencies (
    task_gid TEXT NOT NULL,
    depends_on_gid TEXT NOT NULL,
    PRIMARY KEY (task_gid, depends_on_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (depends_on_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE
);

-- Task followers (for collaboration metrics)
CREATE TABLE bridge_task_followers (
    task_gid TEXT NOT NULL,
    user_gid TEXT NOT NULL,
    PRIMARY KEY (task_gid, user_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (user_gid) REFERENCES dim_users(user_gid)
);

-- Team membership
CREATE TABLE bridge_team_members (
    team_gid TEXT NOT NULL,
    user_gid TEXT NOT NULL,
    role TEXT,                       -- "member", "admin", etc. (from Asana API)
    PRIMARY KEY (team_gid, user_gid),
    FOREIGN KEY (team_gid) REFERENCES dim_teams(team_gid) ON DELETE CASCADE,
    FOREIGN KEY (user_gid) REFERENCES dim_users(user_gid)
);

CREATE INDEX idx_team_members_user ON bridge_team_members(user_gid);
```

### Full-Text Search

```sql
-- FTS5 virtual tables for search
-- These use content-sync with the explicit integer `id` column as rowid,
-- which is stable across VACUUM (unlike implicit rowid on TEXT PRIMARY KEY tables).

CREATE VIRTUAL TABLE tasks_fts USING fts5(
    task_gid,
    name,
    notes,
    content='fact_tasks',
    content_rowid='id',
    tokenize='porter unicode61'
);

CREATE VIRTUAL TABLE comments_fts USING fts5(
    comment_gid,
    task_gid,
    text,
    content='fact_comments',
    content_rowid='id',
    tokenize='porter unicode61'
);

CREATE VIRTUAL TABLE projects_fts USING fts5(
    project_gid,
    name,
    notes,
    content='dim_projects',
    content_rowid='id',
    tokenize='porter unicode61'
);

-- Custom field values are also searchable via FTS
CREATE VIRTUAL TABLE custom_fields_fts USING fts5(
    task_gid,
    field_name,
    display_value,
    tokenize='porter unicode61'
);

-- Triggers: fact_tasks <-> tasks_fts
CREATE TRIGGER tasks_ai AFTER INSERT ON fact_tasks BEGIN
    INSERT INTO tasks_fts(rowid, task_gid, name, notes)
    VALUES (NEW.id, NEW.task_gid, NEW.name, NEW.notes);
END;

CREATE TRIGGER tasks_ad AFTER DELETE ON fact_tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, task_gid, name, notes)
    VALUES ('delete', OLD.id, OLD.task_gid, OLD.name, OLD.notes);
END;

CREATE TRIGGER tasks_au AFTER UPDATE ON fact_tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, task_gid, name, notes)
    VALUES ('delete', OLD.id, OLD.task_gid, OLD.name, OLD.notes);
    INSERT INTO tasks_fts(rowid, task_gid, name, notes)
    VALUES (NEW.id, NEW.task_gid, NEW.name, NEW.notes);
END;

-- Triggers: fact_comments <-> comments_fts
CREATE TRIGGER comments_ai AFTER INSERT ON fact_comments BEGIN
    INSERT INTO comments_fts(rowid, comment_gid, task_gid, text)
    VALUES (NEW.id, NEW.comment_gid, NEW.task_gid, NEW.text);
END;

CREATE TRIGGER comments_ad AFTER DELETE ON fact_comments BEGIN
    INSERT INTO comments_fts(comments_fts, rowid, comment_gid, task_gid, text)
    VALUES ('delete', OLD.id, OLD.comment_gid, OLD.task_gid, OLD.text);
END;

CREATE TRIGGER comments_au AFTER UPDATE ON fact_comments BEGIN
    INSERT INTO comments_fts(comments_fts, rowid, comment_gid, task_gid, text)
    VALUES ('delete', OLD.id, OLD.comment_gid, OLD.task_gid, OLD.text);
    INSERT INTO comments_fts(rowid, comment_gid, task_gid, text)
    VALUES (NEW.id, NEW.comment_gid, NEW.task_gid, NEW.text);
END;

-- Triggers: dim_projects <-> projects_fts
CREATE TRIGGER projects_ai AFTER INSERT ON dim_projects BEGIN
    INSERT INTO projects_fts(rowid, project_gid, name, notes)
    VALUES (NEW.id, NEW.project_gid, NEW.name, NEW.notes);
END;

CREATE TRIGGER projects_ad AFTER DELETE ON dim_projects BEGIN
    INSERT INTO projects_fts(projects_fts, rowid, project_gid, name, notes)
    VALUES ('delete', OLD.id, OLD.project_gid, OLD.name, OLD.notes);
END;

CREATE TRIGGER projects_au AFTER UPDATE ON dim_projects BEGIN
    INSERT INTO projects_fts(projects_fts, rowid, project_gid, name, notes)
    VALUES ('delete', OLD.id, OLD.project_gid, OLD.name, OLD.notes);
    INSERT INTO projects_fts(rowid, project_gid, name, notes)
    VALUES (NEW.id, NEW.project_gid, NEW.name, NEW.notes);
END;

-- Triggers: fact_task_custom_fields <-> custom_fields_fts
CREATE TRIGGER tcf_ai AFTER INSERT ON fact_task_custom_fields BEGIN
    INSERT INTO custom_fields_fts(task_gid, field_name, display_value)
    VALUES (NEW.task_gid,
            (SELECT name FROM dim_custom_fields WHERE field_gid = NEW.field_gid),
            NEW.display_value);
END;

CREATE TRIGGER tcf_ad AFTER DELETE ON fact_task_custom_fields BEGIN
    DELETE FROM custom_fields_fts
    WHERE task_gid = OLD.task_gid
      AND field_name = (SELECT name FROM dim_custom_fields WHERE field_gid = OLD.field_gid)
      AND display_value = OLD.display_value;
END;

CREATE TRIGGER tcf_au AFTER UPDATE ON fact_task_custom_fields BEGIN
    DELETE FROM custom_fields_fts
    WHERE task_gid = OLD.task_gid
      AND field_name = (SELECT name FROM dim_custom_fields WHERE field_gid = OLD.field_gid)
      AND display_value = OLD.display_value;
    INSERT INTO custom_fields_fts(task_gid, field_name, display_value)
    VALUES (NEW.task_gid,
            (SELECT name FROM dim_custom_fields WHERE field_gid = NEW.field_gid),
            NEW.display_value);
END;
```

### Operational Tables

```sql
-- Monitored entities
CREATE TABLE monitored_entities (
    entity_key TEXT PRIMARY KEY,    -- "user:alice@example.com", "portfolio:123"
    entity_type TEXT NOT NULL,
    entity_gid TEXT NOT NULL,
    display_name TEXT,
    added_at TEXT NOT NULL,
    last_sync_at TEXT,
    sync_enabled INTEGER DEFAULT 1
);

-- Synced date ranges per entity (for gap detection)
CREATE TABLE synced_ranges (
    id INTEGER PRIMARY KEY,
    entity_key TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    synced_at TEXT NOT NULL,
    UNIQUE(entity_key, start_date, end_date)
);

CREATE INDEX idx_synced_entity ON synced_ranges(entity_key);

-- Sync job log (append-only history of all sync attempts)
CREATE TABLE sync_jobs (
    id INTEGER PRIMARY KEY,
    entity_key TEXT NOT NULL,
    status TEXT NOT NULL,           -- running, completed, failed, partial
    started_at TEXT NOT NULL,
    completed_at TEXT,
    total_items INTEGER DEFAULT 0,
    synced_items INTEGER DEFAULT 0,
    skipped_items INTEGER DEFAULT 0,
    failed_items INTEGER DEFAULT 0,
    batches_total INTEGER DEFAULT 0,
    batches_completed INTEGER DEFAULT 0,
    error_message TEXT,
    sync_range_start TEXT,          -- Start of requested date range
    sync_range_end TEXT             -- End of requested date range
);

CREATE INDEX idx_sync_jobs_entity ON sync_jobs(entity_key, started_at);
CREATE INDEX idx_sync_jobs_status ON sync_jobs(status);

-- Configuration
CREATE TABLE app_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### LLM Summary Tables

```sql
-- Task-level summaries
CREATE TABLE fact_task_summaries (
    task_gid TEXT PRIMARY KEY,
    headline TEXT NOT NULL,
    what_happened TEXT NOT NULL,
    why_it_matters TEXT NOT NULL,
    complexity_signal TEXT NOT NULL,
    notability_score INTEGER NOT NULL,
    change_types TEXT NOT NULL,     -- JSON array
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE
);

-- Period summaries for users
CREATE TABLE fact_user_period_summaries (
    user_gid TEXT NOT NULL,
    period_key TEXT NOT NULL,
    headline TEXT NOT NULL,
    what_changed TEXT NOT NULL,
    why_it_matters TEXT NOT NULL,
    key_accomplishments TEXT NOT NULL,  -- JSON array
    collaboration_notes TEXT,
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (user_gid, period_key),
    FOREIGN KEY (user_gid) REFERENCES dim_users(user_gid)
);

-- Period summaries for projects
CREATE TABLE fact_project_period_summaries (
    project_gid TEXT NOT NULL,
    period_key TEXT NOT NULL,
    headline TEXT NOT NULL,
    what_changed TEXT NOT NULL,
    why_it_matters TEXT NOT NULL,
    key_milestones TEXT NOT NULL,   -- JSON array
    health_assessment TEXT,
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (project_gid, period_key),
    FOREIGN KEY (project_gid) REFERENCES dim_projects(project_gid)
);

-- Period summaries for portfolios
CREATE TABLE fact_portfolio_period_summaries (
    portfolio_gid TEXT NOT NULL,
    period_key TEXT NOT NULL,
    headline TEXT NOT NULL,
    what_changed TEXT NOT NULL,
    why_it_matters TEXT NOT NULL,
    key_milestones TEXT NOT NULL,   -- JSON array
    health_assessment TEXT,
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (portfolio_gid, period_key),
    FOREIGN KEY (portfolio_gid) REFERENCES dim_portfolios(portfolio_gid)
);

-- Period summaries for teams
CREATE TABLE fact_team_period_summaries (
    team_gid TEXT NOT NULL,
    period_key TEXT NOT NULL,
    headline TEXT NOT NULL,
    what_changed TEXT NOT NULL,
    why_it_matters TEXT NOT NULL,
    key_accomplishments TEXT NOT NULL,  -- JSON array
    health_assessment TEXT,
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (team_gid, period_key),
    FOREIGN KEY (team_gid) REFERENCES dim_teams(team_gid)
);
```

## Library API

### Primary Entry Point

```rust
pub struct AsanaDW {
    db: Database,
    client: asanaclient::Client,
    llm: mixtape_core::Agent,
}

/// LLM provider selection. Determined by the `llm_provider` config key.
/// Defaults to Bedrock if not configured.
/// - Bedrock: uses standard AWS credential chain
/// - Anthropic: reads ANTHROPIC_API_KEY from environment
#[derive(Default)]
pub enum LlmProvider {
    #[default]
    Bedrock,
    Anthropic,
}

/// Database wraps a connection pool with WAL mode for concurrent reads.
/// Writers acquire an exclusive lock; readers proceed without blocking.
pub struct Database {
    writer: Mutex<Connection>,   // Single writer connection
    pool: Vec<Connection>,       // Reader pool (sized to available cores)
}

impl Database {
    pub fn writer(&self) -> MutexGuard<'_, Connection>;
    pub fn reader(&self) -> &Connection;
}

impl AsanaDW {
    // Initialization
    pub async fn open() -> Result<Self>;
    pub async fn open_at(path: impl AsRef<Path>) -> Result<Self>;

    // Status
    pub fn status(&self) -> Result<WarehouseStatus>;

    // Monitoring
    pub fn monitor(&self, entity_type: EntityType, identifier: &str) -> Result<()>;
    pub fn unmonitor(&self, entity_type: EntityType, identifier: &str) -> Result<()>;
    pub fn monitored_entities(&self) -> Result<Vec<MonitoredEntity>>;

    // Sync operations
    pub async fn sync_user(&self, email_or_gid: &str, opts: &SyncOptions) -> Result<SyncReport>;
    pub async fn sync_team(&self, identifier: &str, opts: &SyncOptions) -> Result<SyncReport>;
    pub async fn sync_portfolio(&self, gid_or_url: &str, opts: &SyncOptions) -> Result<SyncReport>;
    pub async fn sync_project(&self, gid_or_url: &str, opts: &SyncOptions) -> Result<SyncReport>;
    pub async fn sync_all(&self, opts: &SyncOptions) -> Result<Vec<SyncReport>>;

    // Querying
    pub fn query(&self) -> QueryBuilder;
    pub fn search(&self, query: &str, opts: &SearchOptions) -> Result<SearchResults>;

    // Metrics
    pub fn user_metrics(&self, user_gid: &str, period: &Period) -> Result<UserMetrics>;
    pub fn project_metrics(&self, project_gid: &str, period: &Period) -> Result<ProjectMetrics>;
    pub fn portfolio_metrics(&self, portfolio_gid: &str, period: &Period) -> Result<PortfolioMetrics>;
    pub fn team_metrics(&self, team_gid: &str, period: &Period) -> Result<TeamMetrics>;

    // Summarization
    pub async fn summarize_task(&self, task_gid: &str, force: bool) -> Result<TaskSummary>;
    pub async fn summarize_user_period(&self, user_gid: &str, period: &Period, force: bool) -> Result<UserPeriodSummary>;
    pub async fn summarize_project_period(&self, project_gid: &str, period: &Period, force: bool) -> Result<ProjectPeriodSummary>;
    pub async fn summarize_portfolio_period(&self, portfolio_gid: &str, period: &Period, force: bool) -> Result<PortfolioPeriodSummary>;
    pub async fn summarize_team_period(&self, team_gid: &str, period: &Period, force: bool) -> Result<TeamPeriodSummary>;
}

/// Parse an Asana URL into its component identifiers.
/// Standalone function — does not require an AsanaDW instance.
pub fn parse_asana_url(url: &str) -> Result<AsanaUrlInfo>;
```

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Asana API error: {0}")]
    Api(#[from] asanaclient::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("Sync error for {entity_key}: {message}")]
    Sync { entity_key: String, message: String },

    #[error("Invalid URL: {0}")]
    UrlParse(String),

    #[error("Invalid identifier: {0}")]
    InvalidIdentifier(String),

    #[error("Invalid period format: {0}")]
    PeriodParse(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

### Key Types

```rust
pub struct SyncOptions {
    /// How many days back to sync (modification-based). Mutually exclusive with `since`.
    pub days: Option<u32>,
    /// Absolute start date. Mutually exclusive with `days`.
    pub since: Option<NaiveDate>,
    /// Days per batch (default: 30).
    pub batch_size_days: u32,
    /// Progress callback, invoked after each batch completes.
    pub on_progress: Option<Box<dyn Fn(&SyncProgress)>>,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self { days: Some(90), since: None, batch_size_days: 30, on_progress: None }
    }
}

pub struct SyncProgress {
    pub entity_key: String,
    pub batch_index: u32,                   // 0-based
    pub batch_total: u32,
    pub batch_start: NaiveDate,
    pub batch_end: NaiveDate,
    pub items_synced_so_far: u64,
}

pub struct SyncReport {
    pub entity_key: String,
    pub status: SyncStatus,
    pub total_items: u64,
    pub synced_items: u64,
    pub skipped_items: u64,
    pub failed_items: u64,
    pub batches_total: u32,
    pub batches_completed: u32,
    pub failed_ranges: Vec<(NaiveDate, NaiveDate)>,
    pub duration: Duration,
}

pub enum SyncStatus {
    Completed,
    Partial { failed_batches: u32 },
    Failed { error: String },
}

pub struct WarehouseStatus {
    pub db_path: PathBuf,
    pub db_size_bytes: u64,
    pub total_tasks: u64,
    pub total_projects: u64,
    pub total_portfolios: u64,
    pub total_users: u64,
    pub earliest_data: Option<NaiveDate>,
    pub latest_data: Option<NaiveDate>,
    pub monitored_entities: Vec<MonitoredEntityStatus>,
    pub pending_gaps: u64,              // Count of date ranges needing sync
}

pub struct MonitoredEntityStatus {
    pub entity_key: String,
    pub display_name: Option<String>,
    pub last_sync_at: Option<String>,
    pub last_sync_status: Option<SyncStatus>,
}
```

### Search Types

```rust
pub struct SearchOptions {
    pub entity_types: Option<Vec<SearchEntityType>>,  // Filter to specific types
    pub assignee: Option<String>,                     // Filter tasks by assignee
    pub limit: u32,                                   // Max results (default: 50)
}

pub enum SearchEntityType {
    Tasks,
    Projects,
    Comments,
}

pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub total_count: u64,               // Total matches (may exceed hits.len())
    pub query: String,
}

pub struct SearchHit {
    pub entity_type: SearchEntityType,
    pub gid: String,                    // Task, project, or comment GID
    pub title: String,                  // Task name, project name, or "Comment on <task>"
    pub snippet: String,                // Matching text with highlights (marked with **)
    pub project_name: Option<String>,   // Parent project (for tasks and comments)
    pub assignee_name: Option<String>,  // Assignee (for tasks)
    pub asana_url: String,              // Reconstructed Asana URL
    pub relevance: f64,                 // FTS5 rank score
}
```

### Metrics Types

```rust
pub struct UserMetrics {
    pub user_gid: String,
    pub user_name: String,
    pub period: Period,
    pub throughput: ThroughputMetrics,
    pub lead_time: LeadTimeMetrics,
    pub collaboration: CollaborationMetrics,
}

pub struct ProjectMetrics {
    pub project_gid: String,
    pub project_name: String,
    pub period: Period,
    pub throughput: ThroughputMetrics,
    pub health: HealthMetrics,
    pub lead_time: LeadTimeMetrics,
}

pub struct PortfolioMetrics {
    pub portfolio_gid: String,
    pub portfolio_name: String,
    pub period: Period,
    pub throughput: ThroughputMetrics,
    pub health: HealthMetrics,
    pub project_breakdown: Vec<ProjectMetrics>,  // Per-project within portfolio
}

pub struct TeamMetrics {
    pub team_gid: String,
    pub team_name: String,
    pub period: Period,
    pub throughput: ThroughputMetrics,
    pub health: HealthMetrics,
    pub member_breakdown: Vec<UserMetrics>,  // Per-member within team
}

pub struct ThroughputMetrics {
    pub tasks_completed: u64,
    pub tasks_created: u64,
    pub completion_rate: f64,           // completed / (completed + still open in period)
    pub subtasks_completed: u64,
}

pub struct HealthMetrics {
    pub overdue_tasks: u64,
    pub status_updates_count: u64,
    pub latest_status: Option<String>,  // on_track, at_risk, off_track, on_hold
    pub blocker_count: u64,             // Tasks with unresolved dependencies
}

pub struct LeadTimeMetrics {
    pub median_days_to_complete: Option<f64>,
    pub p90_days_to_complete: Option<f64>,
    pub avg_days_to_complete: Option<f64>,
}

pub struct CollaborationMetrics {
    pub unique_collaborators: u64,      // Distinct users on tasks (followers, commenters)
    pub comments_authored: u64,
    pub tasks_with_followers: u64,
}
```

### QueryBuilder

```rust
pub struct QueryBuilder<'a> { /* ... */ }

impl<'a> QueryBuilder<'a> {
    // Filters
    pub fn assignee(self, email_or_gid: &str) -> Self;
    pub fn project(self, gid_or_name: &str) -> Self;
    pub fn portfolio(self, gid_or_name: &str) -> Self;
    pub fn team(self, gid_or_name: &str) -> Self;
    pub fn completed(self) -> Self;
    pub fn incomplete(self) -> Self;
    pub fn overdue(self) -> Self;
    pub fn period(self, period: Period) -> Self;
    pub fn since(self, date: NaiveDate) -> Self;
    pub fn until(self, date: NaiveDate) -> Self;
    pub fn custom_field(self, field_name: &str, value: &str) -> Self;

    // Output
    pub fn tasks(self) -> Result<Vec<Task>>;
    pub fn projects(self) -> Result<Vec<Project>>;
    pub fn count(self) -> Result<u64>;
    pub fn to_csv(self) -> Result<String>;
    pub fn to_json(self) -> Result<String>;
}
```

### Period Type

```rust
pub enum Period {
    Year(i32),                  // "2025"
    Half(i32, u8),              // "2025-H1"
    Quarter(i32, u8),           // "2025-Q1"
    Month(i32, u8),             // "2025-01"
    Week(i32, u8),              // "2025-W05" (ISO week)
    Rolling(u32, NaiveDate),    // "30d" from date

    // To-date variants (anchored to today)
    YearToDate(i32),            // "ytd" or "2025-ytd"
    HalfToDate(i32, u8),        // "htd"
    QuarterToDate(i32, u8),     // "qtd"
    MonthToDate(i32, u8),       // "mtd"
    WeekToDate(i32, u8),        // "wtd"
}

impl Period {
    pub fn parse(s: &str) -> Result<Self>;
    pub fn to_key(&self) -> String;
    pub fn date_range(&self) -> (NaiveDate, NaiveDate);
    pub fn previous(&self) -> Self;

    /// For period-over-period comparisons: returns the equivalent to-date
    /// range in the prior period. E.g., if this is Q1 2026 and today is
    /// Feb 7, returns Q1 2025 clamped to the same relative day offset.
    pub fn prior_period_to_date(&self, as_of: NaiveDate) -> Self;

    /// Returns true if this period is the "current" period (contains today).
    pub fn is_current(&self) -> bool;
}
```

## Sync Architecture

### Sync Flow

```
1. Parse identifier (GID, email, URL)
        ↓
2. Resolve to canonical GID via API
        ↓
3. Create sync_jobs record (status=running)
        ↓
4. Determine sync scope:
   - User: fetch tasks assigned to user
   - Team: fetch team members + team's projects, then their tasks
   - Portfolio: fetch portfolio items, then their tasks
   - Project: fetch project sections, then project tasks
        ↓
5. Compute date range (from --days or --since)
        ↓
6. Split into monthly batches, check synced_ranges for gaps
   Only process batches that have gaps in coverage
        ↓
7. For each batch (month-sized window):
   a. BEGIN TRANSACTION
   b. Fetch from API (modified_since filter, with pagination)
   c. Delete-and-reinsert bridge rows for synced tasks
      (ON DELETE CASCADE handles child rows)
   d. Upsert tasks, comments, custom fields
   e. Extend dim_date if new dates encountered
   f. Update synced_ranges for this batch
   g. Update sync_jobs progress (batches_completed++)
   h. COMMIT
   i. Report progress via on_progress callback
   On error: ROLLBACK, record failure, continue to next batch
        ↓
8. Update sync_jobs record (status=completed|partial|failed)
        ↓
9. Return SyncReport
```

### Upsert Strategy

The sync engine uses `INSERT OR REPLACE` keyed on the `UNIQUE(task_gid)` constraint. When a task already exists, SQLite deletes the old row (cascading to bridge tables, comments, custom fields via `ON DELETE CASCADE`) and inserts a new row with a fresh `id`.

This means every re-sync of a task re-inserts its entire dependency graph, even if nothing changed. The FTS triggers handle this correctly (delete old FTS entry, insert new one). This is acceptable for v1 given the batch-transactional model. A future optimization could check `modified_at` and skip tasks that haven't changed since `cached_at`.

### Rate Limiting

The Asana API enforces per-minute rate limits and returns HTTP 429 responses. The sync engine handles this with:

1. **Exponential backoff** — On 429, wait for the duration specified by the `Retry-After` header (or 60s if absent), then retry.
2. **Up to 3 retries** per request before considering it a failure.
3. **Batch-level failure isolation** — A failed request within a batch causes that batch to roll back. Remaining batches proceed normally.
4. **Backpressure** — The sync engine tracks 429 frequency and voluntarily throttles request rate when approaching limits.

### Deletion Handling

When a task is re-synced, its bridge table rows (project memberships, tags, dependencies, followers) and child rows (comments, custom field values) are cleaned up via `ON DELETE CASCADE`. The sync upserts tasks using `INSERT OR REPLACE`, which triggers a DELETE+INSERT cycle that cascades correctly.

For entities that have been deleted from Asana entirely (no longer returned by the API), a periodic `gc` pass can be added in a future version. For v1, stale records remain in the database but don't affect correctness since queries always filter by synced scope.

### Gap Detection

```rust
pub struct SyncedRange {
    pub entity_key: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

/// Find date ranges within [desired_start, desired_end] that are not
/// covered by any existing synced range. Merges overlapping/adjacent
/// ranges before computing gaps. Returns gaps aligned to month boundaries
/// for batch processing.
pub fn find_gaps(
    ranges: &[SyncedRange],
    desired_start: NaiveDate,
    desired_end: NaiveDate,
) -> Vec<(NaiveDate, NaiveDate)>;
```

### URL Parsing

```rust
pub enum AsanaUrlInfo {
    Task { task_gid: String, project_gid: Option<String> },
    Project { project_gid: String, workspace_gid: Option<String> },
    Portfolio { portfolio_gid: String },
    Team { team_gid: String, workspace_gid: String },
}

// URL patterns:
// https://app.asana.com/0/portfolio/{portfolio_gid}/list
// https://app.asana.com/0/{workspace_gid}/{project_gid}
// https://app.asana.com/0/{project_gid}/{task_gid}
// https://app.asana.com/0/{workspace_gid}/{team_gid}/...
```

## Schema Migrations

Schema versioning is managed by `rusqlite_migration`. Each migration is a numbered SQL file in `src/storage/migrations/`. On `AsanaDW::open()`, pending migrations run automatically. The migration framework tracks which migrations have been applied and runs only new ones.

## Project Structure

```
asanadw/
├── Cargo.toml
├── docs/
│   ├── prd.md
│   └── design.md
├── src/
│   ├── lib.rs              # Public API, re-exports
│   ├── error.rs            # Error enum
│   ├── bin/
│   │   └── asanadw.rs      # CLI entry point
│   ├── storage/
│   │   ├── mod.rs           # Database struct, connection management, pragmas
│   │   ├── schema.rs        # dim_date/dim_period population logic
│   │   ├── repository.rs    # CRUD operations
│   │   └── migrations/
│   │       └── 001_initial.sql
│   ├── sync/
│   │   ├── mod.rs
│   │   ├── syncer.rs        # Orchestration, batch processing
│   │   ├── gap.rs           # Gap detection
│   │   └── rate_limit.rs    # Backoff and throttle logic
│   ├── query/
│   │   ├── mod.rs
│   │   ├── builder.rs       # QueryBuilder
│   │   └── period.rs        # Period enum
│   ├── search/
│   │   └── mod.rs           # FTS engine, SearchOptions, SearchResults
│   ├── metrics/
│   │   ├── mod.rs
│   │   ├── types.rs         # Metric structs
│   │   ├── user.rs
│   │   ├── project.rs
│   │   ├── portfolio.rs
│   │   └── team.rs
│   ├── url/
│   │   └── mod.rs           # URL parsing (standalone)
│   └── llm/
│       ├── mod.rs
│       ├── types.rs
│       └── agents/
│           ├── task.rs
│           └── period.rs
└── tests/
    └── integration/
```

## Dependencies

```toml
[dependencies]
asanaclient = { path = "../asanaclient" }
mixtape-core = { version = "0.2", features = ["anthropic", "bedrock"] }
rusqlite = { version = "0.38", features = ["bundled"] }
rusqlite_migration = "2.4"
tokio = { version = "1.49", features = ["full"] }
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2.0"
log = "0.4"
env_logger = "0.11"
dirs = "6.0"
url = "2.5"
regex = "1.12"
```
