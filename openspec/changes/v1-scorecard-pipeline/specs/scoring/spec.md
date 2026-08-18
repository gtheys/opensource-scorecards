## Purpose

Computes a transparent 0–100 health score per project from cached raw signals, using public weights and within-category normalization, so users can see exactly why a project scored what it did.

## ADDED Requirements

### Requirement: Weighted three-bucket score

The system SHALL compute `score = 0.40 * maintenance + 0.35 * community + 0.25 * quality`, with each bucket 0–100, and weights loaded from a config file (not hardcoded).

#### Scenario: Weights are configurable

- **WHEN** the weights in the config file are changed
- **THEN** re-running score produces updated scores without code changes

### Requirement: Within-category normalization

The system SHALL normalize raw signal values within each category (min-max or percentile) before weighting, and SHALL mark scores as not comparable across categories.

#### Scenario: Category-relative popularity

- **WHEN** a 500-star repo is scored in a category whose max is 1,000 stars vs one whose max is 100,000 stars
- **THEN** it receives a higher normalized popularity value in the first category

### Requirement: Maintenance signals

The maintenance bucket SHALL combine: days since last commit (decay curve, not cliff), 90-day commit frequency, release cadence, median issue close time, median PR close time, and open/closed issue ratio — all computed on bot-filtered data.

#### Scenario: Archived repo penalized, not zeroed

- **WHEN** a repo has the archived flag set
- **THEN** its maintenance score receives a hard penalty but is not forced to zero if other signals (e.g. answered issues) are healthy

#### Scenario: Stable-but-alive repo floors above zero

- **WHEN** a repo has no recent commits but maintainers still answer issues
- **THEN** its maintenance score stays above a documented non-zero floor

### Requirement: Community signals

The community bucket SHALL combine: stars (log10 scale), forks, distinct contributors over 12 months, bus factor (% commits by top author), and author track record (other repos' stars, account age).

#### Scenario: Log-scaled stars

- **WHEN** two repos have 100 and 10,000 stars
- **THEN** the star signal ratio reflects log10(100) vs log10(10,000), not 1:100

### Requirement: Quality signals

The quality bucket SHALL combine: CI configured, tests present (heuristic), README/docs above length threshold, and OSI-approved license.

#### Scenario: No license, no marks

- **WHEN** a repo has no detectable OSI-approved license
- **THEN** the license signal scores zero

### Requirement: Explainable output

The system SHALL emit, per repo, a score artifact containing the total score, each bucket score, and every individual signal value (raw and normalized), sufficient to explain the score without re-running.

#### Scenario: Breakdown completeness

- **WHEN** the score artifact for a repo is inspected
- **THEN** every contributing signal's raw value, normalized value, and weight is present
