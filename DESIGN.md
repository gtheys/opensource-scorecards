---
name: Open Source Scorecards
description: Maintenance-health scorecards for open source — scores from commit activity, releases, issue response, CI, and tests. Not stars.
colors:
  primary: "#187a45"
  primary-hover: "#136238"
  primary-bright: "#3fbf7f"
  monitor-navy: "#101827"
  navy-deep: "#0b1120"
  alert-gold: "#f0b429"
  gold-hover: "#e0a51f"
  gold-ink: "#1a1405"
  accent-soft: "#edf7f0"
  accent-soft-dark: "#14251c"
  border: "#e6eae7"
  muted: "#667069"
  hero-ink: "#f3f5f7"
  hero-sub: "#c3cbd6"
  ghost-border: "#3a4657"
  grade-c: "#4c5750"
  grade-d: "#9a3b3b"
typography:
  display:
    fontFamily: "DM Sans, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontWeight: 700
    letterSpacing: "-0.05em"
  headline:
    fontFamily: "DM Sans, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    letterSpacing: "-0.03em"
  body:
    fontFamily: "DM Sans, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontWeight: 400
  label:
    fontFamily: "DM Sans, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    fontSize: "clamp(.68rem, .6rem + .4vw, .74rem)"
    letterSpacing: "0.06em"
  mono:
    fontFamily: "IBM Plex Mono, SF Mono, Consolas, monospace"
    fontSize: "clamp(.72rem, .64rem + .4vw, .86rem)"
rounded:
  bar: "3px"
  md: "6px"
  lg: "8px"
  card: "10px"
  hero: "12px"
spacing:
  sm: "8px"
  md: "14px"
  lg: "16px"
  xl: "40px"
components:
  btn-primary:
    backgroundColor: "{colors.alert-gold}"
    textColor: "{colors.gold-ink}"
    rounded: "{rounded.lg}"
    padding: "0.65rem 1.2rem"
  btn-primary-hover:
    backgroundColor: "{colors.gold-hover}"
  btn-ghost:
    backgroundColor: "transparent"
    textColor: "#e6eaf0"
    rounded: "{rounded.lg}"
    padding: "0.65rem 1.2rem"
  card:
    backgroundColor: "{colors.hero-ink}"
    rounded: "{rounded.card}"
    padding: "1.1rem 1.2rem"
  card-hover:
    backgroundColor: "{colors.accent-soft}"
  score-bar:
    backgroundColor: "{colors.border}"
    rounded: "{rounded.bar}"
    height: "0.55rem"
  score-bar-fill:
    backgroundColor: "{colors.primary}"
    rounded: "{rounded.bar}"
  score-bar-fill-top:
    backgroundColor: "{colors.alert-gold}"
    rounded: "{rounded.bar}"
---

# Design System: Open Source Scorecards

## Overview

**Creative North Star: "The Vital Signs Monitor"**

Scorecards reads like a clinical readout for open-source health: dense tables, tabular numbers, and score bars that behave like telemetry. The aesthetic is clinical but warm — the Monitor Navy hero band and gold top-tier accents keep it from feeling like a spreadsheet export, and the copy voice (direct, mildly opinionated, "Not stars.") carries the personality so the chrome doesn't have to.

The system is built on Pico CSS (classless, vendored) with the identity re-pointed onto Pico's `--pico-*` variables. Decoration is minimal by doctrine: depth comes from borders and tint fills, emphasis comes from weight and color, and the data table is always the hero. Density is fluid — type, padding, and column budgets scale with `clamp()` instead of jumping at breakpoints.

**Key Characteristics:**
- Data tables as the primary interface; sticky headers, fixed layout, never horizontal scroll.
- Score bars everywhere a score appears; gold fill marks top-tier (rank ≤ 3 / grade A ≥ 80).
- Navy hero band + gold primary action on the hub; category pages are quiet working surfaces.
- Dark mode via OS preference (`prefers-color-scheme`), Pico's `data-theme` switch, set before first paint.
- Mono font for anything that is an identifier: repo slugs, owner names, top-project callouts.

