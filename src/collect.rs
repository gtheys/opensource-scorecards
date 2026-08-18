use crate::config::Config;
use crate::models::*;
use anyhow::Context as _;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::path::PathBuf;

// ponytail: ~11 API calls/repo. Full neovim run (1393 repos) exceeds one 5k/hr
// window; cache makes runs resumable — re-run until complete. GraphQL batching
// is the documented v2 upgrade if this hurts.
const MIN_BUDGET: usize = 15;
// ponytail: commit history capped at 10 pages (1000 commits) per repo; bus
// factor/contributor counts saturate for very active repos. Raise if needed.
const MAX_COMMIT_PAGES: u8 = 10;

pub async fn run(cfg: &Config, category: &str, force: bool) -> anyhow::Result<()> {
    let cat = cfg.category(category)?;
    let repos: Vec<String> = load_repo_list(&cat.data_dir.join("repos.yaml"))?;
    let raw_dir = cat.data_dir.join("raw");
    std::fs::create_dir_all(&raw_dir)?;

    let token = std::env::var("GITHUB_TOKEN")
        .context("GITHUB_TOKEN not set (needed for 5k req/hr budget)")?;
    let crab = octocrab::Octocrab::builder()
        .personal_token(token)
        .build()?;

    let bots = BotFilter::new(&cfg.bot_logins);
    let stale_after = Duration::hours(cfg.staleness_hours);
    let mut done = 0usize;
    let mut skipped = 0usize;

    for slug in &repos {
        let (owner, repo) = slug
            .split_once('/')
            .ok_or_else(|| anyhow::anyhow!("bad slug '{slug}'"))?;
        let path = cache_path(&raw_dir, slug);

        if !force && is_fresh(&path, stale_after) {
            skipped += 1;
            continue;
        }

        // Budget guard before spending calls on this repo.
        let rl = crab.ratelimit().get().await?;
        if rl.rate.remaining < MIN_BUDGET {
            println!(
                "rate budget low ({} left, reset {}); stopping. {done} fetched, {skipped} cached. Re-run later — cache resumes.",
                rl.rate.remaining, rl.rate.reset
            );
            return Ok(());
        }

        let record = match fetch_repo(&crab, owner, repo, &bots).await {
            Ok(record) => record,
            Err(e) => {
                eprintln!("warn: {slug}: {e:#}; keeping previous cache if any");
                continue;
            }
        };
        std::fs::write(&path, serde_json::to_string_pretty(&record)?)?;
        done += 1;
        if done.is_multiple_of(25) {
            println!("{done} fetched, {skipped} cached…");
        }
    }
    println!("collect done: {done} fetched, {skipped} fresh-cached, {} total", repos.len());
    Ok(())
}

fn load_repo_list(path: &std::path::Path) -> anyhow::Result<Vec<String>> {
    #[derive(serde::Deserialize)]
    struct List {
        repos: Vec<String>,
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read {} (run `seed` first)", path.display()))?;
    Ok(serde_yaml::from_str::<List>(&text)?.repos)
}

fn is_fresh(path: &std::path::Path, stale_after: Duration) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else { return false };
    let Ok(rec) = serde_json::from_str::<RepoRecord>(&text) else { return false };
    Utc::now() - rec.fetched_at < stale_after
}

fn cache_path(dir: &std::path::Path, slug: &str) -> PathBuf {
    dir.join(format!("{}.json", slug.replace('/', "__")))
}

