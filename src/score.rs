use crate::config::Config;
use crate::models::{RepoRecord, Status};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

// AIDEV-NOTE: two signal families, per PROJECT_INITIATION §4.4:
// - "normalized": raw counts min-max scaled within the category (popularity lies raw)
// - "absolute": curves/binary values already in 0..1 (time decays, ratios, flags)
// Bucket weights + per-signal weights live in config/weights.toml.

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct ScoredCategory {
    pub category: String,
    pub generated_at: DateTime<Utc>,
    pub weights: WeightsDoc,
    pub projects: Vec<ScoredProject>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct WeightsDoc {
    pub buckets: HashMap<String, f64>,
    pub signals: HashMap<String, HashMap<String, f64>>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct ScoredProject {
    pub slug: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buckets: Option<HashMap<String, f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signals: Option<HashMap<String, SignalScore>>,
    // AIDEV-NOTE: display-only raw star count, not a scoring signal (stars_log scores it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stars: Option<u64>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SignalScore {
    pub raw: f64,
    /// 0..1 after transform/normalization.
    pub score: f64,
    /// Weight within its bucket.
    pub weight: f64,
}

pub fn run(cfg: &Config, category: &str) -> anyhow::Result<()> {
    let cat = cfg.category(category)?;
    let records = load_records(&cat.data_dir.join("raw"))?;
    let scored = score_category(cfg, category, records);
    let out = cat.data_dir.join("scores.json");
    std::fs::write(&out, serde_json::to_string_pretty(&scored)?)?;
    append_history(&cat.data_dir, &scored)?;
    let ranked = scored.projects.iter().filter(|p| p.total.is_some()).count();
    println!(
        "wrote {} ({} scored) to {}",
        scored.projects.len(),
        ranked,
        out.display()
    );
    Ok(())
}

// AIDEV-NOTE: one JSON line per scoring day — {date, scores: {slug: total}}.
// Append keeps file growth linear and diff-friendly; today's entry is replaced
// in place (idempotent re-runs don't duplicate). Powers the hub's top-movers.
fn append_history(data_dir: &Path, scored: &ScoredCategory) -> anyhow::Result<()> {
    let path = data_dir.join("history.json");
    let today = scored.generated_at.format("%Y-%m-%d").to_string();
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect();
    let entry = serde_json::json!({
        "date": today,
        "scores": scored.projects.iter().filter_map(|p| p.total.map(|t| (p.slug.as_str(), (t * 1000.0).round() / 1000.0))).collect::<std::collections::BTreeMap<_,_>>(),
    });
    let line = serde_json::to_string(&entry)?;
    if let Some(pos) = lines.iter().position(|l| {
        serde_json::from_str::<serde_json::Value>(l)
            .ok()
            .and_then(|v| v["date"].as_str().map(|d| d == today))
            .unwrap_or(false)
    }) {
        lines[pos] = line;
    } else {
        lines.push(line);
    }
    std::fs::write(&path, lines.join("\n") + "\n")?;
    Ok(())
}

fn load_records(dir: &Path) -> anyhow::Result<Vec<RepoRecord>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("read {}: {e} (run `collect` first)", dir.display()))?
    {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "json") {
            out.push(serde_json::from_str::<RepoRecord>(
                &std::fs::read_to_string(&path)?,
            )?);
        }
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
}

/// Every signal as (name, bucket, raw value 0..1 pre-normalization where applicable).
struct Signals {
    /// (bucket, name, raw, score, needs category min-max)
    /// `score` starts equal to `raw`; pass 2 overwrites it for normalized signals.
    values: Vec<(&'static str, &'static str, f64, f64, bool)>,
}

