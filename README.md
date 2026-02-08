# asanadw

Sync Asana data to a local SQLite data warehouse for offline queries, metrics, and LLM-powered summaries.

## Quick start

```sh
cargo install --path .
export ASANA_TOKEN="your-asana-personal-access-token"
asanadw monitor add-favorites
asanadw sync all
```

## Monitoring

Before syncing, you tell asanadw which entities to track. Monitored entities are synced when you run `sync all`.

```sh
# Add all your favorited projects and portfolios
asanadw monitor add-favorites

# Add individual entities
asanadw monitor add project 1234567890
asanadw monitor add user user@example.com
asanadw monitor add team 1234567890
asanadw monitor add portfolio 1234567890

# Asana URLs work too
asanadw monitor add project https://app.asana.com/0/1234567890/list

# List and remove
asanadw monitor list
asanadw monitor remove project:1234567890
```

## Syncing

Sync pulls data from the Asana API into the local database.

```sh
# Sync all monitored entities
asanadw sync all

# Sync a single entity
asanadw sync project 1234567890
asanadw sync user user@example.com
asanadw sync team 1234567890
asanadw sync portfolio 1234567890
```

What each entity type syncs:

- **project** -- tasks, comments, custom fields, sections
- **user** -- tasks assigned to the user
- **team** -- team members and team projects
- **portfolio** -- contained projects (and their tasks)

### Filtering by date

```sh
asanadw sync all --days 90          # last 90 days
asanadw sync all --since 2024-01-01 # since a specific date
asanadw sync all --full             # force full sync (ignore incremental tokens)
```

## Incremental sync

After the first full sync of a project, subsequent syncs use the Asana Events API to fetch only what changed. This is significantly faster for large projects.

### How it works

1. After a full sync, asanadw stores an events sync token for each project.
2. On the next sync, it asks the Events API "what changed since this token?"
3. Only the changed tasks are fetched individually.
4. If more than 50 tasks changed, asanadw falls back to a full bulk fetch (faster than 50+ individual GETs).

### Token expiry

Asana event sync tokens expire after 24 hours. When a token expires:

- The next sync does a one-time full sync for that project
- A fresh token is stored automatically
- Subsequent syncs resume the fast incremental path

### Forcing a full sync

```sh
asanadw sync all --full
asanadw sync project 1234567890 --full
```

### Scheduling syncs

To stay on the fast incremental path, run `sync all` at least once every 24 hours. Running every 15-30 minutes is recommended for near-real-time data.

**cron** (every 15 minutes):

```
*/15 * * * * ASANA_TOKEN="your-token" /path/to/asanadw sync all >> /tmp/asanadw-sync.log 2>&1
```

**launchd** (macOS, every 15 minutes):

Save as `~/Library/LaunchAgents/com.asanadw.sync.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.asanadw.sync</string>
    <key>ProgramArguments</key>
    <array>
        <string>/path/to/asanadw</string>
        <string>sync</string>
        <string>all</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>ASANA_TOKEN</key>
        <string>your-token</string>
    </dict>
    <key>StartInterval</key>
    <integer>900</integer>
    <key>StandardOutPath</key>
    <string>/tmp/asanadw-sync.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/asanadw-sync.log</string>
</dict>
</plist>
```

Load with:

```sh
launchctl load ~/Library/LaunchAgents/com.asanadw.sync.plist
```

## Querying

Query synced tasks with filters.

```sh
asanadw query --mine --incomplete
asanadw query --project 1234567890 --overdue
asanadw query --assignee user@example.com --completed
asanadw query --team 1234567890 --due-before 2025-03-01
asanadw query --portfolio 1234567890 --created-after 2025-01-01
```

### Filters

| Flag | Description |
|------|-------------|
| `--project <GID>` | Filter by project |
| `--portfolio <GID>` | Filter by portfolio |
| `--team <GID>` | Filter by team |
| `--assignee <GID or email>` | Filter by assignee |
| `--mine` | Tasks assigned to you |
| `--completed` | Completed tasks only |
| `--incomplete` | Incomplete tasks only |
| `--overdue` | Overdue tasks only |
| `--created-after <YYYY-MM-DD>` | Created after date |
| `--created-before <YYYY-MM-DD>` | Created before date |
| `--due-after <YYYY-MM-DD>` | Due after date |
| `--due-before <YYYY-MM-DD>` | Due before date |
| `--limit <N>` | Max results (default: 100) |

