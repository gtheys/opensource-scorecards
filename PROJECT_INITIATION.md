# Project Initiation Document — Open Source Scorecards

| Field        | Value                                  |
| ------------ | -------------------------------------- |
| Project      | Open Source Scorecards                 |
| Status       | Draft                                  |
| Author       | Geert                                  |
| Created      | 2025                                   |
| Repository   | /home/geert/Code/personal/opensource-scorecards |

---

## 1. Purpose

Build a public web page that scores open-source projects within a category
(neovim plugins, DevOps tools, TUIs, ...) on "how good / healthy is this
project", using data pulled from GitHub and the wider internet.

The score answers: *"If I adopt this project today, how likely is it to be
maintained, supported, and safe a year from now?"*

## 2. Problem Statement

Choosing between open-source alternatives is manual research: checking stars,
last commit, open issues, author reputation across multiple tabs. Existing
tools (Snyk Advisor, npms, OpenSSF Scorecard) are either ecosystem-locked,
security-only, or commercial. No free, cross-category, community-health-focused
leaderboard exists for niches like neovim plugins or TUIs.

## 3. Goals & Non-Goals

### Goals

- Compute a single 0–100 health score per project from measurable signals.
- Category pages ranking projects (neovim plugins first).
- Per-project breakdown page showing how the score was composed.
- Static site, regenerated daily, zero/minimal hosting cost.
- Transparent, documented scoring model (weights public, tunable).

### Non-Goals

- Security vulnerability scanning (defer to OpenSSF Scorecard as an input).
- Package-registry downloads analysis (v2+).
- User accounts, submissions, comments.
- Commercial "press release" monitoring (low signal-per-effort).

## 4. Scoring Model (v1)

Weighted sum of three buckets, each 0–100, normalized **within category**:

```
score = 0.40 * maintenance + 0.35 * community + 0.25 * quality
```

### 4.1 Maintenance (40%)

| Signal                  | Source         | Notes                              |
| ----------------------- | -------------- | ---------------------------------- |
| Days since last commit  | GitHub API     | Decay curve, not cliff             |
| Commit frequency (90d)  | GitHub API     | Bot commits filtered               |
| Release cadence         | GitHub API     | Releases/tags per year             |
| Median issue close time | GitHub API     | Last 50 closed issues              |
| Median PR close time    | GitHub API     | Last 50 closed PRs                 |
| Open/closed issue ratio | GitHub API     | Backlog health                     |
| Archived flag           | GitHub API     | Hard penalty, not zero (see §7.2)  |

### 4.2 Community (35%)

| Signal                | Source         | Notes                              |
| --------------------- | -------------- | ---------------------------------- |
| Stars (log scale)     | GitHub API     | Raw counts lie; log10              |
| Forks                 | GitHub API     |                                    |
| Contributor count     | GitHub API     | Distinct authors, 12 months        |
| Bus factor            | GitHub API     | % commits by top author            |
| Author track record   | GitHub API     | Sum of author's other repos' stars (log), account age, org membership |

### 4.3 Quality (25%)

| Signal              | Source              | Notes                        |
| ------------------- | ------------------- | ---------------------------- |
| CI configured       | GitHub API          | workflows / status checks    |
| Tests present       | repo tree           | heuristic: test dirs/files   |
| README/docs present | GitHub API          | length threshold             |
| License             | GitHub API          | OSI-approved = full marks    |
| OpenSSF Scorecard   | scorecard.dev API   | optional input, cached       |

### 4.4 Category normalization

Signals are min-max or percentile normalized **within each category** before
weighting. A 500-star neovim plugin is a top project; a 500-star DevOps tool
is mid-tier. Cross-category scores are not comparable — the UI must say so.

## 5. Data Sources

| Source            | Cost  | Rate limit          | Used for                          |
| ----------------- | ----- | ------------------- | --------------------------------- |
| GitHub REST API   | free  | 5,000 req/hr authed | All repo signals (v1)             |
| GitHub GraphQL    | free  | shared              | Batch author/repo queries         |
| ecosyste.ms API   | free  | generous            | Dependents, funding (v2)          |
| OpenSSF Scorecard | free  | weekly public data  | Security posture input            |
| HN Algolia API    | free  | none documented     | Mention count / "buzz" (v2)       |
| GH Archive        | free  | BigQuery quota      | Historical trends (v2+)           |

Category seed lists come from curated awesome-lists (e.g. awesome-neovim),
parsed to extract GitHub repo URLs.

## 6. Architecture (v1)

```
awesome-list ──> seed parser ──> repo list (yaml/json)
                                      │
              GitHub API collector (daily cron, GitHub Actions)
                                      │
                              raw data cache (json)
                                      │
                            scoring engine (weights in config)
                                      │
                        static site generator (scores.json → HTML)
                                      │
                            GitHub Pages / Cloudflare Pages
```

- **Language:** Python or TypeScript — pick at implementation time; both
  have mature GitHub API clients. Decision deferred to first spike.
- **Storage:** none. JSON artifacts committed or kept as workflow artifacts.
- **Hosting:** static pages only. No backend in v1.

## 7. Risks & Mitigations

### 7.1 Gaming (Goodhart's law)

Stars are buyable; activity can be faked. Mitigation: weight recency and
issue-resolution behavior over raw popularity; log-scale all popularity
metrics; document that the score measures *maintenance likelihood*, not
code quality.

### 7.2 "Done" vs "dead"

Stable projects with no recent commits are not necessarily abandoned.
Mitigation: archived flag is a penalty, not a zero; if issues are still being
answered, maintenance score floors at a non-zero value. Category-level
calibration (a colorscheme plugin needs less churn than a linter).

### 7.3 Bot-inflated activity

Dependabot/Renovate inflate commit and PR counts. Mitigation: filter known
bot authors (`[bot]` suffix, known app logins) from all activity metrics.

### 7.4 Rate limits

5k req/hr covers ~500 repos with ~10 calls each — sufficient for v1
(one category, daily refresh). Mitigation for growth: GraphQL batching,
ecosyste.ms as fallback, incremental refresh (only stale repos).

### 7.5 Maintainer backlash

Projects scoring poorly may object. Mitigation: fully public methodology,
per-signal breakdown on project pages, clear "this measures X, not Y"
disclaimers.

## 8. Milestones

| # | Milestone                                   | Effort est. |
| - | ------------------------------------------- | ----------- |
| 1 | Seed parser + GitHub collector for neovim plugins | 1 evening |
| 2 | Scoring engine v1 + weights config          | 1 evening   |
| 3 | Static page (ranked table + per-project breakdown) | 1 evening |
| 4 | Daily GitHub Action + Pages deploy          | ½ evening   |
| 5 | v2: second category (TUIs or DevOps tools), ecosyste.ms dependents, HN mentions | later     |

## 9. Success Criteria

- v1 live: one category, ≥50 repos scored, daily refresh, public methodology.
- Score passes the sniff test: known-good projects (e.g. telescope.nvim)
  top the list; known-dead projects bottom it. Manual review of top/bottom 10.
- Anyone can answer "why did X score Y" from the breakdown page alone.

## 10. Open Questions

1. Language/stack: Python (PyGithub/httpx) vs TypeScript (Octokit) — decide in first spike.
2. Scoring curve shapes per signal (linear decay vs step) — tune after first data pull.
3. Do archived-but-popular projects get excluded or shown with penalty? (Current: penalty.)
4. Site generator: plain HTML template vs Astro/Hugo — decide when building milestone 3.