fn signals_of(s: &crate::models::RepoSignals, now: DateTime<Utc>, decay_days: f64) -> Signals {
    let days_since_commit = s
        .last_commit_at
        .map(|d| (now - d).num_seconds() as f64 / 86400.0)
        .unwrap_or(f64::INFINITY);
    let commit_recency = (-days_since_commit / decay_days).exp();

    let issue_speed = s
        .median_issue_close_days
        .map(|d| 14.0 / (14.0 + d)) // saturating inverse: 14d ref → 0.5
        .unwrap_or(0.0);
    let pr_speed = s
        .median_pr_close_days
        .map(|d| 3.0 / (3.0 + d))
        .unwrap_or(0.0);
    let backlog = if s.open_issues + u64::from(s.closed_issues_sampled) > 0 {
        f64::from(s.closed_issues_sampled)
            / (s.open_issues + u64::from(s.closed_issues_sampled)) as f64
    } else {
        0.5 // no issues at all: neutral, not punitive
    };

    let age_years = s
        .top_author
        .as_ref()
        .and_then(|a| a.account_created_at)
        .map(|d| (now - d).num_days() as f64 / 365.25)
        .unwrap_or(0.0);
    let author_stars = s
        .top_author
        .as_ref()
        .map(|a| (a.other_repos_star_sum as f64 + 1.0).log10())
        .unwrap_or(0.0);
    // AIDEV-TODO: blend age into track record? For now stars-only, age kept as raw data.
    let _ = age_years;

    Signals {
        values: vec![
            (
                "maintenance",
                "days_since_last_commit",
                commit_recency,
                commit_recency,
                false,
            ),
            (
                "maintenance",
                "commit_frequency_90d",
                (f64::from(s.commits_90d) + 1.0).ln(),
                (f64::from(s.commits_90d) + 1.0).ln(),
                true,
            ),
            (
                "maintenance",
                "release_cadence",
                f64::from(s.releases_365d),
                f64::from(s.releases_365d),
                true,
            ),
            (
                "maintenance",
                "median_issue_close_days",
                issue_speed,
                issue_speed,
                false,
            ),
            (
                "maintenance",
                "median_pr_close_days",
                pr_speed,
                pr_speed,
                false,
            ),
            (
                "maintenance",
                "open_closed_issue_ratio",
                backlog,
                backlog,
                false,
            ),
            (
                "community",
                "stars_log",
                (s.stars as f64 + 1.0).log10(),
                (s.stars as f64 + 1.0).log10(),
                true,
            ),
            (
                "community",
                "forks_log",
                (s.forks as f64 + 1.0).log10(),
                (s.forks as f64 + 1.0).log10(),
                true,
            ),
            (
                "community",
                "contributors_12mo",
                (f64::from(s.contributors_12mo) + 1.0).ln(),
                (f64::from(s.contributors_12mo) + 1.0).ln(),
                true,
            ),
            (
                "community",
                "bus_factor",
                1.0 - s.top_author_commit_share,
                1.0 - s.top_author_commit_share,
                false,
            ),
            (
                "community",
                "author_track_record",
                author_stars,
                author_stars,
                true,
            ),
            (
                "quality",
                "ci_configured",
                s.has_ci as u8 as f64,
                s.has_ci as u8 as f64,
                false,
            ),
            (
                "quality",
                "tests_present",
                s.has_tests as u8 as f64,
                s.has_tests as u8 as f64,
                false,
            ),
            (
                "quality",
                "readme_present",
                (s.readme_len as f64 / 2000.0).min(1.0),
                (s.readme_len as f64 / 2000.0).min(1.0),
                false,
            ),
            // ponytail: any recognized SPDX id counts as OSI; NOASSERTION/none = 0.
            // A real OSI-list check only matters if gaming shows up.
            (
                "quality",
                "license_osi",
                license_score(s.license_spdx.as_deref()),
                license_score(s.license_spdx.as_deref()),
                false,
            ),
        ],
    }
}

fn license_score(spdx: Option<&str>) -> f64 {
    match spdx {
        Some(id) if id != "NOASSERTION" => 1.0,
        _ => 0.0,
    }
}

/// Min-max normalize a column; constant columns score 0.5 (carries no signal).
fn normalize(values: &mut [f64]) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in values.iter() {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    for v in values.iter_mut() {
        *v = if hi > lo { (*v - lo) / (hi - lo) } else { 0.5 };
    }
}

