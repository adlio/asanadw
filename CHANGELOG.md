# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-02-12

### Added

- Portfolio full-text search — portfolios now appear in search results alongside tasks, comments, and projects
- Multi-word search queries no longer require quoting (e.g. `asanadw search VX Team Portfolio`)

### Fixed

- FTS triggers now wrap nullable columns with COALESCE to prevent silent index corruption
- Task and comment upserts use ON CONFLICT DO UPDATE instead of INSERT OR REPLACE to avoid unnecessary FTS delete+insert cycles

## [0.1.1] - 2026-02-11

### Added

- Nested portfolio support — portfolios containing sub-portfolios are now recursively synced (up to 6 hierarchy levels)
- New `bridge_portfolio_portfolios` table linking parent and child portfolios
- Incremental sync via Events API for all resource types (sections, status updates, project metadata)
- Status update syncing for projects and portfolios

### Fixed

- Handle status update fetch failures gracefully instead of aborting sync
- Empty search queries now return empty results instead of erroring

## [0.1.0] - 2026-02-08

### Added

- Initial release
- Sync Asana projects, users, teams, and portfolios to local SQLite
- Incremental sync via Asana Events API
- Task querying with filters (assignee, project, date ranges, completion status)
- Full-text search across tasks, comments, projects, and custom fields
- Metrics computation (throughput, cycle time, overdue rates)
- LLM-powered summaries via Anthropic API or AWS Bedrock
- Star schema database with dimension, fact, and bridge tables
- Monitor system for tracking entities to sync
- Scheduling guidance for cron and launchd

[Unreleased]: https://github.com/adlio/asanadw/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/adlio/asanadw/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/adlio/asanadw/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/adlio/asanadw/releases/tag/v0.1.0
