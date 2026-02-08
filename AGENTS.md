# Agent Instructions for asanadw

## Overview

asanadw is a CLI tool that syncs Asana data into a local SQLite data warehouse. It supports offline queries, full-text
search, metrics computation, and LLM-powered summaries.

```
CLI (clap) → lib.rs (orchestration) → asanaclient (HTTP) → Asana API
                                     → storage (SQLite)
                                     → mixtape-core (LLM)
```

## Directory Structure

```
src/
├── bin/
│   └── asanadw.rs          # CLI entry point (clap command definitions)
├── lib.rs                  # Library root: workspace detection, user resolution, entity orchestration
├── error.rs                # Error types (thiserror)
├── date_util.rs            # Date/period parsing utilities
├── url/
│   └── mod.rs              # Asana URL parsing (extract GIDs from URLs)
├── storage/
│   ├── mod.rs              # Database initialization, WAL mode, migrations
│   ├── schema.rs           # Schema creation and FTS5 virtual tables
│   ├── repository.rs       # All SQL read/write operations
│   └── migrations/         # SQL migration files
├── sync/
│   ├── mod.rs              # Sync types (SyncOptions, SyncReport, SyncProgress)
│   ├── syncer.rs           # Core sync logic: incremental (Events API) and full sync
│   ├── api_helpers.rs      # Asana API response transformation
│   ├── gap.rs              # Date gap detection for sync ranges
│   └── rate_limit.rs       # API rate limit handling
├── search/
│   └── mod.rs              # FTS5 full-text search across tasks, comments, projects, custom fields
├── query/
│   ├── mod.rs              # Query module exports
│   ├── builder.rs          # SQL query builder with dynamic filters
│   └── period.rs           # Period parsing (qtd, ytd, rolling-30d, 2024-Q1, etc.)
├── metrics/
│   ├── mod.rs              # Metrics computation (tasks created/completed, by assignee, etc.)
│   └── types.rs            # Metrics result types
└── llm/
    ├── mod.rs              # LLM provider setup (Bedrock, Anthropic via mixtape-core)
    └── agents/             # LLM agent prompts for summarization
```

## Database Schema

Star schema design:

- **dim_** tables: users, teams, projects, portfolios, sections, date, period, custom_fields, enum_options
- **fact_** tables: tasks, comments, status_updates, task_custom_fields, summaries (task/user/project/portfolio/team)
- **bridge_** tables: task_projects, portfolio_projects, task_tags, task_dependencies, task_followers, team_members, task_multi_enum_values

FTS5 virtual tables: `tasks_fts`, `comments_fts`, `projects_fts`, `custom_fields_fts`

## Key Concepts

| Concept | Description |
|---------|-------------|
| **Monitored entity** | A project/user/team/portfolio registered for sync via `monitor add` |
| **Incremental sync** | Uses Asana Events API; tokens expire after 24h; >50 changes triggers full sync |
| **Sync range** | Tracks which date ranges have been synced per entity to avoid redundant fetches |
| **Period** | Time range for metrics/summaries: `qtd`, `ytd`, `rolling-30d`, `2024-Q1`, `2024-M03` |

## Commands

```bash
make ci              # Run before committing (fmt, clippy, build, docs, test)
make test            # Run tests only
make coverage        # Coverage report
make coverage-html   # HTML coverage report
```

## Code Style

- Run `make ci` before committing
- All clippy warnings are errors
- Use `thiserror` for library error types
- `anyhow` is used in the binary for top-level error handling
- Follow existing patterns in the code
