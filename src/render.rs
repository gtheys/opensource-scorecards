use crate::config::Config;
use crate::score::ScoredCategory;
use std::path::Path;
use tera::{Context, Tera};

pub fn run(cfg: &Config, category: &str) -> anyhow::Result<()> {
    let cat = cfg.category(category)?;
    let scored: ScoredCategory =
        serde_json::from_str(&std::fs::read_to_string(cat.data_dir.join("scores.json"))?)?;
    render_to(cfg, &scored, Path::new("site"), &cat.name)
}

fn templates() -> anyhow::Result<Tera> {
    let mut tera = Tera::new();
    tera.load_from_glob("templates/**/*.html")?;
    Ok(tera)
}

pub fn render_to(
    _cfg: &Config,
    scored: &ScoredCategory,
    out_dir: &Path,
    category_name: &str,
) -> anyhow::Result<()> {
    let tera = templates()?;
    std::fs::create_dir_all(out_dir.join("projects"))?;

    let ranked: Vec<_> = scored.projects.iter().filter(|p| p.total.is_some()).collect();
    let unavailable: Vec<_> = scored.projects.iter().filter(|p| p.total.is_none()).collect();

    // Leaderboard
    let mut ctx = Context::new();
    ctx.insert("category_name", category_name);
    ctx.insert("generated_at", &scored.generated_at.to_rfc3339());
    ctx.insert("ranked", &ranked);
    ctx.insert("unavailable", &unavailable);
    write(&tera, "leaderboard.html", &ctx, &out_dir.join("index.html"))?;

    // Per-project breakdown
    for p in &scored.projects {
        let mut ctx = Context::new();
        ctx.insert("p", p);
        ctx.insert("weights", &scored.weights);
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
    write(&tera, "methodology.html", &ctx, &out_dir.join("methodology.html"))?;

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
        render_to(&cfg, &scored, &dir, "Neovim Plugins").unwrap();

        let index = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(index.contains("Neovim Plugins"));
        assert!(index.contains("not comparable across categories"));
        assert!(index.contains("nvim-telescope/telescope.nvim"));

        let project = std::fs::read_to_string(
            dir.join("projects/nvim-telescope__telescope.nvim.html"),
        )
        .unwrap();
        assert!(project.contains("Total score:"));
        assert!(project.contains("Signal breakdown"));
        assert!(project.contains("license_osi"));

        let meth = std::fs::read_to_string(dir.join("methodology.html")).unwrap();
        assert!(meth.contains("0.4")); // maintenance bucket weight from config
        assert!(meth.contains("days_since_last_commit"));
    }
}
