# Tasks: v1-scorecard-pipeline

## 1. Project Setup

- [x] 1.1 `cargo init` binary crate; add deps: clap, tokio, octocrab, serde, serde_json, serde_yaml, toml, tera, anyhow, thiserror, time
- [x] 1.2 CLI skeleton with clap subcommands `seed`, `collect`, `score`, `render` (stubs)
- [x] 1.3 `config/weights.toml` with v1 bucket weights (0.40/0.35/0.25), per-signal weights, bot login list, staleness threshold, neovim category entry
- [x] 1.4 CI: basic GitHub Action running `cargo test` + `cargo clippy`

## 2. Seed Parser

- [x] 2.1 Fetch/download awesome-neovim README (or accept local file path)
- [x] 2.2 Extract GitHub URLs from markdown links; normalize to `owner/name` (strip `.git`, trailing slash, fragments, `/tree/...` etc.)
- [x] 2.3 Skip non-GitHub links; dedupe case-insensitively; sort alphabetically
- [x] 2.4 Write `data/neovim/repos.yaml`; verify idempotency (two runs, byte-identical)
- [x] 2.5 Unit tests with markdown fixtures (dupes, gitlab links, suffixes)

## 3. Collector

- [x] 3.1 Raw cache module: per-repo JSON file at `data/neovim/raw/<owner>__<repo>.json` with `fetched_at`; skip-if-fresh logic + `--force`
- [x] 3.2 Repo metadata fetch: stars, forks, archived, license, description, README presence/length
- [x] 3.3 Activity fetch: last commit date, 90-day commits (bot-filtered), releases/tags
- [x] 3.4 Issues/PRs fetch: last 50 closed issues + PRs (bot-filtered), open counts
- [x] 3.5 Contributors fetch: distinct authors 12mo, top-author commit share, author track-record data
- [x] 3.6 Quality fetch: workflow files present, test dirs/files heuristic via repo tree
- [x] 3.7 Rate-limit guard: check remaining budget, abort/sleep with clear message; 404/301 recorded as unavailable/redirect
- [x] 3.8 Integration test against recorded JSON fixtures (no live API in tests)

## 4. Scoring Engine

- [x] 4.1 Signal computation from raw cache (per scoring spec: maintenance, community, quality signals)
- [x] 4.2 Within-category normalization (min-max; percentile if min-max proves outlier-fragile)
- [x] 4.3 Decay curve for days-since-last-commit; maintenance floor + archived penalty
- [x] 4.4 Bucket aggregation with config weights → total score
- [x] 4.5 Emit `scores.json` with full per-signal breakdown (raw, normalized, weight, contribution)
- [x] 4.6 Unit tests: fixture repos covering archived, stable-but-alive, bot-heavy, no-license cases

## 5. Renderer

- [x] 5.1 Tera templates: leaderboard, project breakdown, methodology, shared base + CSS
- [x] 5.2 Leaderboard page: ranked table, bucket scores, cross-category disclaimer
- [x] 5.3 Project pages: full signal breakdown; unavailable-repo state
- [x] 5.4 Methodology page generated from `config/weights.toml` (weights always in sync)
- [x] 5.5 Render test: snapshot/golden-file check on fixture scores.json

## 6. Automation & Deploy

- [x] 6.1 Nightly GitHub Action: collect → score → render, commit `data/`, using GITHUB_TOKEN
- [ ] 6.2 Deploy `site/` to GitHub Pages (or Cloudflare Pages — decide here)
- [x] 6.3 README: what the score measures, how to run locally, how to tune weights

## 7. Calibration (success criteria)

- [ ] 7.1 Full real-data run on awesome-neovim; sanity-check top/bottom 10 (telescope.nvim should rank high)
- [ ] 7.2 Tune decay curves/weights in config from real data; document final values in methodology
