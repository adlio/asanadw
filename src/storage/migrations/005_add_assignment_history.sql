CREATE TABLE fact_assignment_events (
    id INTEGER PRIMARY KEY,
    task_gid TEXT NOT NULL,
    old_assignee_gid TEXT,
    new_assignee_gid TEXT,
    changed_at TEXT NOT NULL,
    FOREIGN KEY (task_gid) REFERENCES fact_tasks(task_gid) ON DELETE CASCADE,
    FOREIGN KEY (old_assignee_gid) REFERENCES dim_users(user_gid),
    FOREIGN KEY (new_assignee_gid) REFERENCES dim_users(user_gid)
);
CREATE INDEX idx_assign_events_task ON fact_assignment_events(task_gid);
CREATE INDEX idx_assign_events_old ON fact_assignment_events(old_assignee_gid);
CREATE INDEX idx_assign_events_new ON fact_assignment_events(new_assignee_gid);

-- Track assignee changes automatically via trigger
CREATE TRIGGER track_assignee_change AFTER UPDATE ON fact_tasks
    WHEN COALESCE(OLD.assignee_gid, '') != COALESCE(NEW.assignee_gid, '')
BEGIN
    INSERT INTO fact_assignment_events (task_gid, old_assignee_gid, new_assignee_gid, changed_at)
    VALUES (NEW.task_gid, OLD.assignee_gid, NEW.assignee_gid, datetime('now'));
END;

-- Drop the dead enum_options TEXT column from dim_custom_fields
-- (replaced by dim_enum_options table in migration 003)
ALTER TABLE dim_custom_fields DROP COLUMN enum_options;
