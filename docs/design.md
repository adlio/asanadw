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
    project_gid TEXT PRIMARY KEY,
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

-- Calendar dimension (pre-populated)
CREATE TABLE dim_date (
    date_key TEXT PRIMARY KEY,  -- YYYY-MM-DD
    year INTEGER NOT NULL,
    quarter INTEGER NOT NULL,
    month INTEGER NOT NULL,
    week INTEGER NOT NULL,      -- ISO week
    day_of_week INTEGER NOT NULL,
    is_weekend INTEGER NOT NULL,
    year_key TEXT NOT NULL,     -- "2025"
    half_key TEXT NOT NULL,     -- "2025-H1"
    quarter_key TEXT NOT NULL,  -- "2025-Q1"
    month_key TEXT NOT NULL,    -- "2025-01"
    week_key TEXT NOT NULL      -- "2025-W05"
);

-- Period definitions
CREATE TABLE dim_period (
    period_key TEXT PRIMARY KEY,
    period_type TEXT NOT NULL,  -- year, half, quarter, month, week
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL
);
```

### Fact Tables

```sql
-- Task records (primary fact table)
CREATE TABLE fact_tasks (
    task_gid TEXT PRIMARY KEY,
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
    parent_gid TEXT,            -- Parent task for subtasks
    is_subtask INTEGER DEFAULT 0,
    num_subtasks INTEGER DEFAULT 0,
    num_likes INTEGER DEFAULT 0,

    -- Computed metrics
    days_to_complete INTEGER,   -- NULL if not completed
    is_overdue INTEGER DEFAULT 0,

    -- Custom fields (stored as JSON)
    custom_fields TEXT,         -- JSON object

    -- Sync metadata
    cached_at TEXT NOT NULL,

    FOREIGN KEY (assignee_gid) REFERENCES dim_users(user_gid),
    FOREIGN KEY (parent_gid) REFERENCES fact_tasks(task_gid),
    FOREIGN KEY (created_date_key) REFERENCES dim_date(date_key),
    FOREIGN KEY (completed_date_key) REFERENCES dim_date(date_key)
);

CREATE INDEX idx_tasks_assignee ON fact_tasks(assignee_gid);
CREATE INDEX idx_tasks_completed ON fact_tasks(is_completed, completed_date_key);
CREATE INDEX idx_tasks_created ON fact_tasks(created_date_key);
CREATE INDEX idx_tasks_parent ON fact_tasks(parent_gid);
CREATE INDEX idx_tasks_due ON fact_tasks(due_on);

-- Comments and activity
CREATE TABLE fact_comments (
    comment_gid TEXT PRIMARY KEY,
    task_gid TEXT NOT NULL,
    author_gid TEXT,
    text TEXT,
    html_text TEXT,
    story_type TEXT NOT NULL,   -- comment, system, etc.
    created_at TEXT NOT NULL,
    created_date_key TEXT NOT NULL,
    cached_at TEXT NOT NULL,
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid),
    FOREIGN KEY (author_gid) REFERENCES dim_users(user_gid)
);

CREATE INDEX idx_comments_task ON fact_comments(task_gid);
CREATE INDEX idx_comments_author ON fact_comments(author_gid);

-- Status updates for projects and portfolios
CREATE TABLE fact_status_updates (
    status_gid TEXT PRIMARY KEY,
    parent_gid TEXT NOT NULL,   -- Project or portfolio GID
    parent_type TEXT NOT NULL,  -- 'project' or 'portfolio'
    author_gid TEXT,
    title TEXT NOT NULL,
    text TEXT,
    html_text TEXT,
    status_type TEXT NOT NULL,  -- on_track, at_risk, off_track, on_hold
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
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid),
    FOREIGN KEY (project_gid) REFERENCES dim_projects(project_gid)
);

CREATE INDEX idx_btp_project ON bridge_task_projects(project_gid);

-- Portfolio to project membership
CREATE TABLE bridge_portfolio_projects (
    portfolio_gid TEXT NOT NULL,
    project_gid TEXT NOT NULL,
    PRIMARY KEY (portfolio_gid, project_gid),
    FOREIGN KEY (portfolio_gid) REFERENCES dim_portfolios(portfolio_gid),
    FOREIGN KEY (project_gid) REFERENCES dim_projects(project_gid)
);

-- Task tags
CREATE TABLE bridge_task_tags (
    task_gid TEXT NOT NULL,
    tag_gid TEXT NOT NULL,
    tag_name TEXT NOT NULL,
    PRIMARY KEY (task_gid, tag_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid)
);

