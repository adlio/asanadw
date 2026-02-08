-- Dimension tables

CREATE TABLE dim_users (
    user_gid TEXT PRIMARY KEY,
    email TEXT,
    name TEXT NOT NULL,
    photo_url TEXT,
    cached_at TEXT NOT NULL
);
CREATE INDEX idx_users_email ON dim_users(email);

CREATE TABLE dim_teams (
    team_gid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    workspace_gid TEXT NOT NULL,
    description TEXT,
    cached_at TEXT NOT NULL
);

CREATE TABLE dim_projects (
    id INTEGER PRIMARY KEY,
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

CREATE TABLE dim_sections (
    section_gid TEXT PRIMARY KEY,
    project_gid TEXT NOT NULL,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    cached_at TEXT NOT NULL,
    FOREIGN KEY (project_gid) REFERENCES dim_projects(project_gid) ON DELETE CASCADE
);
CREATE INDEX idx_sections_project ON dim_sections(project_gid);

CREATE TABLE dim_date (
    date_key TEXT PRIMARY KEY,
    year INTEGER NOT NULL,
    quarter INTEGER NOT NULL,
    month INTEGER NOT NULL,
    week INTEGER NOT NULL,
    day_of_week INTEGER NOT NULL,
    day_of_month INTEGER NOT NULL,
    day_of_year INTEGER NOT NULL,
    is_weekend INTEGER NOT NULL,
    is_first_day_of_month INTEGER NOT NULL,
    is_last_day_of_month INTEGER NOT NULL,
    is_first_day_of_quarter INTEGER NOT NULL,
    is_last_day_of_quarter INTEGER NOT NULL,
    year_key TEXT NOT NULL,
    half_key TEXT NOT NULL,
    quarter_key TEXT NOT NULL,
    month_key TEXT NOT NULL,
    week_key TEXT NOT NULL,
    day_of_quarter INTEGER NOT NULL,
    day_of_half INTEGER NOT NULL,
    prior_year_date_key TEXT,
    prior_quarter_date_key TEXT,
    prior_month_date_key TEXT
);

CREATE TABLE dim_period (
    period_key TEXT PRIMARY KEY,
    period_type TEXT NOT NULL,
    label TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    days_in_period INTEGER NOT NULL,
    prior_period_key TEXT
);
CREATE INDEX idx_period_type ON dim_period(period_type, start_date);

-- Fact tables

CREATE TABLE fact_tasks (
    id INTEGER PRIMARY KEY,
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
    parent_gid TEXT,
    is_subtask INTEGER DEFAULT 0,
    num_subtasks INTEGER DEFAULT 0,
    num_likes INTEGER DEFAULT 0,
    days_to_complete INTEGER,
    is_overdue INTEGER DEFAULT 0,
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

CREATE TABLE dim_custom_fields (
    field_gid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    field_type TEXT NOT NULL,
    enum_options TEXT,
    cached_at TEXT NOT NULL
);

CREATE TABLE fact_task_custom_fields (
    task_gid TEXT NOT NULL,
    field_gid TEXT NOT NULL,
    text_value TEXT,
    number_value REAL,
    date_value TEXT,
    enum_value_gid TEXT,
    display_value TEXT NOT NULL,
    PRIMARY KEY (task_gid, field_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (field_gid) REFERENCES dim_custom_fields(field_gid)
);
CREATE INDEX idx_tcf_field ON fact_task_custom_fields(field_gid);
CREATE INDEX idx_tcf_text ON fact_task_custom_fields(field_gid, text_value);
CREATE INDEX idx_tcf_number ON fact_task_custom_fields(field_gid, number_value);

CREATE TABLE fact_comments (
    id INTEGER PRIMARY KEY,
    comment_gid TEXT NOT NULL UNIQUE,
    task_gid TEXT NOT NULL,
    author_gid TEXT,
    text TEXT,
    html_text TEXT,
    story_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    created_date_key TEXT NOT NULL,
    cached_at TEXT NOT NULL,
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (author_gid) REFERENCES dim_users(user_gid)
);
CREATE INDEX idx_comments_task ON fact_comments(task_gid);
CREATE INDEX idx_comments_author ON fact_comments(author_gid);

CREATE TABLE fact_status_updates (
    status_gid TEXT PRIMARY KEY,
    parent_gid TEXT NOT NULL,
    parent_type TEXT NOT NULL,
    author_gid TEXT,
    title TEXT NOT NULL,
    text TEXT,
    html_text TEXT,
    status_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    created_date_key TEXT NOT NULL,
    cached_at TEXT NOT NULL,
    FOREIGN KEY (author_gid) REFERENCES dim_users(user_gid)
);
CREATE INDEX idx_status_parent ON fact_status_updates(parent_gid, parent_type);

-- Bridge tables

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

CREATE TABLE bridge_portfolio_projects (
    portfolio_gid TEXT NOT NULL,
    project_gid TEXT NOT NULL,
    PRIMARY KEY (portfolio_gid, project_gid),
    FOREIGN KEY (portfolio_gid) REFERENCES dim_portfolios(portfolio_gid) ON DELETE CASCADE,
    FOREIGN KEY (project_gid) REFERENCES dim_projects(project_gid) ON DELETE CASCADE
);

CREATE TABLE bridge_task_tags (
    task_gid TEXT NOT NULL,
    tag_gid TEXT NOT NULL,
    tag_name TEXT NOT NULL,
    PRIMARY KEY (task_gid, tag_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE
);

CREATE TABLE bridge_task_dependencies (
    task_gid TEXT NOT NULL,
    depends_on_gid TEXT NOT NULL,
    PRIMARY KEY (task_gid, depends_on_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (depends_on_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE
);

CREATE TABLE bridge_task_followers (
    task_gid TEXT NOT NULL,
    user_gid TEXT NOT NULL,
    PRIMARY KEY (task_gid, user_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (user_gid) REFERENCES dim_users(user_gid)
);

CREATE TABLE bridge_team_members (
    team_gid TEXT NOT NULL,
    user_gid TEXT NOT NULL,
    role TEXT,
    PRIMARY KEY (team_gid, user_gid),
    FOREIGN KEY (team_gid) REFERENCES dim_teams(team_gid) ON DELETE CASCADE,
    FOREIGN KEY (user_gid) REFERENCES dim_users(user_gid)
);
CREATE INDEX idx_team_members_user ON bridge_team_members(user_gid);

-- Full-text search

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

CREATE VIRTUAL TABLE custom_fields_fts USING fts5(
    task_gid,
    field_name,
    display_value,
    tokenize='porter unicode61'
);

-- FTS triggers: fact_tasks <-> tasks_fts
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

-- FTS triggers: fact_comments <-> comments_fts
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

-- FTS triggers: dim_projects <-> projects_fts
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

-- FTS triggers: fact_task_custom_fields <-> custom_fields_fts
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

-- Operational tables

CREATE TABLE monitored_entities (
    entity_key TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_gid TEXT NOT NULL,
    display_name TEXT,
    added_at TEXT NOT NULL,
    last_sync_at TEXT,
    sync_enabled INTEGER DEFAULT 1
);

CREATE TABLE synced_ranges (
    id INTEGER PRIMARY KEY,
    entity_key TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    synced_at TEXT NOT NULL,
    UNIQUE(entity_key, start_date, end_date)
);
CREATE INDEX idx_synced_entity ON synced_ranges(entity_key);

CREATE TABLE sync_jobs (
    id INTEGER PRIMARY KEY,
    entity_key TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    total_items INTEGER DEFAULT 0,
    synced_items INTEGER DEFAULT 0,
    skipped_items INTEGER DEFAULT 0,
    failed_items INTEGER DEFAULT 0,
    batches_total INTEGER DEFAULT 0,
    batches_completed INTEGER DEFAULT 0,
    error_message TEXT,
    sync_range_start TEXT,
    sync_range_end TEXT
);
CREATE INDEX idx_sync_jobs_entity ON sync_jobs(entity_key, started_at);
CREATE INDEX idx_sync_jobs_status ON sync_jobs(status);

CREATE TABLE app_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- LLM summary tables

CREATE TABLE fact_task_summaries (
    task_gid TEXT PRIMARY KEY,
    headline TEXT NOT NULL,
    what_happened TEXT NOT NULL,
    why_it_matters TEXT NOT NULL,
    complexity_signal TEXT NOT NULL,
    notability_score INTEGER NOT NULL,
    change_types TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE
);

CREATE TABLE fact_user_period_summaries (
    user_gid TEXT NOT NULL,
    period_key TEXT NOT NULL,
    headline TEXT NOT NULL,
    what_changed TEXT NOT NULL,
    why_it_matters TEXT NOT NULL,
    key_accomplishments TEXT NOT NULL,
    collaboration_notes TEXT,
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (user_gid, period_key),
    FOREIGN KEY (user_gid) REFERENCES dim_users(user_gid)
);

CREATE TABLE fact_project_period_summaries (
    project_gid TEXT NOT NULL,
    period_key TEXT NOT NULL,
    headline TEXT NOT NULL,
    what_changed TEXT NOT NULL,
    why_it_matters TEXT NOT NULL,
    key_milestones TEXT NOT NULL,
    health_assessment TEXT,
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (project_gid, period_key),
    FOREIGN KEY (project_gid) REFERENCES dim_projects(project_gid)
);

CREATE TABLE fact_portfolio_period_summaries (
    portfolio_gid TEXT NOT NULL,
    period_key TEXT NOT NULL,
    headline TEXT NOT NULL,
    what_changed TEXT NOT NULL,
    why_it_matters TEXT NOT NULL,
    key_milestones TEXT NOT NULL,
    health_assessment TEXT,
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (portfolio_gid, period_key),
    FOREIGN KEY (portfolio_gid) REFERENCES dim_portfolios(portfolio_gid)
);

CREATE TABLE fact_team_period_summaries (
    team_gid TEXT NOT NULL,
    period_key TEXT NOT NULL,
    headline TEXT NOT NULL,
    what_changed TEXT NOT NULL,
    why_it_matters TEXT NOT NULL,
    key_accomplishments TEXT NOT NULL,
    health_assessment TEXT,
    prompt_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (team_gid, period_key),
    FOREIGN KEY (team_gid) REFERENCES dim_teams(team_gid)
);