async fn fetch_repo(
    crab: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    bots: &BotFilter,
) -> anyhow::Result<RepoRecord> {
    let now = Utc::now();
    let slug = format!("{owner}/{repo}");
    let rh = crab.repos(owner, repo);

    let meta = match rh.get().await {
        Ok(m) => m,
        Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 404 => {
            return Ok(RepoRecord { slug, fetched_at: now, status: Status::NotFound, data: None });
        }
        Err(octocrab::Error::GitHub { source, .. })
            if source.status_code == 301 || source.status_code == 302 =>
        {
            return Ok(RepoRecord { slug, fetched_at: now, status: Status::Redirected, data: None });
        }
        Err(e) => return Err(e.into()),
    };

    // One commit-history fetch serves: last commit, 90d count, 12mo contributors, bus factor.
    let commits = fetch_commits(crab, owner, repo, now - Duration::days(365)).await?;
    let human: Vec<_> = commits.iter().filter(|c| !bots.is_bot(c.author.as_deref())).collect();
    let cutoff_90d = now - Duration::days(90);
    let commits_90d = human.iter().filter(|c| c.date > cutoff_90d).count() as u32;
    let last_commit_at = human.first().map(|c| c.date);
    let (contributors_12mo, top_share, top_login) = author_stats(&human);

    let releases_365d = count_releases(crab, owner, repo, now - Duration::days(365)).await?;

    // Closed issues+PRs sample (issues API mixes both; split by `pull_request` field).
    let (issue_close, pr_close, closed_issues) = fetch_closed_items(crab, owner, repo).await?;

    let open_issues = search_count(crab, &format!("repo:{owner}/{repo} is:issue is:open")).await?;
    let open_prs = search_count(crab, &format!("repo:{owner}/{repo} is:pr is:open")).await?;

    let has_ci = has_workflows(crab, owner, repo).await?;
    let has_tests = has_test_files(crab, owner, repo).await?;
    let readme_len = readme_len(crab, owner, repo).await;

    let top_author = match top_login {
        Some(login) => Some(author_info(crab, &login).await?),
        None => None,
    };

    Ok(RepoRecord {
        slug,
        fetched_at: now,
        status: Status::Ok,
        data: Some(RepoSignals {
            archived: meta.archived.unwrap_or(false),
            stars: meta.stargazers_count.unwrap_or(0).into(),
            forks: meta.forks_count.unwrap_or(0).into(),
            description: meta.description,
            license_spdx: meta.license.map(|l| l.spdx_id),
            last_commit_at,
            commits_90d,
            releases_365d,
            median_issue_close_days: issue_close,
            median_pr_close_days: pr_close,
            open_issues: open_issues + open_prs,
            closed_issues_sampled: closed_issues,
            contributors_12mo,
            top_author_commit_share: top_share,
            top_author,
            has_ci,
            has_tests,
            readme_len,
        }),
    })
}

struct CommitInfo {
    date: DateTime<Utc>,
    author: Option<String>,
}

async fn fetch_commits(
    crab: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    since: DateTime<Utc>,
) -> anyhow::Result<Vec<CommitInfo>> {
    let mut out = Vec::new();
    let mut page = crab
        .repos(owner, repo)
        .list_commits()
        .since(since)
        .per_page(100)
        .send()
        .await?;
    let mut pages = 1;
    loop {
        out.extend(page.items.iter().map(|c| CommitInfo {
            date: c.commit.committer.as_ref().and_then(|x| x.date).unwrap_or_else(Utc::now),
            author: c.author.as_ref().map(|a| a.login.clone()),
        }));
        if pages >= MAX_COMMIT_PAGES {
            break;
        }
        match crab.get_page(&page.next).await? {
            Some(next) => {
                page = next;
                pages += 1;
            }
            None => break,
        }
    }
    // Newest first (API order, but sort defensively).
    out.sort_by_key(|c| std::cmp::Reverse(c.date));
    Ok(out)
}

/// Distinct authors + top-author share over the sampled commits.
fn author_stats(commits: &[&CommitInfo]) -> (u32, f64, Option<String>) {
    let mut by_author: HashMap<&str, u32> = HashMap::new();
    for c in commits {
        if let Some(a) = c.author.as_deref() {
            *by_author.entry(a).or_default() += 1;
        }
    }
    let total: u32 = by_author.values().sum();
    let top = by_author.iter().max_by_key(|(_, n)| *n);
    match top {
        Some((login, n)) if total > 0 => (
            by_author.len() as u32,
            *n as f64 / total as f64,
            Some(login.to_string()),
        ),
        _ => (by_author.len() as u32, 0.0, None),
    }
}

async fn count_releases(
    crab: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    since: DateTime<Utc>,
) -> anyhow::Result<u32> {
    let page = crab
        .repos(owner, repo)
        .releases()
        .list()
        .per_page(100)
        .send()
        .await?;
    Ok(page
        .items
        .iter()
        .filter(|r| r.published_at.is_some_and(|d| d > since))
        .count() as u32)
}

/// Median close-days for the last 50 closed issues and PRs (bot-filtered).
async fn fetch_closed_items(
    crab: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
) -> anyhow::Result<(Option<f64>, Option<f64>, u32)> {
    let page = crab
        .issues(owner, repo)
        .list()
        .state(octocrab::params::State::Closed)
        .sort(octocrab::params::issues::Sort::Updated)
        .direction(octocrab::params::Direction::Descending)
        .per_page(50)
        .send()
        .await?;
    let mut issues = Vec::new();
    let mut prs = Vec::new();
    for it in &page.items {
        let Some(closed) = it.closed_at else { continue };
        let days = (closed - it.created_at).num_seconds() as f64 / 86400.0;
        if it.pull_request.is_some() {
            prs.push(days);
        } else {
            issues.push(days);
        }
    }
    let n_issues = issues.len() as u32;
    Ok((median(&mut issues), median(&mut prs), n_issues))
}

async fn search_count(crab: &octocrab::Octocrab, q: &str) -> anyhow::Result<u64> {
    let res = crab
        .search()
        .issues_and_pull_requests(q)
        .per_page(1)
        .send()
        .await?;
    Ok(res.total_count.unwrap_or(0))
}

