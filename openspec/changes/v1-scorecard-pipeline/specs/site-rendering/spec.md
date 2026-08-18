## Purpose

Renders score artifacts into a static site — category leaderboard, per-project breakdowns, and a public methodology page — deployable to any static host with no backend.

## ADDED Requirements

### Requirement: Category leaderboard page

The system SHALL render, per category, a ranked table of projects sorted by total score descending, showing total score and the three bucket scores.

#### Scenario: Ranking order

- **WHEN** the category page is rendered from score artifacts
- **THEN** projects appear in descending total-score order with rank numbers

#### Scenario: Cross-category disclaimer

- **WHEN** any leaderboard page is viewed
- **THEN** it displays a notice that scores are normalized within category and not comparable across categories

### Requirement: Per-project breakdown page

The system SHALL render, per project, a page showing every signal's raw value, normalized value, and contribution to the score.

#### Scenario: Self-explanatory score

- **WHEN** a user opens a project's breakdown page
- **THEN** they can answer "why did X score Y" from that page alone, without external tools

#### Scenario: Unavailable repo shown

- **WHEN** a repo was recorded as unavailable during collection
- **THEN** its page shows the unavailability state instead of a score, and it is excluded from ranking

### Requirement: Methodology page

The system SHALL render a methodology page documenting the scoring formula, all weights, all signals and their sources, and what the score does and does not measure.

#### Scenario: Weights published

- **WHEN** the methodology page is viewed
- **THEN** the current weights from the config file are accurately reflected

### Requirement: Fully static output

The system SHALL produce plain HTML/CSS/JSON output requiring no server-side runtime, suitable for GitHub Pages or Cloudflare Pages.

#### Scenario: Zero-backend deploy

- **WHEN** the output directory is uploaded to a static host
- **THEN** all pages and links work without any server-side code
