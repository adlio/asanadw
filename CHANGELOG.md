# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/adlio/asanadw/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/adlio/asanadw/releases/tag/v0.1.0
