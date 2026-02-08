# asanadw Product Requirements Document

## Overview

asanadw is a Rust CLI and library that syncs Asana data to a local SQLite data warehouse, enabling offline queries, full-text search, metrics computation, and AI-powered summarization.

## Problem Statement

Asana's web interface and API are optimized for real-time collaboration, not historical analysis. Teams need:

- **Offline access** to task history without API rate limits
- **Fast search** across tasks, projects, and comments
- **Period-based metrics** (weekly/monthly/quarterly) for reporting
- **AI summaries** of activity for status updates and reviews

## Target Users

- Engineering managers tracking team velocity
- Program managers monitoring portfolio health
- Individual contributors reviewing their own productivity
- Executives needing roll-up summaries

## Core Features

### 1. Data Synchronization

Sync Asana data to local SQLite with full fidelity. Sync is **modification-timestamp based**: `--days N` means "fetch all entities modified within the last N days." Alternatively, `--since DATE` accepts an absolute start date (e.g., `--since 2024-01-01`). These are mutually exclusive; `--days` is the default.

| Entity Type | Identifier Formats | What Gets Synced |
|-------------|-------------------|------------------|
| User | Email or GID | All tasks assigned to the user |
| Team | Name, GID, or URL | All projects owned by the team, plus their tasks |
| Portfolio | GID or URL | All projects in portfolio, plus their tasks |
| Project | GID or URL | All tasks, subtasks, comments, status updates |

**Sync Depth:**
- Tasks with all standard and custom fields
- Subtasks (recursive, unlimited depth)
- Comments and activity stories
- Attachment metadata (not file contents)
- Project/portfolio status updates

**Sync Modes:**
- `sync <entity>` - One-time sync
- `monitor add` + `sync all` - Continuous monitoring

**Incremental Processing:** When syncing large date ranges, the sync engine splits the work into monthly batches and processes them sequentially. Progress is reported to the user as each batch completes (e.g., `Syncing month 3 of 12...`). Each batch is committed independently, so a failure mid-sync preserves all previously completed batches.

**Failure Behavior:** Each monthly batch is committed in its own database transaction. If a batch fails (network error, API rate limit, etc.), the transaction is rolled back for that batch only. The `synced_ranges` table accurately reflects which date ranges have been successfully synced. On the next `sync`, gap detection identifies the missing ranges and retries only those. A `SyncReport` is always returned summarizing successes, failures, and skipped items.

**Rate Limiting:** The Asana API enforces per-minute rate limits and returns HTTP 429 responses when exceeded. The sync engine handles this with exponential backoff and retry (up to 3 attempts per request). The `Retry-After` header from Asana is respected when present. If retries are exhausted, the current batch fails and the sync continues to the next batch.

### 2. Entity Monitoring

Track entities for automatic synchronization:

```
asanadw monitor add portfolio 1208241409266353
asanadw monitor add portfolio https://app.asana.com/0/portfolio/1208241409266353/list
asanadw monitor add user user@example.com
asanadw monitor add team "Backend Team"
asanadw monitor list
asanadw monitor remove user user@example.com
asanadw sync all --days 30
```

Monitor commands accept the same identifier formats as sync commands (see table above).

### 3. Search

Full-text search across all synced content:

```
asanadw search "Phase 2 Design Doc"
asanadw search "migration" --type tasks --assignee user@example.com
```

Search covers:
- Task names and descriptions
- Project names and descriptions
- Comment text
- Custom field values (text-based fields)

**Search Result Output:** Results are ranked by relevance and include:
- Entity type (task, project, or comment)
- Name/title with matching terms highlighted
- Context snippet (truncated description or comment text)
- Parent project name and assignee (for tasks)
- Link to the entity in Asana (reconstructed URL)

### 4. Querying

Flexible queries with filtering and output formats:

```
asanadw query --assignee user@example.com --period 2025-W05 --json
asanadw query --project "Backend Redesign" --completed --csv
asanadw query --portfolio "VX Team Portfolio" --incomplete
asanadw query --team "Backend Team" --overdue
```

**Filters:**

| Filter | Description |
|--------|-------------|
| `--assignee <EMAIL_OR_GID>` | Filter by task assignee |
| `--project <NAME_OR_GID_OR_URL>` | Filter by project |
| `--portfolio <NAME_OR_GID_OR_URL>` | Filter by portfolio |
| `--team <NAME_OR_GID>` | Filter by team |
| `--period <PERIOD>` | Filter by time period |
| `--completed` | Only completed tasks |
| `--incomplete` | Only incomplete tasks |
| `--overdue` | Only overdue tasks |

**Output Formats:**

| Flag | Description |
|------|-------------|
| `--json` | JSON output |
| `--csv` | CSV output |
| (default) | Human-readable table |

### 5. Metrics

Period-based metrics for productivity analysis:

```
asanadw metrics user user@example.com --period 2025-W05
asanadw metrics project 1208241409266353 --period 2026-Q1
asanadw metrics portfolio 1208241409266353 --period 2025-H1
```

**Metric Categories:**

| Category | Metrics |
|----------|---------|
| Throughput | Tasks completed, tasks created, completion rate |
| Health | Overdue tasks, status update frequency, blocker count |
| Lead Time | Days to complete, cycle time by task type |

**Period Formats:**
- `2025` - Full year
- `2025-H1` - Half year
- `2026-Q1` - Quarter
- `2025-01` - Month
- `2025-W05` - ISO week
- `30d` - Rolling last N days

**To-Date Periods:**
- `ytd` - Year to date
- `htd` - Half to date
- `qtd` - Quarter to date
- `mtd` - Month to date
- `wtd` - Week to date (ISO week)

