# Open Source Scorecards

How healthy is an open-source project? A 0–100 score answering:
*"If I adopt this project today, how likely is it to be maintained, supported,
and safe a year from now?"*

The score measures **maintenance likelihood**, not code quality. Weights and
methodology are public (see `config/weights.toml` and the rendered
methodology page). Scores are normalized **within a category** and are not
comparable across categories.

## Pipeline

```
awesome-list ──▶ seed ──▶ data/<cat>/repos.yaml
GitHub API  ──▶ collect ─▶ data/<cat>/raw/*.json   (cached, bot-filtered)
raw cache   ──▶ score ───▶ data/<cat>/scores.json  (full per-signal breakdown)
scores      ──▶ render ──▶ site/                   (static, no backend)
```

## Run locally

```bash
cargo build --release
export GITHUB_TOKEN=$(gh auth token)   # or any classic PAT
./target/release/scorecards seed       # repo list from awesome-neovim
./target/release/scorecards collect    # fetch signals (resumable; ~11 calls/repo)
./target/release/scorecards score      # compute scores
./target/release/scorecards render     # static site → site/
```

Note: a full neovim-category refresh is ~15k API calls. Unauthenticated-free
options don't exist; with a 5k/hr PAT the cache makes runs resumable over a few
hours. Incremental runs (staleness threshold, default 20h) only refetch stale
repos.

## Tuning the score

Edit `config/weights.toml` (bucket weights, per-signal weights, archived
penalty, maintenance floor, bot logins), then:

```bash
./target/release/scorecards score && ./target/release/scorecards render
```

No code changes needed; the methodology page regenerates from the same file.

## Development

```bash
cargo test           # unit + fixture tests (no live API)
cargo clippy -- -D warnings
```

Planning docs: `PROJECT_INITIATION.md`, `openspec/changes/v1-scorecard-pipeline/`.
