use crate::config::Config;
use crate::score::{ScoredCategory, ScoredProject};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::path::Path;
use tera::{Context, Tera};

pub fn run(cfg: &Config, category: &str) -> anyhow::Result<()> {
    let cat = cfg.category(category)?;
    let scored: ScoredCategory =
        serde_json::from_str(&std::fs::read_to_string(cat.data_dir.join("scores.json"))?)?;
    render_to(cfg, &scored, Path::new("site"), category, &cat.data_dir)
}

fn templates() -> anyhow::Result<Tera> {
    let mut tera = Tera::new();
    // AIDEV-NOTE: register filters before load_from_glob — Tera validates filter names at template-parse time.
    tera.register_filter("stars_fmt", stars_fmt);
    tera.load_from_glob("templates/**/*.html")?;
    Ok(tera)
}

/// Formats star counts for display: ≥1000 → `{:.1}k`, <1000 → digits, null → em dash.
fn stars_fmt(
    value: &tera::Value,
    _kwargs: tera::Kwargs,
    _state: &tera::State,
) -> tera::TeraResult<tera::Value> {
    let out = match value.as_u64() {
        Some(n) if n >= 1000 => format!("{:.1}k", n as f64 / 1000.0),
        Some(n) => n.to_string(),
        None => "—".to_string(),
    };
    Ok(tera::Value::from(out))
}

// AIDEV-NOTE: display metadata (description, licence, last commit) lives in the raw
// cache, not in scores.json — load it lazily at render time so the UI can show it
// without the scorer having to carry it. Absent raw dir → empty map, renders fine.
#[derive(serde::Deserialize)]
struct RawRecord {
    slug: String,
    data: Option<RawData>,
}

#[derive(serde::Deserialize)]
struct RawData {
    #[serde(default)]
    archived: bool,
    description: Option<String>,
    license_spdx: Option<String>,
    last_commit_at: Option<DateTime<Utc>>,
}

fn load_meta(data_dir: &Path) -> std::collections::HashMap<String, Value> {
    let mut map = std::collections::HashMap::new();
    let Ok(entries) = std::fs::read_dir(data_dir.join("raw")) else {
        return map;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().is_some_and(|e| e == "json") {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(r) = serde_json::from_str::<RawRecord>(&text) else {
                continue;
            };
            let Some(d) = r.data else { continue };
            map.insert(
                r.slug,
                json!({
                    "description": d.description,
                    "license": d.license_spdx,
                    "last_commit": d.last_commit_at.map(|t| t.format("%Y-%m-%d").to_string()),
                    "archived": d.archived,
                }),
            );
        }
    }
    map
}

