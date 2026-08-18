## Purpose

Turns a curated awesome-list (markdown) into a canonical, deduplicated list of GitHub repos for a category, so the collector knows what to fetch.

## ADDED Requirements

### Requirement: Parse awesome-list into repo list

The system SHALL parse a category seed file (awesome-list markdown) and extract all GitHub repository URLs into a structured repo list (owner/name pairs) stored as YAML or JSON.

#### Scenario: Extract repos from awesome-neovim

- **WHEN** the seed command runs against the awesome-neovim list
- **THEN** a repo list is produced containing every linked GitHub repo as `owner/name`

#### Scenario: Non-GitHub links are skipped

- **WHEN** the seed list contains links to GitLab, SourceHut, or non-repo pages
- **THEN** those entries are excluded from the output

### Requirement: Deduplicate and normalize entries

The system SHALL deduplicate repos case-insensitively, strip trailing slashes/`.git` suffixes/URL fragments, and drop repos that no longer resolve.

#### Scenario: Duplicate entries merged

- **WHEN** the list contains `https://github.com/foo/bar` and `github.com/foo/bar/`
- **THEN** the output contains exactly one `foo/bar` entry

### Requirement: Stable, diff-friendly output

The system SHALL write the repo list sorted alphabetically so regenerating it produces minimal diffs.

#### Scenario: Regeneration is idempotent

- **WHEN** the seed command runs twice against an unchanged list
- **THEN** the two outputs are byte-identical