## Colors

The palette is a clinical triad: navy for trust and chrome, green for health, gold for exceptional performance. Green is re-pointed onto Pico's primary role, so all Pico primitives (links, focus rings) inherit it.

### Primary
- **Healthy Green** (`--pico-primary`): links, interactive accents, score-bar fills, card hover borders, pillar rules. The "this project is alive" color.
- **Healthy Green Hover** (`--pico-primary-hover`): hover state for primary links/buttons.
- **Healthy Green Bright** (dark-mode `--pico-primary`): lifted green so links stay legible on dark surfaces. Dark mode keeps the deeper `--pico-primary-background` for filled elements.

### Tertiary
- **Alert Gold** (`--gold`): reserved for exceptional performance — top-3 score bars, badge grade A (≥80), and the hub's primary CTA. Rarity is the point.
- **Gold Hover**: hover state for the gold CTA.
- **Gold Ink**: text color on gold fills (near-black warm brown).

### Neutral
- **Monitor Navy** (`--navy`): hero band background, badge label segment, brand anchor.
- **Navy Deep**: dark-mode navy.
- **Soft Tint** (`--accent-soft`): row hover, card hover, notice backgrounds — depth without shadow.
- **Soft Tint Dark**: dark-mode tint.
- **Hairline Border** (`--pico-border-color`): all borders and the empty track of score bars.
- **Muted Slate** (`--pico-muted-color`): secondary text, table headers, footer.
- **Hero Ink / Hero Sub**: text and secondary text on the navy band.
- **Ghost Border**: ghost-button outline on dark surfaces.
- **Grade Grey-Green / Grade Red**: badge grades C (50–64) and D (<50) — the failure end of the scale, used only in badges.

### Named Rules
**The Gold Is Earned Rule.** Alert Gold appears only where a project earned it: top-tier rows, grade-A badges, and the single hub CTA. Never use gold decoratively — a gold accent on a mid-tier element lies about the data.

## Typography

**Display/Body Font:** DM Sans (with `-apple-system, BlinkMacSystemFont, Segoe UI` fallback)
**Mono Font:** IBM Plex Mono (with `SF Mono, Consolas` fallback)

**Character:** DM Sans's geometric warmth keeps dense data friendly; tight negative letter-spacing on headings gives the page its confident, editorial snap. IBM Plex Mono marks anything machine-shaped: slugs, owners, code, top-project callouts.

Note: the webfonts are declared but not shipped — most visitors see the system fallbacks. The stacks are chosen so the fallback (system sans / system mono) preserves the system's character. Load the real fonts only if the trade against static-site minimalism is re-decided deliberately.

### Hierarchy
- **Display** (700, letter-spacing −0.05em): page and hero `h1` — the question or the project slug.
- **Headline** (letter-spacing −0.03em): `h2` section headers and card counts (700 at 1.5rem).
- **Body** (400, 1rem Pico base): prose, descriptions; 62–70ch max on reading measure (hero sub 62ch, leaderboard intro 70ch).
- **Label** (`clamp(.68rem, .6rem + .4vw, .74rem)`, uppercase, letter-spacing .06em): table column headers, muted slate. Floor kept at 10.9px for phone legibility.
- **Mono** (`clamp(.72rem, .64rem + .4vw, .86rem)`): identifiers in tables, badge markdown snippets, card top-project lines.

### Named Rules
**The Identifiers Are Mono Rule.** Anything that names a machine artifact (repo slug, owner login, code snippet) is set in IBM Plex Mono, fluid-sized. Prose is never mono; identifiers are never sans.

## Layout

Single centered container (Pico `.container`). The hub is a navy hero band followed by a card grid (`auto-fit, minmax(260px, 1fr)`) and pillar grids (`auto-fit, minmax(230px, 1fr)`). Category pages are one full-width table under a short intro and the normalization notice.