**Period-over-Period Comparisons:** When comparing periods (e.g., week-over-week, quarter-over-quarter, year-over-year), the tool automatically uses to-date values for the current period to ensure fair comparison. For example, comparing Q1 2025 vs Q1 2026 when today is mid-Q1 2026 will compare full Q1 2025 against Q1-to-date 2026 using the equivalent date range within the quarter.

### 6. AI Summarization

LLM-powered summaries for tasks, users, projects, portfolios, and teams:

```
asanadw summarize task 1234567890
asanadw summarize user user@example.com --period 2025-W05
asanadw summarize project 1208241409266353 --period 2026-Q1
asanadw summarize portfolio 1208241409266353 --period 2025-H1
asanadw summarize team "Backend Team" --period 2025-W05
```

**Task Summary Output:**
- Headline (1-2 sentences)
- What happened (factual description)
- Why it matters (business context)
- Complexity signal (trivial/straightforward/involved/substantial)
- Notability score (0-10)

**Period Summary Output (user/project/portfolio/team):**
- Headline capturing the period's theme
- Key accomplishments (bullet list)
- Collaboration notes
- Health assessment (for projects/portfolios/teams)

## CLI Interface

### Global Options

```
asanadw [OPTIONS] <COMMAND>

Options:
  --db <PATH>       Database path (default: ~/.asanadw/asanadw.db)
  -v, --verbose     Increase logging verbosity
  --json            Output as JSON where applicable
  --csv             Output as CSV where applicable
  -h, --help        Print help
  -V, --version     Print version
```

### Commands

#### Sync Commands

```
asanadw sync user <EMAIL_OR_GID> [--days N | --since DATE]
asanadw sync team <NAME_OR_GID_OR_URL> [--days N | --since DATE]
asanadw sync portfolio <GID_OR_URL> [--days N | --since DATE]
asanadw sync project <GID_OR_URL> [--days N | --since DATE]
asanadw sync all [--days N | --since DATE]
```

`--days N` syncs all entities modified within the last N days (default: 90). `--since DATE` accepts an absolute start date (e.g., `2024-01-01`). These options are mutually exclusive.

#### Monitor Commands

```
asanadw monitor add user <EMAIL_OR_GID>
asanadw monitor add team <NAME_OR_GID_OR_URL>
asanadw monitor add portfolio <GID_OR_URL>
asanadw monitor add project <GID_OR_URL>
asanadw monitor remove <ENTITY_TYPE> <IDENTIFIER>
asanadw monitor list
```

Monitor commands accept the same identifier formats as their corresponding sync commands.

#### Query Commands

```
asanadw query [FILTERS...] [--json|--csv]
asanadw search <QUERY> [--type tasks|projects|comments] [--assignee EMAIL_OR_GID]
```

See [Querying](#4-querying) for the full list of filters.

#### Metrics Commands

```
asanadw metrics user <EMAIL_OR_GID> --period <PERIOD>
asanadw metrics project <GID_OR_URL> --period <PERIOD>
asanadw metrics portfolio <GID_OR_URL> --period <PERIOD>
asanadw metrics team <NAME_OR_GID_OR_URL> --period <PERIOD>
```

#### Summarize Commands

```
asanadw summarize task <GID> [--force]
asanadw summarize user <EMAIL_OR_GID> --period <PERIOD> [--force]
asanadw summarize project <GID_OR_URL> --period <PERIOD> [--force]
asanadw summarize portfolio <GID_OR_URL> --period <PERIOD> [--force]
asanadw summarize team <NAME_OR_GID_OR_URL> --period <PERIOD> [--force]
```

#### Status Command

```
asanadw status
```

Displays a summary of the local data warehouse:
- Total tasks, projects, portfolios, and users synced
- Date range coverage (earliest and latest synced data)
- Last sync time per monitored entity
- Database file size
- Count of pending sync gaps (ranges that need re-syncing)

#### Config Commands

```
asanadw config get <KEY>
asanadw config set <KEY> <VALUE>
asanadw config list
```

## URL Support

Accept Asana URLs anywhere a GID is expected:

```
# These are equivalent:
asanadw sync portfolio 1208241409266353
asanadw sync portfolio https://app.asana.com/0/portfolio/1208241409266353/list

# Project URL
asanadw sync project https://app.asana.com/0/1234567890/board

# Task URL
asanadw sync project https://app.asana.com/0/1234567890/9876543210
```

## Configuration

The Asana API token is provided via the `ASANA_TOKEN` environment variable. All other settings are managed through the `config` commands and stored in the database.

| Key | Description | Default |
|-----|-------------|---------|
| `workspace_gid` | Default workspace | (auto-detected) |
| `default_days` | Default sync window (days) | 90 |
| `llm_provider` | LLM provider (`bedrock` or `anthropic`) | bedrock |
| `llm_model` | Model for summarization | claude-opus-4-6 |

**LLM Provider Configuration:**

AI summarization is powered by [mixtape-core](https://github.com/adlio/mixtape) and supports two providers:

- **Anthropic API** — Set `llm_provider` to `anthropic`. Requires the `ANTHROPIC_API_KEY` environment variable.
- **AWS Bedrock** — Set `llm_provider` to `bedrock`. Uses standard AWS credential resolution (environment variables, `~/.aws/credentials`, IAM roles, etc.). No additional API key is needed.

## Non-Goals (v1)

- Real-time webhook sync (polling only)
- Multi-user/team database (single-user focus)
- Attachment file storage (metadata only)
- Write-back to Asana (read-only)
