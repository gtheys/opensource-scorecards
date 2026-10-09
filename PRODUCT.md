# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary: developers evaluating whether to adopt an open-source plugin or tool, who want a maintenance-health signal without manual multi-tab research. Secondary (confirmed): OSS maintainers checking their own score and badge.

## Product Purpose

Ranks open-source plugin ecosystems (Neovim plugins, Herdr plugins, Pi extensions, Monetized OSS) by maintenance health — a 0–100 score answering: *"If I adopt this project today, how likely is it to be maintained, supported, and safe a year from now?"* The score measures maintenance likelihood, not code quality. Success: anyone can answer "why did X score Y" from the per-project breakdown page alone, and known-good projects top the list while known-dead ones bottom it (sniff test).

## Positioning

Free, cross-category, community-health-focused leaderboard for niches (neovim plugins, TUIs) where existing tools fail: Snyk Advisor is ecosystem-locked, OpenSSF Scorecard is security-only, npms is npm-only. Mechanism competitors cannot truthfully copy: bot-filtered activity signals (Dependabot-only repos score as what they are), honest time decay, fully public and tunable weights (`config/weights.toml`), per-signal math on every project page, and scores normalized within a category — never comparable across categories.

## Operating Context

Static site regenerated nightly by a GitHub Action, published to GitHub Pages at <https://gtheys.github.io/opensource-scorecards/>. Zero-backend: `render` works entirely from committed data. Ranks tracked day over day; "Biggest movers" section and Atom feed (`movers.xml`) come from daily history. Projects embed auto-refreshing SVG badges in their READMEs linking back to full breakdowns. Categories are seeded from curated awesome-lists; adding a category is a config entry plus an awesome-list URL.

## Capabilities and Constraints

- Pipeline: `seed` → `collect` (resumable, cached, bot-filtered GitHub API) → `score` → `render` (static HTML, no backend).
- Score: `0.40 maintenance + 0.35 community + 0.25 quality`, each bucket 0–100, min-max/percentile normalized within category.
- Popularity metrics are log-scaled; archived flag is a penalty, not zero; maintenance score floors non-zero if issues still get answered ("done" vs "dead").
- Rate limit reality: 5k req/hr; full neovim refresh ~15k API calls via rate-limit-aware loop; incremental refetch (default 20h staleness).
- Non-goals (confirmed): security vulnerability scanning, user accounts/submissions/comments, commercial monitoring. Package-registry downloads deferred to v2+.

## Brand Commitments

- Name: **Open Source Scorecards**. Built by Geert Theys (gtheys).
- Binding voice: direct, evidence-first, mildly opinionated (e.g. "Star counts reward marketing", "rewards showing up"). The anti-stars POV is core identity, not a copy accident.
- Licensing is part of the brand: code AGPL-3.0-or-later (stays open even as a hosted service), data CC BY 4.0.
- Transparency commitments are binding: public methodology page, tunable public weights, full per-signal breakdowns, explicit "measures maintenance, not quality" disclaimers.

## Evidence on Hand

- Live site with 4 categories: Herdr plugins (~2,066 scored), Neovim plugins (~1,328), Pi extensions (73), Monetized OSS (~1,118).
- Real data in `data/` (scores, metrics, raw caches, history) — reusable under CC BY 4.0.
- Templates in `templates/`, rendered output in `site/` (index, per-category pages, per-project breakdowns, methodology, badges).
- No testimonials, press, or case studies — future work must not fabricate any.

## Product Principles

1. **Transparency over authority** — every score is explainable from public weights and per-signal math; no black box.
2. **Maintenance, not popularity** — stars are buyable; recency and issue-resolution behavior outweigh raw counts.
3. **Honest decay** — time penalizes honestly; a repo silent eight months scores the way it feels to depend on it.
4. **Category-relative truth** — normalization within category only; the UI must say scores are not cross-category comparable.
5. **Boring infrastructure** — static site, committed data, nightly cron; render never needs API access.