Tables use `table-layout: fixed` with explicit fluid column budgets (`clamp()` widths) — the table can never scroll horizontally. Density is fluid: cell padding and font-size scale with the viewport. Progressive disclosure sheds secondary columns as the screen narrows: owner at ≤72rem, sub-scores at ≤62rem, licence at ≤48rem. Hidden data stays available on the per-project page.

## Elevation & Depth

Flat at rest, lift on state. There is no ambient shadow vocabulary — depth at rest comes from hairline borders and tint fills (Soft Tint). Pico's single default shadow is the only shadow in the system; a subtle lift is acceptable as a response to hover/focus, never as decoration.

### Named Rules
**The Flat-By-Default Rule.** Surfaces are flat. Borders and tints do the layering. Any new shadow must be tied to a state change, not to a resting element.

## Shapes

Quietly rounded, scaled to element size: score bars and tiny fills at 3px, notices and code blocks at 6px, buttons and the logo at 8px, cards at 10px, the hero band at 12px. Radius grows with surface area — nothing sharp, nothing pill-shaped (Pico's pill-radius search inputs are unused).

## Components

### Buttons
- **Shape:** gently rounded (8px), padding `.65rem 1.2rem`, weight 600.
- **Primary:** Alert Gold fill with Gold Ink text — one per page, on the hub hero only.
- **Ghost:** transparent with Ghost Border outline and Hero Ink text, for the secondary hero action on navy; hover lifts with a faint white wash.
- **Elsewhere:** Pico default link/button styling in Healthy Green.

### Cards
- **Shape:** rounded (10px), hairline border, flat background.
- **Content:** category name (`h2`), big count (700, 1.5rem, −0.03em), mono top-project line, muted updated date.
- **Hover:** border switches to Healthy Green, background to Soft Tint — the whole card is the link.

### Score Bars
- **Shape:** inline track (3px radius, .55rem height) with a fill whose width is the score percent; Healthy Green fill, Alert Gold for top-tier rows.
- **Behavior:** fluid width via `clamp()`; at ≤72rem the bar stacks under the number as a full-width cell bar.
- **Numbers:** right-aligned, `tabular-nums`, `white-space: nowrap` (`.num`).

### Tables
- Sticky uppercase Label headers on the page background; Hairline row borders; row hover in Soft Tint.
- Top-3 rows get gold bar fills and a bold rank number (`tr.top-tier`).
- Long slugs break with `overflow-wrap: anywhere` inside their column budget.

### Pillars & Notices
- **Pillar:** 3px Healthy Green left rule, padded left; used for "why" and "how it works" trios.
- **Notice:** Soft Tint background, 3px green left border, asymmetric radius (0 6px 6px 0) — used for the cross-category comparability warning and unavailable-repo states.

### Navigation
- Header: brand (logo + "Scorecards", 700, −0.045em) left; muted text links right, Healthy Green on hover. Hairline bottom border. Footer mirrors it with licence links.

### Badges (signature)
- SVG, `shape-rendering: crispEdges`: navy label segment reading "scorecard", grade-colored value segment (`A` gold ≥80, `B` green ≥65, `C` grey-green ≥50, `D` red <50) showing "score · rank". Verdana stack at 11px, white text. Links back to the project scorecard.

## Do's and Don'ts

### Do:
- **Do** put the comparability notice ("not comparable across categories") on every leaderboard — it is product truth, not decoration.
- **Do** keep every score next to its bar, and every bar filled by the actual percent.
- **Do** shed columns progressively (owner → buckets → licence) instead of allowing horizontal scroll.
- **Do** use mono for slugs, owners, and code; tabular-nums for all numbers.
- **Do** keep gold rare: top-tier fills, grade-A badges, one hub CTA.

### Don't:
- **Don't** add resting shadows or gradients — depth is borders and tints.
- **Don't** render stars more prominently than the score; star worship is the anti-position.
- **Don't** use gold or grade colors decoratively or on mid-tier elements.
- **Don't** let a table scroll horizontally or wrap a column header mid-label.
- **Don't** add webfont loading, JS frameworks, or runtime CDNs without re-deciding the static-minimalist trade explicitly.
