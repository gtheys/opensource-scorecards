use scorecards::{collect, config, render, score, seed};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "scorecards", about = "Open-source project health scorecards")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Parse an awesome-list into a canonical repo list
    Seed {
        /// Path or URL to the awesome-list markdown (optional if config sets seed_source)
        source: Option<String>,
        /// Category key from config (e.g. neovim)
        #[arg(short, long, default_value = "neovim")]
        category: String,
    },
    /// Fetch GitHub signals into the raw cache
    Collect {
        #[arg(short, long, default_value = "neovim")]
        category: String,
        /// Ignore cache freshness, refetch everything
        #[arg(long)]
        force: bool,
    },
    /// Compute scores from the raw cache
    Score {
        #[arg(short, long, default_value = "neovim")]
        category: String,
    },
    /// Render the static site
    Render {
        #[arg(short, long, default_value = "neovim")]
        category: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg = config::Config::load(PathBuf::from("config/weights.toml"))?;
    match cli.cmd {
        Cmd::Seed { source, category } => seed::run(&cfg, source.as_deref(), &category).await,
        Cmd::Collect { category, force } => collect::run(&cfg, &category, force).await,
        Cmd::Score { category } => score::run(&cfg, &category),
        Cmd::Render { category } => render::run(&cfg, &category),
    }
}
