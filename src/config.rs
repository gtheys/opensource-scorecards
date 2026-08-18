use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub buckets: Buckets,
    pub maintenance: HashMap<String, f64>,
    pub community: HashMap<String, f64>,
    pub quality: HashMap<String, f64>,
    #[serde(default = "default_staleness")]
    pub staleness_hours: i64,
    #[serde(default)]
    pub bot_logins: Vec<String>,
    pub categories: HashMap<String, Category>,
    #[serde(default)]
    pub penalties: Penalties,
}

#[derive(Debug, Deserialize)]
pub struct Penalties {
    #[serde(default = "default_archived_mult")]
    pub archived_multiplier: f64,
    #[serde(default = "default_floor")]
    pub maintenance_floor: f64,
}

impl Default for Penalties {
    fn default() -> Self {
        Self {
            archived_multiplier: default_archived_mult(),
            maintenance_floor: default_floor(),
        }
    }
}

fn default_archived_mult() -> f64 {
    0.3
}
fn default_floor() -> f64 {
    15.0
}

#[derive(Debug, Deserialize)]
pub struct Buckets {
    pub maintenance: f64,
    pub community: f64,
    pub quality: f64,
}

#[derive(Debug, Deserialize)]
pub struct Category {
    pub name: String,
    /// Directory holding repos.yaml, raw/, scores.json for this category
    pub data_dir: PathBuf,
    /// Default awesome-list URL for `seed` when no source arg is given.
    #[serde(default)]
    pub seed_source: Option<String>,
}

fn default_staleness() -> i64 {
    20
}

impl Config {
    pub fn load(path: PathBuf) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        Ok(toml::from_str(&text)?)
    }

    pub fn category(&self, key: &str) -> anyhow::Result<&Category> {
        self.categories
            .get(key)
            .ok_or_else(|| anyhow::anyhow!("unknown category '{key}' (check config)"))
    }
}
