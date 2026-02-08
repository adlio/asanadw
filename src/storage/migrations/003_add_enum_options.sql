CREATE TABLE dim_enum_options (
    field_gid TEXT NOT NULL,
    option_gid TEXT NOT NULL,
    name TEXT,
    color TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    cached_at TEXT NOT NULL,
    PRIMARY KEY (field_gid, option_gid),
    FOREIGN KEY (field_gid) REFERENCES dim_custom_fields(field_gid) ON DELETE CASCADE
);
CREATE INDEX idx_enum_options_option ON dim_enum_options(option_gid);

CREATE TABLE bridge_task_multi_enum_values (
    task_gid TEXT NOT NULL,
    field_gid TEXT NOT NULL,
    option_gid TEXT NOT NULL,
    PRIMARY KEY (task_gid, field_gid, option_gid),
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (field_gid) REFERENCES dim_custom_fields(field_gid),
    FOREIGN KEY (option_gid) REFERENCES dim_enum_options(option_gid) ON DELETE CASCADE
);
CREATE INDEX idx_btmev_field ON bridge_task_multi_enum_values(field_gid);
CREATE INDEX idx_btmev_option ON bridge_task_multi_enum_values(option_gid);
