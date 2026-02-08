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

Sync Asana data to local SQLite with full fidelity:

| Entity Type | What Gets Synced |
|-------------|------------------|
| User | All tasks assigned to the user |
| Team | All projects owned by the team, plus their tasks |
| Portfolio | All projects in portfolio, plus their tasks |
| Project | All tasks, subtasks, comments, status updates |

**Sync Depth:**
- Tasks with all standard and custom fields
- Subtasks (recursive, unlimited depth)
- Comments and activity stories
- Attachment metadata (not file contents)
- Project/portfolio status updates

**Sync Modes:**
- `sync <entity>` - One-time sync
- `monitor add` + `sync all` - Continuous monitoring

### 2. Entity Monitoring

Track entities for automatic synchronization:

```
asanadw monitor add portfolio 1208241409266353
asanadw monitor add user user@example.com
asanadw monitor list
asanadw sync all --days 30
```

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

### 4. Querying

Flexible queries with filtering and output formats:

```
asanadw query --assignee user@example.com --period 2025-W05 --json
asanadw query --project "Backend Redesign" --completed --csv
asanadw list tasks --portfolio "VX Team Portfolio"
```

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

### 6. AI Summarization

LLM-powered summaries for tasks and periods:

```
asanadw summarize task 1234567890
asanadw summarize user user@example.com --period 2025-W05
asanadw summarize project 1208241409266353 --period 2026-Q1
```

**Task Summary Output:**
- Headline (1-2 sentences)
- What happened (factual description)
- Why it matters (business context)
- Complexity signal (trivial/straightforward/involved/substantial)
- Notability score (0-10)

**Period Summary Output:**
- Headline capturing the period's theme
- Key accomplishments (bullet list)
- Collaboration notes
- Health assessment (for projects/portfolios)

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
asanadw sync user <EMAIL_OR_GID> [--days N]
asanadw sync team <NAME_OR_GID_OR_URL> [--days N]
asanadw sync portfolio <GID_OR_URL> [--days N]
asanadw sync project <GID_OR_URL> [--days N]
asanadw sync all [--days N]
```

#### Monitor Commands

```
asanadw monitor add user <EMAIL_OR_GID>
asanadw monitor add team <NAME_OR_GID>
asanadw monitor add portfolio <GID_OR_URL>
asanadw monitor add project <GID_OR_URL>
asanadw monitor remove <ENTITY_TYPE> <IDENTIFIER>
asanadw monitor list
```

#### Query Commands

```
asanadw list tasks [--portfolio NAME] [--project NAME] [--assignee EMAIL]
asanadw list projects [--portfolio NAME] [--team NAME]
asanadw query [--assignee EMAIL] [--project NAME] [--period STR] [--completed]
asanadw search <QUERY> [--type tasks|projects|comments] [--assignee EMAIL]
```

#### Metrics Commands

```
asanadw metrics user <EMAIL_OR_GID> --period <PERIOD>
asanadw metrics project <GID_OR_URL> --period <PERIOD>
asanadw metrics portfolio <GID_OR_URL> --period <PERIOD>
asanadw metrics team <NAME_OR_GID> --period <PERIOD>
```

#### Summarize Commands

```
asanadw summarize task <GID> [--force]
asanadw summarize user <EMAIL> --period <PERIOD> [--force]
asanadw summarize project <GID> --period <PERIOD> [--force]
```

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

| Key | Description | Default |
|-----|-------------|---------|
| `asana_token` | Personal Access Token | (required) |
| `workspace_gid` | Default workspace | (auto-detected) |
| `default_days` | Default sync window | 90 |
| `llm_model` | Model for summarization | claude-3-sonnet |

## Non-Goals (v1)

- Real-time webhook sync (polling only)
- Multi-user/team database (single-user focus)
- Attachment file storage (metadata only)
- Write-back to Asana (read-only)
