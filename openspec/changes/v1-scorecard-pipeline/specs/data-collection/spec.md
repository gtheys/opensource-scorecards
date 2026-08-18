## Purpose

Fetches raw project-health signals from the GitHub API for every repo in a category and caches them as JSON artifacts, so scoring runs on reproducible data without re-hitting the API.

## ADDED Requirements

### Requirement: Collect repo signals

The system SHALL fetch, per repo: repo metadata (stars, forks, archived flag, license, description), commit activity (last commit date, 90-day commit count), releases, open/closed issues, open/closed PRs, contributors, and presence of CI workflows, tests, and README.

#### Scenario: Full signal set collected

- **WHEN** the collect command runs for a category
- **THEN** every repo in the repo list has a raw JSON cache file containing all v1 signals

#### Scenario: Missing or moved repo

- **WHEN** a repo in the list returns 404 or 301 from the GitHub API
- **THEN** the repo is recorded as unavailable/redirected in the cache and the run continues with remaining repos

### Requirement: Respect rate limits

The system SHALL authenticate with a GitHub token, track remaining rate-limit budget, and complete a single-category run (≤500 repos) within the 5,000 req/hr authenticated limit.

#### Scenario: Rate limit approached

- **WHEN** the remaining rate budget drops below the number of calls needed to finish
- **THEN** the system sleeps until the reset window or stops with a clear error listing uncollected repos

### Requirement: Filter bot activity

The system SHALL exclude commits, PRs, and issues authored by known bots (login ending in `[bot]`, known app logins such as dependabot/renovate) from all activity counts.

#### Scenario: Dependabot PRs excluded

- **WHEN** a repo's last 50 closed PRs include dependabot PRs
- **THEN** those PRs do not count toward PR close-time or activity metrics

### Requirement: Incremental refresh

The system SHALL skip re-fetching a repo whose cache entry is younger than a configurable staleness threshold (default 20 hours), unless forced.

#### Scenario: Fresh cache reused

- **WHEN** collect runs twice within the staleness window
- **THEN** the second run makes no API calls for already-cached repos
