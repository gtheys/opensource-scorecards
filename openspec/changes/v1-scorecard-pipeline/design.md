# Design: v1-scorecard-pipeline

## Context

Greenfield repo containing only `PROJECT_INITIATION.md`. See proposal.md for motivation and scope. Constraints: zero/minimal hosting cost, daily refresh, one category (neovim plugins, ~50–500 repos), GitHub 5k req/hr authenticated limit.

## Goals / Non-Goals

**Goals:**
- Single Rust binary, four subcommands matching the pipeline stages.
- JSON artifacts on disk as the only "database"; every stage re-runnable independently.
- Deploy via one GitHub Actions workflow to GitHub Pages.

**Non-Goals:**
- No database, no daemon, no backend, no caching layer beyond files.
- No GraphQL batching (v1 fits in REST budget; see Risks).
- No live OpenSSF fetch in v1 (optional cached input later).

## Decisions

### D1: Rust + octocrab
`octocrab` is the de-facto maintained Rust GitHub client: typed models, pagination, rate-limit introspection. Alternatives: Python/PyGithub (faster spike, but user chose Rust); raw `reqwest` (re-implementing pagination/models for nothing).

### D2: One binary, staged subcommands
```
scorecards seed    awesome-neovim.md  → data/neovim/repos.yaml
scorecards collect data/neovim/repos.yaml → data/neovim/raw/<owner>__<repo>.json
scorecards score   data/neovim/raw/    → data/neovim/scores.json
scorecards render  data/neovim/scores.json → site/
```
Alternatives: four binaries (needless); one run-to-completion command (can't re-score without re-fetching — wasted API budget during weight tuning). Stage boundaries = file boundaries, so each stage is testable on fixtures alone.

### D3: Files as storage
Raw cache: one JSON file per repo (`<owner>__<repo>.json`) with a `fetched_at` timestamp — incremental refresh is an mtime/timestamp check. Scores: single `scores.json` per category. All committed to the repo by the Action, giving free history/diffs. Alternative (SQLite) adds a dependency and tooling for zero v1 benefit.

### D4: Weights + curves in one config file
`config/weights.toml`: bucket weights, per-signal weights, decay-curve parameters, bot login list, staleness threshold, category definitions. Score tuning = edit file, re-run `score`, diff `scores.json`. 

### D5: Rendering with tera templates
`tera` (Jinja-like) + a hand-written CSS file. Alternative: Astro/Hugo — extra toolchain, node dependency, for what is three page types. `format!` strings were considered; templates keep HTML out of Rust and are designer-editable.

### D6: GitHub Actions for the whole pipeline
Nightly cron: checkout → `cargo run -- collect` → `score` → `render` → commit `data/` + deploy `site/` to Pages. Token: `GITHUB_TOKEN` (5k req/hr, same repo). No secrets beyond that.

### D7: Bot filtering via login heuristics
Filter authors whose login ends in `[bot]` plus a config-listed set (dependabot, renovate, etc.). Applied at collection time so cached data is already clean; scoring never re-filters.

## Risks / Trade-offs

- REST call volume grows beyond budget with more categories → GraphQL batching via `graphql-client` (deferred; v1 worst case ~10 calls × 500 repos = at the limit, so per-repo call count must stay lean — use single `repo` endpoint + search/issues endpoints sparingly).
- "Done vs dead" mis-scores stable projects → non-zero maintenance floor + archived-as-penalty (see scoring spec); calibrate curves after first real data pull.
- Awesome-list formats drift between lists → seed parser targets awesome-neovim only in v1; generalize when adding category two.
- Committing `data/` grows the repo → JSON is small (~500 × few KB); revisit only if history bloat becomes real.
- Score gaming (Goodhart) → log-scale popularity, weight behavior over stars, methodology page states what the score measures.

## Open Questions

- Exact decay-curve shapes per signal — tune against first real data pull (does not change specs, only config values).
- GitHub Pages vs Cloudflare Pages — decide at milestone 4; both consume the same `site/` output.