/// Rank deltas vs the previous scoring day, from `history.json` (JSONL written
/// by `score`). Compares the two most recent distinct dates present. Returns
/// top risers and fallers (by rank movement) and drops (newly unscored repos).
/// Empty vec when there's nothing to compare against — hub hides the section.
fn compute_movers(data_dir: &Path, scored: &ScoredCategory) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(data_dir.join("history.json")) else {
        return Vec::new();
    };
    // Parse all entries, keep {date -> scores map}, sorted by date.
    let mut days: Vec<(String, std::collections::HashMap<String, f64>)> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).ok()?;
            let date = v["date"].as_str()?.to_string();
            let scores = v["scores"]
                .as_object()?
                .iter()
                .filter_map(|(k, val)| val.as_f64().map(|f| (k.clone(), f)))
                .collect();
            Some((date, scores))
        })
        .collect();
    days.sort_by(|a, b| a.0.cmp(&b.0));
    if days.len() < 2 {
        return Vec::new();
    }
    let (prev_date, prev) = days[days.len() - 2].clone();

    // Current ranks (same ordering logic as the leaderboard).
    let mut current: Vec<&ScoredProject> = scored
        .projects
        .iter()
        .filter(|p| p.total.is_some())
        .collect();
    current.sort_by(|a, b| {
        b.total
            .partial_cmp(&a.total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let prev_rank = |slug: &str| -> Option<usize> {
        let mut scored_prev: Vec<(String, f64)> =
            prev.iter().map(|(s, t)| (s.clone(), *t)).collect();
        scored_prev.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_prev
            .iter()
            .position(|(s, _)| s == slug)
            .map(|i| i + 1)
    };

    let mut movers: Vec<(usize, Value)> = Vec::new(); // (abs rank delta, row)
    for (i, p) in current.iter().enumerate() {
        let cur_rank = i + 1;
        let Some(prev_r) = prev_rank(&p.slug) else {
            continue;
        };
        let delta = prev_r as i64 - cur_rank as i64; // >0 = moved up
        if delta == 0 || movers.len() >= 12 {
            continue;
        }
        movers.push((
            delta.unsigned_abs() as usize,
            json!({
                "slug": p.slug,
                "delta": delta,
                "rank": cur_rank,
                "prev_rank": prev_r,
                "total": p.total,
                "prev_date": prev_date,
            }),
        ));
    }
    movers.sort_by_key(|a| std::cmp::Reverse(a.0));
    movers.into_iter().map(|(_, row)| row).take(8).collect()
}

/// One display row for the leaderboard: score data + metadata merged, pre-ranked.
fn lb_row(
    rank: usize,
    p: &crate::score::ScoredProject,
    meta: &std::collections::HashMap<String, Value>,
) -> Value {
    let m = meta.get(&p.slug);
    json!({
        "rank": rank,
        "slug": p.slug,
        "owner": p.slug.split('/').next().unwrap_or(&p.slug),
        "total": p.total,
        "stars": p.stars,
        "buckets": p.buckets,
        "description": m.and_then(|m| m.get("description")).cloned().unwrap_or(Value::Null),
        "license": m.and_then(|m| m.get("license")).cloned().unwrap_or(Value::Null),
    })
}

/// Per-category summary for the hub cards. Reads every category's committed
/// scores.json so the hub is complete no matter which render run writes it.
fn category_cards(cfg: &Config) -> Vec<Value> {
    let mut cats: Vec<_> = cfg.categories.iter().collect();
    cats.sort_by(|a, b| a.0.cmp(b.0));
    cats.iter()
        .map(|(key, cat)| {
            let scored: Option<ScoredCategory> =
                std::fs::read_to_string(cat.data_dir.join("scores.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok());
            match scored {
                Some(s) => {
                    let ranked: Vec<_> = s.projects.iter().filter(|p| p.total.is_some()).collect();
                    let top = ranked.iter().max_by(|a, b| {
                        a.total
                            .partial_cmp(&b.total)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    json!({
                        "key": key,
                        "name": cat.name,
                        "total": s.projects.len(),
                        "scored": ranked.len(),
                        "top_slug": top.map(|p| p.slug.clone()),
                        "top_total": top.and_then(|p| p.total),
                        "generated_date": s.generated_at.format("%Y-%m-%d").to_string(),
                    })
                }
                None => json!({
                    "key": key,
                    "name": cat.name,
                    "total": 0,
                    "scored": 0,
                    "top_slug": null,
                    "top_total": null,
                    "generated_date": null,
                }),
            }
        })
        .collect()
}

/// Renders one category. Leaderboard goes to `<key>.html`; every run also
/// rewrites the shared `index.html` hub and `methodology.html` (idempotent).
pub fn render_to(
    cfg: &Config,
    scored: &ScoredCategory,
    out_dir: &Path,
    category_key: &str,
    data_dir: &Path,
) -> anyhow::Result<()> {
    let tera = templates()?;
    std::fs::create_dir_all(out_dir.join("projects"))?;
    std::fs::create_dir_all(out_dir.join("authors"))?;
    copy_assets(out_dir);
    // AIDEV-NOTE: projects/ is a shared namespace across categories — a repo in
    // two lists gets one page (last render wins). Lua/TS overlap ~zero; revisit if real.

    let mut categories: Vec<_> = cfg
        .categories
        .iter()
        .map(|(key, cat)| json!({"key": key, "name": cat.name}))
        .collect();
    categories.sort_by(|a, b| a["key"].as_str().cmp(&b["key"].as_str()));
    let category_name = &cfg.category(category_key)?.name;

    let mut ranked: Vec<_> = scored
        .projects
        .iter()
        .filter(|p| p.total.is_some())
        .collect();
    ranked.sort_by(|a, b| {
        b.total
            .partial_cmp(&a.total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let unavailable: Vec<_> = scored
        .projects
        .iter()
        .filter(|p| p.total.is_none())
        .collect();

    let meta = load_meta(data_dir);
    let cards = category_cards(cfg);
    let total_scored: usize = cards
        .iter()
        .filter_map(|c| c["scored"].as_u64())
        .sum::<u64>() as usize;
    let generated_date = scored.generated_at.format("%Y-%m-%d").to_string();

    // AIDEV-NOTE: top movers come from history.json (appended by `score`).
    // Local rank deltas within THIS category vs the previous scoring day.
    // No history, or only one day of it → empty list, hub hides the section.
    let movers = compute_movers(data_dir, scored);

    // Leaderboard (per-category page, root="" for top-level links)
    let rows: Vec<Value> = ranked
        .iter()
        .enumerate()
        .map(|(i, p)| lb_row(i + 1, p, &meta))
        .collect();
    let mut ctx = Context::new();
    ctx.insert("category_name", category_name);
    ctx.insert("generated_at", &scored.generated_at.to_rfc3339());
    ctx.insert("generated_date", &generated_date);
    ctx.insert("rows", &rows);
    ctx.insert("unavailable", &unavailable);
    ctx.insert("categories", &categories);
    ctx.insert("root", "");
    write(
        &tera,
        "leaderboard.html",
        &ctx,
        &out_dir.join(format!("{category_key}.html")),
    )?;

    // Per-project breakdown
    for p in &scored.projects {
        let mut ctx = Context::new();
        ctx.insert("p", p);
        ctx.insert("weights", &scored.weights);
        ctx.insert("category_key", category_key);
        ctx.insert("category_name", category_name);
        ctx.insert("categories", &categories);
        ctx.insert("root", "../");
        // Display metadata + rank for the plain-language header. Rank is 1-based
        // within the ranked set; unranked projects get null.
        ctx.insert("m", meta.get(&p.slug).unwrap_or(&Value::Null));
        let rank = ranked.iter().position(|r| r.slug == p.slug).map(|i| i + 1);
        ctx.insert("rank", &rank);
        ctx.insert("rank_total", &ranked.len());
        ctx.insert("owner", p.slug.split('/').next().unwrap_or(&p.slug));
        let mut rows: Vec<_> = p
            .signals
            .as_ref()
            .map(|m| {
                let mut v: Vec<_> = m
                    .iter()
                    .map(|(name, s)| {
                        json!({
                            "name": name,
                            "raw": (s.raw * 1000.0).round() / 1000.0,
                            "score": (s.score * 1000.0).round() / 1000.0,
                            "weight": s.weight,
                        })
                    })
                    .collect();
                v.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
                v
            })
            .unwrap_or_default();
        rows.shrink_to_fit();
        ctx.insert("signal_rows", &rows);
        let file = out_dir.join(format!("projects/{}.html", p.slug.replace('/', "__")));
        write(&tera, "project.html", &ctx, &file)?;
    }

    // AIDEV-NOTE: per-owner pages. Owner = slug prefix; avatar/profile URLs are
    // deterministic (github.com/<owner>.png) so no API data needed. authors/ is a
    // shared namespace across categories — same caveat as projects/ above.
    let mut by_owner: std::collections::BTreeMap<&str, Vec<_>> = Default::default();
    for p in &scored.projects {
        by_owner
            .entry(p.slug.split('/').next().unwrap_or(&p.slug))
            .or_default()
            .push(p);
    }
    for (owner, mut repos) in by_owner {
        repos.sort_by(|a, b| {
            b.total
                .partial_cmp(&a.total)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut ctx = Context::new();
        ctx.insert("owner", owner);
        ctx.insert("repos", &repos);
        ctx.insert("categories", &categories);
        ctx.insert("root", "../");
        write(
            &tera,
            "author.html",
            &ctx,
            &out_dir.join(format!("authors/{owner}.html")),
        )?;
    }

    // Methodology (from the same weights the scorer used → always in sync)
    let mut ctx = Context::new();
    ctx.insert("weights", &scored.weights);
    ctx.insert("categories", &categories);
    ctx.insert("root", "");
    write(
        &tera,
        "methodology.html",
        &ctx,
        &out_dir.join("methodology.html"),
    )?;

    // Category hub (rewritten by every render run; idempotent)
    let mut ctx = Context::new();
    ctx.insert("categories", &categories);
    ctx.insert("category_cards", &cards);
    ctx.insert("movers", &movers);
    ctx.insert("total_scored", &total_scored);
    ctx.insert("root", "");
    write(&tera, "categories.html", &ctx, &out_dir.join("index.html"))?;

    // AIDEV-NOTE: badges/ is a shared namespace across categories (same rule as
    // projects/). Feed is rewritten each run; only the current run's movers are
    // included, so the last render wins — fine for a daily-updated feed.
    write_badges(out_dir, category_key, &ranked, &categories)?;
    write_movers_feed(out_dir, cfg, &movers, category_key)?;

    println!(
        "rendered {} projects ({} ranked) to {}",
        scored.projects.len(),
        ranked.len(),
        out_dir.display()
    );
    Ok(())
}

/// Writes embeddable score badges: `badges/<cat>/<owner>__<repo>.svg`.
/// Flat-color shields (navy label, grade-colored value) so they work on any
/// README background with zero external dependencies. Linked to the project page.
fn write_badges(
    out_dir: &Path,
    category_key: &str,
    ranked: &[&ScoredProject],
    categories: &[Value],
) -> anyhow::Result<()> {
    let dir = out_dir.join("badges").join(category_key);
    std::fs::create_dir_all(&dir)?;
    let base = "https://gtheys.github.io/opensource-scorecards";
    let cat_name = categories
        .iter()
        .find(|c| c["key"].as_str() == Some(category_key))
        .and_then(|c| c["name"].as_str())
        .unwrap_or("Scorecards");
    for (i, p) in ranked.iter().enumerate() {
        let Some(total) = p.total else { continue };
        let rank = i + 1;
        let (color, _grade) = grade_color(total);
        let score_txt = format!("{total:.1}");
        let rank_txt = format!("#{rank}");
        let w_label = 70.0;
        let w_score = 24.0 + (score_txt.len() + rank_txt.len()) as f64 * 8.0;
        let w_total = w_label + w_score;
        let h = 20.0;
        let file_slug = p.slug.replace('/', "__");
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" role="img" aria-label="scorecard: {s} {r} in {c}"><title>{slug}: {s}/100, rank {r} in {c}</title><a href="{base}/projects/{f}.html"><g clip-path="url(#r)" shape-rendering="crispEdges"><rect width="{w}" height="{h}" fill="#101827"/><rect x="{wl}" width="{ws}" height="{h}" fill="{col}"/></g><g fill="#ffffff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" font-size="11"><text x="{cxl}" y="14">scorecard</text><text x="{cxs}" y="14">{s} &#183; {r}</text></g></a></svg>
"##,
            w = w_total,
            s = score_txt,
            r = rank_txt,
            c = html_escape(cat_name),
            slug = html_escape(&p.slug),
            f = file_slug,
            wl = w_label,
            ws = w_score,
            col = color,
            cxl = w_label / 2.0,
            cxs = w_label + w_score / 2.0,
        );
        std::fs::write(dir.join(format!("{file_slug}.svg")), svg)?;
    }
    Ok(())
}

/// Grade color for badge value segments, aligned with the site palette.
fn grade_color(total: f64) -> (&'static str, &'static str) {
    match total {
        t if t >= 80.0 => ("#f0b429", "A"), // gold
        t if t >= 65.0 => ("#187a45", "B"), // green
        t if t >= 50.0 => ("#4c5750", "C"), // grey-green
        _ => ("#9a3b3b", "D"),              // red
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Atom feed of the current run's biggest movers (one category per render run;
/// the hub is rewritten per run, so the last render wins — fine for a daily feed).
fn write_movers_feed(
    out_dir: &Path,
    cfg: &Config,
    movers: &[Value],
    category_key: &str,
) -> anyhow::Result<()> {
    let base = "https://gtheys.github.io/opensource-scorecards";
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let cat_name = cfg
        .categories
        .get(category_key)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| category_key.to_string());
    let mut entries = String::new();
    for m in movers {
        let Some(slug) = m["slug"].as_str() else {
            continue;
        };
        let delta = m["delta"].as_i64().unwrap_or(0);
        let rank = m["rank"].as_i64().unwrap_or(0);
        let total = m["total"].as_f64().unwrap_or(0.0);
        let arrow = if delta > 0 { "&#9650;" } else { "&#9660;" };
        let tot = format!("{total:.1}");
        let file = slug.replace('/', "__");
        let title = format!(
            "{arrow} {} {} {} &#8594; #{} ({:.1}) &#183; {}",
            delta.abs(),
            if delta > 0 { "up" } else { "down" },
            html_escape(slug),
            rank,
            total,
            html_escape(&cat_name),
        );
        entries.push_str(&format!(
            r##"  <entry>
    <id>urn:scorecards:{file}</id>
    <title>{title}</title>
    <link href="{base}/projects/{file}.html"/>
    <updated>{now}</updated>
    <summary>{arrow} {abs} places to #{rank}, score {tot} in {cat}</summary>
  </entry>
"##,
            abs = delta.abs(),
            cat = html_escape(&cat_name),
        ));
    }
    let feed = format!(
        r##"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Open Source Scorecards &#8212; biggest movers</title>
  <id>{base}/</id>
  <link href="{base}/" rel="alternate"/>
  <link href="{base}/movers.xml" rel="self"/>
  <updated>{now}</updated>
  <author><name>Open Source Scorecards</name></author>
{entries}</feed>
"##
    );
    std::fs::write(out_dir.join("movers.xml"), feed)?;
    Ok(())
}

fn write(tera: &Tera, template: &str, ctx: &Context, path: &Path) -> anyhow::Result<()> {
    let html = tera.render(template, ctx)?;
    std::fs::write(path, html)?;
    Ok(())
}

/// Copies repo-root assets/ (favicon, logo) into the site. Skips silently when
/// absent so tests and partial checkouts still render.
fn copy_assets(out_dir: &Path) {
    copy_dir_recursive(Path::new("assets"), &out_dir.join("assets"));
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    if std::fs::create_dir_all(dst).is_err() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(src) else {
        return;
    };
    for e in entries.flatten() {
        let from = e.path();
        let to = dst.join(e.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to);
        } else {
            let _ = std::fs::copy(&from, &to);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::score::{ScoredCategory, score_category};

    #[test]
    fn stars_fmt_cases() {
        let mut tera = Tera::default();
        tera.register_filter("stars_fmt", super::stars_fmt);
        tera.add_raw_template("t", "{{ s | stars_fmt }}").unwrap();
        let render = |s: Option<u64>| -> String {
            let mut ctx = Context::new();
            ctx.insert("s", &s);
            tera.render("t", &ctx).unwrap()
        };
        assert_eq!(render(None), "—");
        assert_eq!(render(Some(0)), "0");
        assert_eq!(render(Some(999)), "999");
        assert_eq!(render(Some(19709)), "19.7k");
    }

    fn fixture_scores() -> ScoredCategory {
        // Golden-input test: render from the real 3-repo scores.json if present,
        // else synthesize from the fixture records.
        let cfg = Config::load("config/weights.toml".into()).unwrap();
        let records: Vec<crate::models::RepoRecord> =
            ["tests/fixtures/nvim-telescope__telescope.nvim.json"]
                .iter()
                .map(|f| serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap())
                .collect();
        score_category(&cfg, "neovim", records)
    }

    #[test]
    fn render_pages() {
        let cfg = Config::load("config/weights.toml".into()).unwrap();
        let scored = fixture_scores();
        let dir = std::env::temp_dir().join("scorecards-render-test");
        let _ = std::fs::remove_dir_all(&dir);
        // Render both categories like CI does; hub/methodology are rewritten
        // every run. data_dir feeds the display-metadata loader.
        render_to(
            &cfg,
            &scored,
            &dir,
            "neovim",
            &cfg.category("neovim").unwrap().data_dir,
        )
        .unwrap();
        render_to(
            &cfg,
            &scored,
            &dir,
            "pi",
            &cfg.category("pi").unwrap().data_dir,
        )
        .unwrap();

        for page in ["neovim.html", "pi.html"] {
            let html = std::fs::read_to_string(dir.join(page)).unwrap();
            assert!(html.contains("not comparable across categories"));
            assert!(html.contains("nvim-telescope/telescope.nvim"));
            assert!(html.contains(r#"href="neovim.html""#));
            assert!(html.contains(r#"href="pi.html""#));
            assert!(html.contains("Stars"));
            assert!(html.contains("19.7k"));
        }

        let hub = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(hub.contains(r#"href="neovim.html""#));
        assert!(hub.contains(r#"href="pi.html""#));
        assert!(hub.contains("Neovim Plugins"));
        assert!(hub.contains("Pi Extensions"));
        // Hub v2: hero question + per-category summary cards.
        assert!(hub.contains("Which open source plugins are actually maintained?"));
        assert!(hub.contains("scored"));

        let project =
            std::fs::read_to_string(dir.join("projects/nvim-telescope__telescope.nvim.html"))
                .unwrap();
        assert!(project.contains("Total score:"));
        assert!(project.contains("Signal breakdown"));
        assert!(project.contains("license_osi"));
        assert!(project.contains("19.7k"));
        // Project pages live one level deep — nav links must be ../-prefixed.
        assert!(project.contains(r#"href="../pi.html""#));
        assert!(project.contains(r#"href="../methodology.html""#));
        // v2: repo link, owner link and rank line.
        assert!(project.contains(r#"href="https://github.com/nvim-telescope/telescope.nvim""#));
        assert!(project.contains(r#"href="../authors/nvim-telescope.html""#));
        assert!(project.contains("of 1"));

        let meth = std::fs::read_to_string(dir.join("methodology.html")).unwrap();
        assert!(meth.contains("0.4")); // maintenance bucket weight from config
        assert!(meth.contains("days_since_last_commit"));

        let lb = std::fs::read_to_string(dir.join("neovim.html")).unwrap();
        assert!(lb.contains(r#"href="authors/nvim-telescope.html""#));
        let author = std::fs::read_to_string(dir.join("authors/nvim-telescope.html")).unwrap();
        assert!(author.contains("github.com/nvim-telescope"));
        assert!(author.contains("nvim-telescope/telescope.nvim"));
        assert!(author.contains(r#"href="../projects/nvim-telescope__telescope.nvim.html""#));
    }
}