async fn has_workflows(crab: &octocrab::Octocrab, owner: &str, repo: &str) -> anyhow::Result<bool> {
    // Empty .github/workflows (or missing dir) => 404/empty page, both mean "no CI".
    let page = crab.workflows(owner, repo).list().per_page(1).send().await;
    Ok(page.is_ok_and(|p| !p.items.is_empty()))
}

/// Root-listing heuristic: test dirs or obvious test files at repo root,
/// plus one level into `lua/` (standard neovim plugin layout).
// ponytail: two listings max; deeper nesting ignored. Walk the tree
// recursively only if false negatives show up in calibration.
async fn has_test_files(crab: &octocrab::Octocrab, owner: &str, repo: &str) -> anyhow::Result<bool> {
    let items = match crab.repos(owner, repo).get_content().path("").send().await {
        Ok(items) => items,
        Err(_) => return Ok(false),
    };
    let looks_testy = |n: &str| {
        let n = n.to_ascii_lowercase();
        n == "test"
            || n == "tests"
            || n == "spec"
            || n.starts_with("test_")
            || n.ends_with("_test.go")
            || n.ends_with(".test.ts")
    };
    if items.items.iter().any(|c| looks_testy(&c.name)) {
        return Ok(true);
    }
    let has_lua = items.items.iter().any(|c| c.name == "lua" && c.r#type == "dir");
    if has_lua
        && let Ok(lua) = crab.repos(owner, repo).get_content().path("lua").send().await
    {
        return Ok(lua.items.iter().any(|c| looks_testy(&c.name)));
    }
    Ok(false)
}

async fn readme_len(crab: &octocrab::Octocrab, owner: &str, repo: &str) -> usize {
    let Ok(readme) = crab.repos(owner, repo).get_readme().send().await else { return 0 };
    readme
        .content
        .and_then(|b64| {
            base64::engine::general_purpose::STANDARD
                .decode(b64.replace('\n', ""))
                .ok()
        })
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

async fn author_info(crab: &octocrab::Octocrab, login: &str) -> anyhow::Result<AuthorInfo> {
    let user = crab.users(login).profile().await?;
    // ponytail: first 100 owned repos only; prolific authors undercount. Fine for
    // a log-scaled signal.
    let repos: Vec<octocrab::models::Repository> = crab
        .get(
            format!("/users/{login}/repos?per_page=100&type=owner"),
            None::<&()>,
        )
        .await
        .unwrap_or_default();
    let star_sum = repos.iter().filter_map(|r| r.stargazers_count).map(u64::from).sum();
    Ok(AuthorInfo {
        login: login.to_string(),
        account_created_at: Some(user.created_at),
        other_repos_star_sum: star_sum,
    })
}

/// Bot detection: `[bot]` suffix plus config-listed logins. Applied at collection
/// time so cached data is already clean (design D7).
pub struct BotFilter {
    listed: Vec<String>,
}

impl BotFilter {
    pub fn new(listed: &[String]) -> Self {
        Self { listed: listed.iter().map(|s| s.to_lowercase()).collect() }
    }
    pub fn is_bot(&self, login: Option<&str>) -> bool {
        let Some(l) = login else { return false };
        let l = l.to_lowercase();
        l.ends_with("[bot]") || self.listed.contains(&l)
    }
}

pub fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    if n % 2 == 1 {
        Some(values[n / 2])
    } else {
        Some((values[n / 2 - 1] + values[n / 2]) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bot_filter_suffix_and_list() {
        let f = BotFilter::new(&["renovate".to_string()]);
        assert!(f.is_bot(Some("dependabot[bot]")));
        assert!(f.is_bot(Some("Renovate")));
        assert!(!f.is_bot(Some("alice")));
        assert!(!f.is_bot(None));
    }

    #[test]
    fn median_odd_even_empty() {
        assert_eq!(median(&mut vec![]), None);
        assert_eq!(median(&mut vec![3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&mut vec![4.0, 1.0, 3.0, 2.0]), Some(2.5));
    }

    #[test]
    fn author_stats_share() {
        let d = Utc::now();
        let c = |a: &str| CommitInfo { date: d, author: Some(a.into()) };
        let commits = vec![c("alice"), c("alice"), c("alice"), c("bob")];
        let refs: Vec<&CommitInfo> = commits.iter().collect();
        let (distinct, share, top) = author_stats(&refs);
        assert_eq!(distinct, 2);
        assert!((share - 0.75).abs() < 1e-9);
        assert_eq!(top.as_deref(), Some("alice"));
    }

    #[test]
    fn cache_path_uses_double_underscore() {
        assert_eq!(
            cache_path(std::path::Path::new("/x"), "o/r"),
            std::path::Path::new("/x/o__r.json")
        );
    }
}
