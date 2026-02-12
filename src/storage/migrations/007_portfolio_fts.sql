-- Add FTS5 for portfolios so they appear in search results.

CREATE VIRTUAL TABLE portfolios_fts USING fts5(
    portfolio_gid,
    name,
    content='dim_portfolios',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Triggers: dim_portfolios <-> portfolios_fts

CREATE TRIGGER portfolios_ai AFTER INSERT ON dim_portfolios BEGIN
    INSERT INTO portfolios_fts(rowid, portfolio_gid, name)
    VALUES (NEW.rowid, NEW.portfolio_gid, COALESCE(NEW.name, ''));
END;

CREATE TRIGGER portfolios_ad AFTER DELETE ON dim_portfolios BEGIN
    INSERT INTO portfolios_fts(portfolios_fts, rowid, portfolio_gid, name)
    VALUES ('delete', OLD.rowid, OLD.portfolio_gid, COALESCE(OLD.name, ''));
END;

CREATE TRIGGER portfolios_au AFTER UPDATE ON dim_portfolios BEGIN
    INSERT INTO portfolios_fts(portfolios_fts, rowid, portfolio_gid, name)
    VALUES ('delete', OLD.rowid, OLD.portfolio_gid, COALESCE(OLD.name, ''));
    INSERT INTO portfolios_fts(rowid, portfolio_gid, name)
    VALUES (NEW.rowid, NEW.portfolio_gid, COALESCE(NEW.name, ''));
END;

-- Backfill existing portfolios into FTS
INSERT INTO portfolios_fts(rowid, portfolio_gid, name)
SELECT rowid, portfolio_gid, COALESCE(name, '')
FROM dim_portfolios;
