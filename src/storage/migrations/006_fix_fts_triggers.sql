-- Fix FTS triggers: wrap nullable columns with COALESCE to prevent
-- NULL vs empty-string mismatches from causing silent FTS delete failures.

-- ── fact_tasks <-> tasks_fts ──────────────────────────────────────

DROP TRIGGER IF EXISTS tasks_ai;
CREATE TRIGGER tasks_ai AFTER INSERT ON fact_tasks BEGIN
    INSERT INTO tasks_fts(rowid, task_gid, name, notes)
    VALUES (NEW.id, NEW.task_gid, COALESCE(NEW.name, ''), COALESCE(NEW.notes, ''));
END;

DROP TRIGGER IF EXISTS tasks_ad;
CREATE TRIGGER tasks_ad AFTER DELETE ON fact_tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, task_gid, name, notes)
    VALUES ('delete', OLD.id, OLD.task_gid, COALESCE(OLD.name, ''), COALESCE(OLD.notes, ''));
END;

DROP TRIGGER IF EXISTS tasks_au;
CREATE TRIGGER tasks_au AFTER UPDATE ON fact_tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, task_gid, name, notes)
    VALUES ('delete', OLD.id, OLD.task_gid, COALESCE(OLD.name, ''), COALESCE(OLD.notes, ''));
    INSERT INTO tasks_fts(rowid, task_gid, name, notes)
    VALUES (NEW.id, NEW.task_gid, COALESCE(NEW.name, ''), COALESCE(NEW.notes, ''));
END;

-- ── fact_comments <-> comments_fts ────────────────────────────────

DROP TRIGGER IF EXISTS comments_ai;
CREATE TRIGGER comments_ai AFTER INSERT ON fact_comments BEGIN
    INSERT INTO comments_fts(rowid, comment_gid, task_gid, text)
    VALUES (NEW.id, NEW.comment_gid, NEW.task_gid, COALESCE(NEW.text, ''));
END;

DROP TRIGGER IF EXISTS comments_ad;
CREATE TRIGGER comments_ad AFTER DELETE ON fact_comments BEGIN
    INSERT INTO comments_fts(comments_fts, rowid, comment_gid, task_gid, text)
    VALUES ('delete', OLD.id, OLD.comment_gid, OLD.task_gid, COALESCE(OLD.text, ''));
END;

DROP TRIGGER IF EXISTS comments_au;
CREATE TRIGGER comments_au AFTER UPDATE ON fact_comments BEGIN
    INSERT INTO comments_fts(comments_fts, rowid, comment_gid, task_gid, text)
    VALUES ('delete', OLD.id, OLD.comment_gid, OLD.task_gid, COALESCE(OLD.text, ''));
    INSERT INTO comments_fts(rowid, comment_gid, task_gid, text)
    VALUES (NEW.id, NEW.comment_gid, NEW.task_gid, COALESCE(NEW.text, ''));
END;

-- ── dim_projects <-> projects_fts ─────────────────────────────────

DROP TRIGGER IF EXISTS projects_ai;
CREATE TRIGGER projects_ai AFTER INSERT ON dim_projects BEGIN
    INSERT INTO projects_fts(rowid, project_gid, name, notes)
    VALUES (NEW.id, NEW.project_gid, COALESCE(NEW.name, ''), COALESCE(NEW.notes, ''));
END;

DROP TRIGGER IF EXISTS projects_ad;
CREATE TRIGGER projects_ad AFTER DELETE ON dim_projects BEGIN
    INSERT INTO projects_fts(projects_fts, rowid, project_gid, name, notes)
    VALUES ('delete', OLD.id, OLD.project_gid, COALESCE(OLD.name, ''), COALESCE(OLD.notes, ''));
END;

DROP TRIGGER IF EXISTS projects_au;
CREATE TRIGGER projects_au AFTER UPDATE ON dim_projects BEGIN
    INSERT INTO projects_fts(projects_fts, rowid, project_gid, name, notes)
    VALUES ('delete', OLD.id, OLD.project_gid, COALESCE(OLD.name, ''), COALESCE(OLD.notes, ''));
    INSERT INTO projects_fts(rowid, project_gid, name, notes)
    VALUES (NEW.id, NEW.project_gid, COALESCE(NEW.name, ''), COALESCE(NEW.notes, ''));
END;
