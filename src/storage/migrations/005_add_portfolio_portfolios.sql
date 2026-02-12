CREATE TABLE IF NOT EXISTS bridge_portfolio_portfolios (
    parent_portfolio_gid TEXT NOT NULL,
    child_portfolio_gid  TEXT NOT NULL,
    PRIMARY KEY (parent_portfolio_gid, child_portfolio_gid),
    FOREIGN KEY (parent_portfolio_gid) REFERENCES dim_portfolios(portfolio_gid) ON DELETE CASCADE,
    FOREIGN KEY (child_portfolio_gid)  REFERENCES dim_portfolios(portfolio_gid) ON DELETE CASCADE
);