-- Task dependencies
CREATE TABLE bridge_task_dependencies (
    task_gid TEXT NOT NULL,
    depends_on_gid TEXT NOT NULL,
    PRIMARY KEY (task_gid, depends_on_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid),
    FOREIGN KEY (depends_on_gid) REFERENCES fact_tasks(task_gid)
);
```

### Full-Text Search

```sql
-- FTS5 virtual table for search
CREATE VIRTUAL TABLE tasks_fts USING fts5(
    task_gid,
    name,
    notes,
    content='fact_tasks',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE VIRTUAL TABLE comments_fts USING fts5(
    comment_gid,
    task_gid,
    text,
    content='fact_comments',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE VIRTUAL TABLE projects_fts USING fts5(
    project_gid,
    name,
    notes,
    content='dim_projects',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER tasks_ai AFTER INSERT ON fact_tasks BEGIN
    INSERT INTO tasks_fts(rowid, task_gid, name, notes)
    VALUES (NEW.rowid, NEW.task_gid, NEW.name, NEW.notes);
END;

CREATE TRIGGER tasks_ad AFTER DELETE ON fact_tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, task_gid, name, notes)
    VALUES ('delete', OLD.rowid, OLD.task_gid, OLD.name, OLD.notes);
END;

CREATE TRIGGER tasks_au AFTER UPDATE ON fact_tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, task_gid, name, notes)
    VALUES ('delete', OLD.rowid, OLD.task_gid, OLD.name, OLD.notes);
    INSERT INTO tasks_fts(rowid, task_gid, name, notes)
    VALUES (NEW.rowid, NEW.task_gid, NEW.name, NEW.notes);
END;
```

### Operational Tables

```sql
-- Monitored entities
CREATE TABLE monitored_entities (
    entity_key TEXT PRIMARY KEY,  -- "user:alice@example.com", "portfolio:123"
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

-- Sync job tracking
CREATE TABLE sync_jobs (
    entity_key TEXT PRIMARY KEY,
    status TEXT NOT NULL,         -- running, completed, failed
    started_at TEXT NOT NULL,
    completed_at TEXT,
    total_items INTEGER DEFAULT 0,
    synced_items INTEGER DEFAULT 0,
    skipped_items INTEGER DEFAULT 0,
    failed_items INTEGER DEFAULT 0,
    error_message TEXT
);

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
    change_types TEXT NOT NULL,   -- JSON array
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid)
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
    key_milestones TEXT NOT NULL,  -- JSON array
    health_assessment TEXT,
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (project_gid, period_key),
    FOREIGN KEY (project_gid) REFERENCES dim_projects(project_gid)
);
```

## Library API

### Primary Entry Point

```rust
pub struct AsanaDW {
    conn: Arc<Mutex<Connection>>,
    client: asanaclient::Client,
}

impl AsanaDW {
    // Initialization
    pub async fn open() -> Result<Self>;
    pub async fn open_at(path: impl AsRef<Path>) -> Result<Self>;
    pub fn connection(&self) -> Connection;

    // Monitoring
    pub fn monitor_entity(&self, entity_type: EntityType, identifier: &str) -> Result<()>;
    pub fn unmonitor_entity(&self, entity_key: &str) -> Result<()>;
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

    // URL parsing
    pub fn parse_asana_url(url: &str) -> Result<AsanaUrlInfo>;
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
    Year(i32),              // "2025"
    Half(i32, u8),          // "2025-H1"
    Quarter(i32, u8),       // "2025-Q1"
    Month(i32, u8),         // "2025-01"
    Week(i32, u8),          // "2025-W05" (ISO week)
    Rolling(u32, NaiveDate),// "30d" from date
}

impl Period {
    pub fn parse(s: &str) -> Result<Self>;
    pub fn to_key(&self) -> String;
    pub fn date_range(&self) -> (NaiveDate, NaiveDate);
    pub fn previous(&self) -> Self;
}
```

## Sync Architecture

### Sync Flow

```
1. Parse identifier (GID, email, URL)
        ↓
2. Resolve to canonical GID via API
        ↓
3. Acquire sync lock (prevent concurrent syncs)
        ↓
4. Determine sync scope:
   - User: fetch tasks assigned to user
   - Team: fetch team's projects, then their tasks
   - Portfolio: fetch portfolio items, then their tasks
   - Project: fetch project tasks directly
        ↓
5. For each entity in scope:
   a. Check synced_ranges for gaps
   b. Fetch from API (with pagination)
   c. Upsert to database
   d. Update synced_ranges
   e. Update sync lock progress
        ↓
6. Release sync lock
        ↓
7. Return SyncReport
```

### Gap Detection

```rust
pub struct SyncedRange {
    pub entity_key: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

// Find gaps in synced coverage
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

## Project Structure

```
asanadw/
├── Cargo.toml
├── docs/
│   ├── prd.md
│   └── design.md
├── src/
│   ├── lib.rs              # Public API
│   ├── error.rs            # Error types
│   ├── bin/
│   │   └── asanadw.rs      # CLI entry point
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── schema.rs       # Schema & migrations
│   │   ├── repository.rs   # CRUD operations
│   │   └── migrations/
│   │       └── 001_initial.sql
│   ├── sync/
│   │   ├── mod.rs
│   │   ├── syncer.rs       # Orchestration
│   │   └── gap.rs          # Gap detection
│   ├── query/
│   │   ├── mod.rs
│   │   ├── builder.rs      # QueryBuilder
│   │   └── period.rs       # Period enum
│   ├── search/
│   │   └── mod.rs          # FTS engine
│   ├── metrics/
│   │   ├── mod.rs
│   │   ├── types.rs
│   │   ├── user.rs
│   │   └── project.rs
│   ├── url/
│   │   └── mod.rs          # URL parsing
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
rusqlite = { version = "0.32", features = ["bundled"] }
rusqlite_migration = "1.2"
tokio = { version = "1.43", features = ["full"] }
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2.0"
log = "0.4"
env_logger = "0.11"
dirs = "5.0"
url = "2.5"
regex = "1.11"
```
