# Open Source Scorecards

[![CI](https://github.com/gtheys/opensource-scorecards/actions/workflows/ci.yml/badge.svg)](https://github.com/gtheys/opensource-scorecards/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/code-AGPL--3.0-blue)](LICENSE)
[![Data: CC BY 4.0](https://img.shields.io/badge/data-CC--BY--4.0-green)](#licence)
[![Live site](https://img.shields.io/badge/live-gtheys.github.io%2Fopensource--scorecards-101827)](https://gtheys.github.io/opensource-scorecards/)

**Which open source plugins are actually maintained?**

Open Source Scorecards ranks plugin ecosystems (Neovim plugins, Herdr plugins, Pi
extensions) by **maintenance health** - a 0-100 score answering:

*"If I adopt this project today, how likely is it to be maintained, supported,
and safe a year from now?"*

The score measures **maintenance likelihood**, not code quality. It is built from
bot-filtered commit activity, release cadence, issue/PR response times, CI,
tests, and docs - **not stars**. Weights are public and tunable
(`config/weights.toml`); every project page shows the full per-signal math.
Scores are normalized **within a category** and are never comparable across
categories.

**Live site:** <https://gtheys.github.io/opensource-scorecards/>

## Why not stars?

Star counts reward marketing. This scorer rewards showing up: a repo kept alive
only by Dependabot still scores like what it is, because bot activity is
filtered before scoring. Time decays honestly - a repo silent for eight months
scores the way it feels to depend on it.

## Embed your score

Scored projects get a badge that refreshes with every nightly run:

```markdown
[![scorecard](https://gtheys.github.io/opensource-scorecards/badges/pi/nicobailon__pi-subagents.svg)](https://gtheys.github.io/opensource-scorecards/projects/nicobailon__pi-subagents.html)
```

Swap the category and `owner__repo` slug for your project. Badges are plain
SVG, work on any README, and link back to the full score breakdown.

## Categories

| Category | Projects | Seeded from |
| --- | --- | --- |
| [Herdr plugins](https://gtheys.github.io/opensource-scorecards/herdr.html) | ~2,066 scored | [awesome-herdr](https://github.com/yigitkonur/awesome-herdr) |
| [Neovim plugins](https://gtheys.github.io/opensource-scorecards/neovim.html) | ~1,328 scored | [awesome-neovim](https://github.com/rockerBOO/awesome-neovim) |
| [Pi extensions](https://gtheys.github.io/opensource-scorecards/pi.html) | 73 scored | [awesome-pi](https://github.com/BubblePtr/awesome-pi) |

Missing your favourite ecosystem? [Open an issue](https://github.com/gtheys/opensource-scorecards/issues) -
adding a category is a config entry plus an awesome-list URL.

## How it works

```
awesome-list --> seed    --> data/<cat>/repos.yaml
GitHub API   --> collect --> data/<cat>/raw/*.json   (cached, bot-filtered)
raw cache    --> score   --> data/<cat>/scores.json  (+ history.json daily)
scores       --> render  --> site/                   (static, no backend)
```

A GitHub Action runs the full pipeline nightly and publishes to GitHub Pages.
Ranks are tracked day over day; the site's **Biggest movers** section and the
[Atom feed](https://gtheys.github.io/opensource-scorecards/movers.xml) come from
that history.

## Run locally

```bash
cargo build --release
export GITHUB_TOKEN=$(gh auth token)    # or any classic PAT

./target/release/scorecards seed --category pi
./target/release/scorecards collect --category pi
./target/release/scorecards score --category pi
./target/release/scorecards render --category pi

just serve   # renders all categories + serves site/ on :8000
```

Notes:

- `collect` is **resumable** - the raw cache is the state. Incremental runs
  only refetch repos older than the staleness threshold (default 20h).
- A full neovim refresh is ~15k API calls; `scripts/collect-loop.sh` sleeps
  through rate-limit windows until the cache is complete.
- `render` needs no API access - it works entirely from committed data.

## Tuning the score

Edit `config/weights.toml` (bucket weights, per-signal weights, archived
penalty, maintenance floor, bot logins), then:

```bash
./target/release/scorecards score && ./target/release/scorecards render
```

No code changes needed - the methodology page regenerates from the same file.

## Development

```bash
cargo test               # unit + golden-render tests (no live API)
cargo clippy -- -D warnings
```

## Licence

- **Code**: [AGPL-3.0-or-later](LICENSE) - open, auditable, and stays open even
  if this grows into a hosted service.
- **Data** (`data/` directory: scores, metrics, raw caches):
  [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) - reuse the dataset
  freely with attribution.

## Credits

Data sources: [awesome-neovim](https://github.com/rockerBOO/awesome-neovim),
[awesome-pi](https://github.com/BubblePtr/awesome-pi),
[awesome-herdr](https://github.com/yigitkonur/awesome-herdr).

Table/layout styling by [Pico CSS](https://picocss.com).

Built by [Geert Theys](https://geert.md) ([gtheys](https://github.com/gtheys)) -
[geerttheys.substack.com](https://geerttheys.substack.com).
