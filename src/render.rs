use crate::config::Config;
use crate::score::ScoredCategory;
use std::path::Path;
use tera::{Context, Tera};

pub fn run(cfg: &Config, category: &str) -> anyhow::Result<()> {
    let scored: ScoredCategory =
        serde_json::from_str(&std::fs::read_to_string(
            cfg.category(category)?.data_dir.join("scores.json"),
        )?)?;
    render_to(cfg, &scored, Path::new("site"), category)
}

fn templates() -> anyhow::Result<Tera> {
    let mut tera = Tera::new();
    tera.load_from_glob("templates/**/*.html")?;
    tera.register_filter("stars_fmt", stars_fmt);
    Ok(tera)
}

/// Formats star counts for display: ≥1000 → `{:.1}k`, <1000 → digits, null → em dash.
fn stars_fmt(value: &tera::Value, _kwargs: tera::Kwargs, _state: &tera::State) -> tera::TeraResult<tera::Value> {
    let out = match value.as_u64() {
        Some(n) if n >= 1000 => format!("{:.1}k", n as f64 / 1000.0),
        Some(n) => n.to_string(),
        None => "—".to_string(),
    };
    Ok(tera::Value::from(out))
}

/// Renders one category. Leaderboard goes to `<key>.html`; every run also
/// rewrites the shared `index.html` hub and `methodology.html` (idempotent).
pub fn render_to(
    cfg: &Config,
    scored: &ScoredCategory,
    out_dir: &Path,
    category_key: &str,
) -> anyhow::Result<()> {
    let tera = templates()?;
    std::fs::create_dir_all(out_dir.join("projects"))?;
    // AIDEV-NOTE: projects/ is a shared namespace across categories — a repo in
    // two lists gets one page (last render wins). Lua/TS overlap ~zero; revisit if real.

    let mut categories: Vec<_> = cfg
        .categories
        .iter()
        .map(|(key, cat)| serde_json::json!({"key": key, "name": cat.name}))
        .collect();
    categories.sort_by(|a, b| a["key"].as_str().cmp(&b["key"].as_str()));
    let category_name = &cfg.category(category_key)?.name;

    let ranked: Vec<_> = scored.projects.iter().filter(|p| p.total.is_some()).collect();
    let unavailable: Vec<_> = scored.projects.iter().filter(|p| p.total.is_none()).collect();

    // Leaderboard (per-category page, root="" for top-level links)
    let mut ctx = Context::new();
    ctx.insert("category_name", category_name);
    ctx.insert("generated_at", &scored.generated_at.to_rfc3339());
    ctx.insert("ranked", &ranked);
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
        let mut rows: Vec<_> = p
            .signals
            .as_ref()
            .map(|m| {
                let mut v: Vec<_> = m
                    .iter()
                    .map(|(name, s)| {
                        serde_json::json!({
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

    // Methodology (from the same weights the scorer used → always in sync)
    let mut ctx = Context::new();
    ctx.insert("weights", &scored.weights);
    ctx.insert("categories", &categories);
    ctx.insert("root", "");
    write(&tera, "methodology.html", &ctx, &out_dir.join("methodology.html"))?;

    // Category hub (rewritten by every render run; idempotent)
    let mut ctx = Context::new();
    ctx.insert("categories", &categories);
    ctx.insert("root", "");
    write(&tera, "categories.html", &ctx, &out_dir.join("index.html"))?;

    println!(
        "rendered {} projects ({} ranked) to {}",
        scored.projects.len(),
        ranked.len(),
        out_dir.display()
    );
    Ok(())
}

fn write(tera: &Tera, template: &str, ctx: &Context, path: &Path) -> anyhow::Result<()> {
    let html = tera.render(template, ctx)?;
    std::fs::write(path, html)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::score::{score_category, ScoredCategory};

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
        let records: Vec<crate::models::RepoRecord> = ["tests/fixtures/nvim-telescope__telescope.nvim.json"]
            .iter()
            .map(|f| serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap())
            .collect();
        score_category(&cfg, "neovim", records)
    }

    #[test]
    fn renders_all_pages() {
        let cfg = Config::load("config/weights.toml".into()).unwrap();
        let scored = fixture_scores();
        let dir = std::env::temp_dir().join("scorecards-render-test");
        let _ = std::fs::remove_dir_all(&dir);
        // Render both categories like CI does; hub/methodology are rewritten per run.
        render_to(&cfg, &scored, &dir, "neovim").unwrap();
        render_to(&cfg, &scored, &dir, "pi").unwrap();

        for page in ["neovim.html", "pi.html"] {
            let html = std::fs::read_to_string(dir.join(page)).unwrap();
            assert!(html.contains("not comparable across categories"));
            assert!(html.contains("nvim-telescope/telescope.nvim"));
            assert!(html.contains(r#"href="neovim.html""#));
            assert!(html.contains(r#"href="pi.html""#));
        }

        let hub = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(hub.contains(r#"href="neovim.html""#));
        assert!(hub.contains(r#"href="pi.html""#));
        assert!(hub.contains("Neovim Plugins"));
        assert!(hub.contains("Pi Extensions"));

        let project = std::fs::read_to_string(
            dir.join("projects/nvim-telescope__telescope.nvim.html"),
        )
        .unwrap();
        assert!(project.contains("Total score:"));
        assert!(project.contains("Signal breakdown"));
        assert!(project.contains("license_osi"));
        // Project pages live one level deep — nav links must be ../-prefixed.
        assert!(project.contains(r#"href="../pi.html""#));
        assert!(project.contains(r#"href="../methodology.html""#));

        let meth = std::fs::read_to_string(dir.join("methodology.html")).unwrap();
        assert!(meth.contains("0.4")); // maintenance bucket weight from config
        assert!(meth.contains("days_since_last_commit"));
    }
}