### Output formats

```sh
asanadw query --mine                # default table
asanadw query --mine --json         # JSON
asanadw query --mine --csv          # CSV
asanadw query --mine --count        # count only
```

## Search

Full-text search across tasks, comments, projects, and custom fields.

```sh
asanadw search "launch plan"
asanadw search "bug" --type task --mine
asanadw search "feedback" --project 1234567890
asanadw search "design review" --type comment --json
```

| Flag | Description |
|------|-------------|
| `--type <TYPE>` | Filter by type: task, comment, project, custom_field |
| `--assignee <GID or email>` | Filter by assignee |
| `--mine` | Tasks assigned to you |
| `--project <GID>` | Filter by project |
| `--limit <N>` | Max results (default: 20) |
| `--json` | JSON output |

## Metrics

Compute task metrics for a user, project, portfolio, or team over a time period.

```sh
asanadw metrics me
asanadw metrics me --period rolling-30d
asanadw metrics user user@example.com --period 2024-Q1
asanadw metrics project 1234567890 --period ytd
asanadw metrics portfolio 1234567890 --period 2024-M06
asanadw metrics team 1234567890 --period qtd --json
```

### Period formats

| Period | Description |
|--------|-------------|
| `qtd` | Quarter to date (default) |
| `ytd` | Year to date |
| `rolling-30d` | Rolling 30 days |
| `2024-Q1` | Specific quarter |
| `2024-M03` | Specific month |

## Summaries

Generate LLM-powered narrative summaries. Requires an LLM provider to be configured (see [Configuration](#configuration)).

```sh
asanadw summarize me
asanadw summarize me --period rolling-30d
asanadw summarize task 1234567890
asanadw summarize user user@example.com --period 2024-Q1
asanadw summarize project 1234567890 --period ytd
asanadw summarize portfolio 1234567890 --json
asanadw summarize team 1234567890 --force   # bypass summary cache
```

| Flag | Description |
|------|-------------|
| `--period <PERIOD>` | Time period (same formats as metrics, default: qtd) |
| `--force` | Bypass cached summary and regenerate |
| `--json` | JSON output |

## Configuration

```sh
asanadw config list
asanadw config get llm_provider
asanadw config set llm_provider bedrock
asanadw config set llm_model claude-sonnet-4-5
```

| Key | Description |
|-----|-------------|
| `workspace_gid` | Asana workspace GID (auto-detected on first sync) |
| `llm_provider` | `bedrock` (default) or `anthropic` |
| `llm_model` | Model name (e.g. `claude-sonnet-4-5`, `claude-haiku-4-5`) |

## Database

Data is stored in a SQLite database at `~/.asanadw/asanadw.db`. Override with `--db`:

```sh
asanadw --db /path/to/custom.db sync all
```

The database uses WAL mode and can be queried directly with any SQLite client:

```sh
sqlite3 ~/.asanadw/asanadw.db "SELECT name FROM fact_tasks WHERE is_completed = 0 LIMIT 10"
```

### Schema

The database follows a star schema:

- **dim_** tables (dimensions): `dim_users`, `dim_teams`, `dim_projects`, `dim_portfolios`, `dim_sections`, `dim_date`, `dim_period`, `dim_custom_fields`, `dim_enum_options`
- **fact_** tables (facts): `fact_tasks`, `fact_comments`, `fact_status_updates`, `fact_task_custom_fields`, `fact_task_summaries`, `fact_user_period_summaries`, `fact_project_period_summaries`, `fact_portfolio_period_summaries`, `fact_team_period_summaries`
- **bridge_** tables (many-to-many): `bridge_task_projects`, `bridge_portfolio_projects`, `bridge_task_tags`, `bridge_task_dependencies`, `bridge_task_followers`, `bridge_team_members`, `bridge_task_multi_enum_values`

Full-text search is powered by FTS5 virtual tables (`tasks_fts`, `comments_fts`, `projects_fts`, `custom_fields_fts`).

## Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `ASANA_TOKEN` | Yes | Asana personal access token |
| `ANTHROPIC_API_KEY` | For `summarize` with `anthropic` provider | Anthropic API key |
| `AWS_*` | For `summarize` with `bedrock` provider (default) | Standard AWS credentials (e.g. `AWS_PROFILE`, `AWS_REGION`) |