pub fn score_category(cfg: &Config, category: &str, records: Vec<RepoRecord>) -> ScoredCategory {
    let now = Utc::now();
    let decay_days = 90.0; // AIDEV-TODO: move to config when curves are tuned (task 7.2)

    // Pass 1: raw signal values per project.
    let ok: Vec<&RepoRecord> = records.iter().filter(|r| r.status == Status::Ok).collect();
    let mut per_project: Vec<Signals> = ok
        .iter()
        .map(|r| {
            signals_of(
                r.data.as_ref().expect("ok record has data"),
                now,
                decay_days,
            )
        })
        .collect();

    // Pass 2: min-max the "normalized" family across the category.
    let n_signals = per_project.first().map(|s| s.values.len()).unwrap_or(0);
    for i in 0..n_signals {
        if per_project[0].values[i].4 {
            let mut col: Vec<f64> = per_project.iter().map(|s| s.values[i].2).collect();
            normalize(&mut col);
            for (s, v) in per_project.iter_mut().zip(col) {
                s.values[i].3 = v;
            }
        }
    }

    // Pass 3: weighted buckets + total.
    let bucket_weights = [
        ("maintenance", cfg.buckets.maintenance, &cfg.maintenance),
        ("community", cfg.buckets.community, &cfg.community),
        ("quality", cfg.buckets.quality, &cfg.quality),
    ];

    let mut projects: Vec<ScoredProject> = Vec::new();
    for (rec, sig) in records
        .iter()
        .filter(|r| r.status == Status::Ok)
        .zip(&per_project)
    {
        let archived = rec.data.as_ref().expect("ok").archived;
        let issues_answered = rec
            .data
            .as_ref()
            .expect("ok")
            .median_issue_close_days
            .is_some();

        let mut signals_map = HashMap::new();
        let mut buckets = HashMap::new();
        for (bname, _bw, weights) in &bucket_weights {
            let mut sum = 0.0;
            let mut wsum = 0.0;
            for (bucket, name, raw, score, _) in &sig.values {
                if bucket != bname {
                    continue;
                }
                let w = weights.get(*name).copied().unwrap_or(0.0);
                sum += w * score;
                wsum += w;
                signals_map.insert(
                    name.to_string(),
                    SignalScore {
                        raw: *raw,
                        score: *score,
                        weight: w,
                    },
                );
            }
            // AIDEV-NOTE: tolerate mis-summed config weights by normalizing by wsum.
            let mut bucket_score = if wsum > 0.0 { 100.0 * sum / wsum } else { 0.0 };
            if *bname == "maintenance" {
                if archived {
                    bucket_score *= cfg.penalties.archived_multiplier;
                }
                if issues_answered {
                    bucket_score = bucket_score.max(cfg.penalties.maintenance_floor);
                }
            }
            buckets.insert(bname.to_string(), (bucket_score * 10.0).round() / 10.0);
        }

        let total = cfg.buckets.maintenance * buckets["maintenance"]
            + cfg.buckets.community * buckets["community"]
            + cfg.buckets.quality * buckets["quality"];
        projects.push(ScoredProject {
            slug: rec.slug.clone(),
            status: rec.status,
            total: Some((total * 10.0).round() / 10.0),
            buckets: Some(buckets),
            signals: Some(signals_map),
            stars: Some(rec.data.as_ref().expect("ok").stars),
        });
    }
    // Unavailable repos: listed, unscored (spec: shown, excluded from ranking).
    for rec in records.iter().filter(|r| r.status != Status::Ok) {
        projects.push(ScoredProject {
            slug: rec.slug.clone(),
            status: rec.status,
            total: None,
            buckets: None,
            signals: None,
            stars: None,
        });
    }
    projects.sort_by(|a, b| {
        b.total
            .unwrap_or(f64::NEG_INFINITY)
            .partial_cmp(&a.total.unwrap_or(f64::NEG_INFINITY))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    ScoredCategory {
        category: category.to_string(),
        generated_at: now,
        weights: WeightsDoc {
            buckets: [
                ("maintenance".into(), cfg.buckets.maintenance),
                ("community".into(), cfg.buckets.community),
                ("quality".into(), cfg.buckets.quality),
            ]
            .into_iter()
            .collect(),
            signals: [
                ("maintenance".into(), cfg.maintenance.clone()),
                ("community".into(), cfg.community.clone()),
                ("quality".into(), cfg.quality.clone()),
            ]
            .into_iter()
            .collect(),
        },
        projects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn cfg() -> Config {
        Config::load("config/weights.toml".into()).expect("weights.toml parses")
    }

    fn signals() -> RepoSignals {
        RepoSignals {
            archived: false,
            stars: 1000,
            forks: 100,
            description: Some("x".into()),
            license_spdx: Some("MIT".into()),
            last_commit_at: Some(Utc::now()),
            commits_90d: 30,
            releases_365d: 4,
            median_issue_close_days: Some(5.0),
            median_pr_close_days: Some(1.0),
            open_issues: 10,
            closed_issues_sampled: 50,
            contributors_12mo: 12,
            top_author_commit_share: 0.4,
            top_author: Some(AuthorInfo {
                login: "alice".into(),
                account_created_at: Some(Utc::now() - chrono::Duration::days(3650)),
                other_repos_star_sum: 5000,
            }),
            has_ci: true,
            has_tests: true,
            readme_len: 5000,
        }
    }

    fn record(slug: &str, archived: bool, s: RepoSignals) -> RepoRecord {
        let mut s = s;
        s.archived = archived;
        RepoRecord {
            slug: slug.into(),
            fetched_at: Utc::now(),
            status: Status::Ok,
            data: Some(s),
        }
    }

    #[test]
    fn healthy_project_scores_high() {
        let scored = score_category(&cfg(), "test", vec![record("a/healthy", false, signals())]);
        let p = &scored.projects[0];
        assert!(p.total.unwrap() > 70.0, "total = {:?}", p.total);
    }

    #[test]
    fn archived_penalized_not_zeroed() {
        let s = signals();
        let active = score_category(&cfg(), "t", vec![record("a/x", false, signals())]);
        let arch = score_category(&cfg(), "t", vec![record("a/x", true, s)]);
        let (a, b) = (
            active.projects[0].total.unwrap(),
            arch.projects[0].total.unwrap(),
        );
        assert!(b < a, "archived {b} should score below active {a}");
        assert!(b > 0.0, "archived must not be zeroed: {b}");
        // Issues still answered → maintenance floor applies.
        let m = arch.projects[0].buckets.as_ref().unwrap()["maintenance"];
        assert!(m >= 15.0, "maintenance floor: {m}");
    }

    #[test]
    fn no_license_zeroes_license_signal() {
        let mut s = signals();
        s.license_spdx = None;
        let scored = score_category(&cfg(), "t", vec![record("a/x", false, s)]);
        let sig = &scored.projects[0].signals.as_ref().unwrap()["license_osi"];
        assert_eq!(sig.score, 0.0);
    }

    #[test]
    fn unavailable_repo_unscored() {
        let missing = RepoRecord {
            slug: "gone/repo".into(),
            fetched_at: Utc::now(),
            status: Status::NotFound,
            data: None,
        };
        let scored = score_category(&cfg(), "t", vec![record("a/x", false, signals()), missing]);
        let gone = scored
            .projects
            .iter()
            .find(|p| p.slug == "gone/repo")
            .unwrap();
        assert!(gone.total.is_none());
    }

    #[test]
    fn category_relative_stars() {
        // Same 1000-star repo: top of a small category, bottom of a huge one.
        let stars_of = |scored: &ScoredCategory| {
            scored
                .projects
                .iter()
                .find(|p| p.slug == "a/mid")
                .unwrap()
                .signals
                .as_ref()
                .unwrap()["stars_log"]
                .score
        };
        let small_cat = score_category(
            &cfg(),
            "t",
            vec![
                record("a/mid", false, signals()),
                record(
                    "a/tiny",
                    false,
                    RepoSignals {
                        stars: 10,
                        ..signals()
                    },
                ),
            ],
        );
        let huge_cat = score_category(
            &cfg(),
            "t",
            vec![
                record("a/mid", false, signals()),
                record(
                    "a/huge",
                    false,
                    RepoSignals {
                        stars: 100_000,
                        ..signals()
                    },
                ),
            ],
        );
        let (a, b) = (stars_of(&small_cat), stars_of(&huge_cat));
        assert!(a > b, "small-cat {a} should beat huge-cat {b}");
        assert_eq!(a, 1.0);
        assert_eq!(b, 0.0);
    }

    #[test]
    fn scored_project_without_stars_deserializes_none() {
        let json = r#"{"slug":"a/b","status":"ok","total":50.0}"#;
        let p: ScoredProject = serde_json::from_str(json).unwrap();
        assert_eq!(p.stars, None);
    }

    #[test]
    fn fixture_stars_carried_through_scoring() {
        let rec: RepoRecord = serde_json::from_str(include_str!(
            "../tests/fixtures/nvim-telescope__telescope.nvim.json"
        ))
        .unwrap();
        let scored = score_category(&cfg(), "t", vec![rec]);
        assert_eq!(scored.projects[0].stars, Some(19709));
    }

    #[test]
    fn constant_column_is_neutral() {
        let mut v = vec![3.0, 3.0, 3.0];
        normalize(&mut v);
        assert_eq!(v, vec![0.5, 0.5, 0.5]);
    }
}
