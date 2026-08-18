# Proposal: v1-scorecard-pipeline

## Why

Choosing between open-source alternatives is manual research across multiple tabs. No free, cross-category, community-health leaderboard exists for niches like neovim plugins. This change builds the v1 pipeline: seed → collect → score → static site, answering "if I adopt this project today, how likely is it to be maintained, supported, and safe a year from now?"

## What Changes

- New Rust CLI binary (`scorecards`) with subcommands: `seed`, `collect`, `score`, `render`.
- Seed parser: extract GitHub repo URLs from awesome-neovim list into a YAML repo list.
- GitHub collector: fetch repo signals via `octocrab` (REST), cache raw JSON to disk, respect 5k req/hr limit, filter bot activity.
- Scoring engine: weighted 0–100 score = 0.40 maintenance + 0.35 community + 0.25 quality, signals min-max normalized within category. Weights in config file.
- Static site renderer: ranked category table + per-project score breakdown pages, plain HTML via `tera` templates.
- GitHub Action: daily cron running collect → score → render, deploy to GitHub Pages.
- Public methodology page documenting weights and signals.

## Capabilities

### New Capabilities

- `seed-parsing`: Parsing curated awesome-lists into a canonical repo list (owner/name) per category.
- `data-collection`: Fetching and caching GitHub repo signals (commits, issues, PRs, releases, contributors, CI, license, README) with rate-limit and bot-filtering behavior.
- `scoring`: Computing normalized, weighted health scores from raw signals, with documented weights and decay curves.
- `site-rendering`: Generating the static site: category leaderboard, per-project breakdown, methodology page.

### Modified Capabilities

(none — greenfield)

## Impact

- **New repo contents**: Cargo project, `config/` (weights, seeds), `data/` (JSON cache), `site/` output, `.github/workflows/`.
- **Dependencies**: `octocrab`, `tokio`, `serde`/`serde_json`/`serde_yaml`, `tera`, `clap`, `reqwest` (via octocrab).
- **External systems**: GitHub REST API (token via env), GitHub Actions + Pages hosting.
- **Out of scope (v2+)**: ecosyste.ms, HN mentions, GH Archive, OpenSSF live fetch, multiple categories, user accounts.
