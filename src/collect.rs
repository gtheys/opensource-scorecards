use crate::config::Config;
use crate::models::*;
use anyhow::Context as _;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use reqwest::header::{
    ACCEPT, AUTHORIZATION, ETAG, HeaderMap, HeaderName, HeaderValue, IF_NONE_MATCH, LINK,
};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::PathBuf;

// ponytail: quiet repos now cost ~2-4 API calls/repo (conditional 304s are
// free; meta + closed items are always paid). Full neovim run (1393 repos)
// still exceeds one 5k/hr window; cache makes runs resumable — re-run until
// complete. GraphQL batching is the documented v2 upgrade if this hurts.
const MIN_BUDGET: usize = 15;
// ponytail: commit history capped at 10 pages (1000 commits) per repo; bus
// factor/contributor counts saturate for very active repos. Raise if needed.
const MAX_COMMIT_PAGES: u8 = 10;
// AIDEV-NOTE: has_ci/has_tests/readme_len change slowly but cost ~4 calls/repo.
// Carry them over from the previous cache entry until 30 days old (SCR-1NQ7).
// top_author is NOT governed by this TTL: it is coupled to the commits ETag —
// refreshed whenever the commit listing changed (Fresh), kept on a commits 304
// (see fetch_repo). Fast signals (commits, releases, issues, stars) are always
// refetched (conditionally, via ETag).
const SLOW_TTL: Duration = Duration::days(30);

#[derive(Debug, Clone)]
struct SlowSignals {
    has_ci: bool,
    has_tests: bool,
    readme_len: usize,
}

/// Pure TTL decision: previous slow values if still within SLOW_TTL, else None
/// (missing prev, no data, or `slow_fetched_at` None/old => refetch).
fn carry_slow(prev: Option<&RepoRecord>, now: DateTime<Utc>) -> Option<SlowSignals> {
    let prev = prev?;
    let data = prev.data.as_ref()?;
    if now - prev.slow_fetched_at? >= SLOW_TTL {
        return None;
    }
    Some(SlowSignals {
        has_ci: data.has_ci,
        has_tests: data.has_tests,
        readme_len: data.readme_len,
    })
}

pub async fn run(
    cfg: &Config,
    category: &str,
    force: bool,
    limit: Option<usize>,
) -> anyhow::Result<()> {
    let cat = cfg.category(category)?;
    let repos: Vec<String> = load_repo_list(&cat.data_dir.join("repos.yaml"))?;
    let raw_dir = cat.data_dir.join("raw");
    std::fs::create_dir_all(&raw_dir)?;

    let token = std::env::var("GITHUB_TOKEN")
        .context("GITHUB_TOKEN not set (needed for 5k req/hr budget)")?;
    let crab = octocrab::Octocrab::builder()
        .personal_token(token.clone())
        .build()?;

    // AIDEV-NOTE: commits + releases are fetched through this raw reqwest
    // client with ETag conditional requests — octocrab hides response headers,
    // so If-None-Match/ETag are out of its reach. A 304 costs 0 rate-limited
    // calls, which is what makes a full refresh fit into one 5k/hr window.
    let client = reqwest::Client::builder()
        .user_agent("scorecards-collector") // GitHub rejects UA-less clients
        .default_headers({
            let mut headers = HeaderMap::new();
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .context("invalid GITHUB_TOKEN")?,
            );
            headers.insert(
                ACCEPT,
                HeaderValue::from_static("application/vnd.github+json"),
            );
            headers.insert(
                HeaderName::from_static("x-github-api-version"),
                HeaderValue::from_static("2022-11-28"),
            );
            headers
        })
        .build()
        .context("build raw HTTP client")?;

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

        // Best-effort: unreadable/corrupt cache => None. Loaded even when fast-stale,
        // since slow signals have their own TTL. --force skips carry-over so a forced
        // run refetches slow signals too (pre-TTL behavior).
        let prev: Option<RepoRecord> = if force {
            None
        } else {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
        };

        let record = match fetch_repo(&crab, &client, owner, repo, &bots, prev.as_ref()).await {
            Ok(record) => record,
            Err(e) => {
                // AIDEV-NOTE: rate-limit errors (core or search bucket) must stop the
                // run, not skip the repo — continuing just burns the rest of the list.
                if format!("{e:#}").contains("rate limit") {
                    println!(
                        "rate limited at {slug}; stopping. {done} fetched, {skipped} cached. Re-run later — cache resumes."
                    );
                    return Ok(());
                }
                eprintln!("warn: {slug}: {e:#}; keeping previous cache if any");
                continue;
            }
        };
        std::fs::write(&path, serde_json::to_string_pretty(&record)?)?;
        done += 1;
        if limit.is_some_and(|n| done >= n) {
            println!(
                "batch limit reached: {done} fetched this run, {skipped} cached. Re-run for next batch."
            );
            return Ok(());
        }
        if done.is_multiple_of(25) {
            println!("{done} fetched, {skipped} cached…");
        }
    }
    println!(
        "collect done: {done} fetched, {skipped} fresh-cached, {} total",
        repos.len()
    );
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
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(rec) = serde_json::from_str::<RepoRecord>(&text) else {
        return false;
    };
    Utc::now() - rec.fetched_at < stale_after
}

fn cache_path(dir: &std::path::Path, slug: &str) -> PathBuf {
    dir.join(format!("{}.json", slug.replace('/', "__")))
}

async fn fetch_repo(
    crab: &octocrab::Octocrab,
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    bots: &BotFilter,
    prev: Option<&RepoRecord>,
) -> anyhow::Result<RepoRecord> {
    let now = Utc::now();
    let slug = format!("{owner}/{repo}");
    let rh = crab.repos(owner, repo);

    let meta = match rh.get().await {
        Ok(m) => m,
        Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 404 => {
            return Ok(RepoRecord {
                slug,
                fetched_at: now,
                status: Status::NotFound,
                data: None,
                slow_fetched_at: None,
                commits_etag: None,
                releases_etag: None,
            });
        }
        Err(octocrab::Error::GitHub { source, .. })
            if source.status_code == 301 || source.status_code == 302 =>
        {
            return Ok(RepoRecord {
                slug,
                fetched_at: now,
                status: Status::Redirected,
                data: None,
                slow_fetched_at: None,
                commits_etag: None,
                releases_etag: None,
            });
        }
        Err(e) => return Err(e.into()),
    };

    // AIDEV-NOTE: reuse-on-304 — when the commit listing is unchanged (ETag
    // match), ALL commit-derived signals below are reused from the previous
    // cache at 0 paid calls. That couples top_author to the commits ETag, not
    // to slow_fetched_at/SLOW_TTL: on a commits 304 it is kept even when the
    // slow TTL expired; on Fresh it is re-derived, but the AuthorInfo network
    // fetch is skipped while the top login is unchanged (see fetch_repo).
    // carry_slow now governs only has_ci/has_tests/readme_len.
    let window = now - Duration::days(365);
    let (cs, top_author, commits_etag) = match fetch_commits(
        client,
        owner,
        repo,
        prev.and_then(|p| p.commits_etag.as_deref()),
    )
    .await?
    {
        EtagResponse::NotModified => {
            let d = prev
                .and_then(|p| p.data.as_ref())
                .ok_or_else(|| anyhow::anyhow!("commits 304 but previous cache has no data"))?;
            (
                CommitSignals {
                    last_commit_at: d.last_commit_at,
                    commits_90d: d.commits_90d,
                    contributors_12mo: d.contributors_12mo,
                    top_author_commit_share: d.top_author_commit_share,
                },
                d.top_author.clone(),
                prev.and_then(|p| p.commits_etag.clone()),
            )
        }
        EtagResponse::Fresh(commits, etag) => {
            // One commit-history fetch serves: last commit, 90d count, 12mo
            // contributors, bus factor. Sampling window applied client-side.
            let human: Vec<&CommitInfo> = within_window(&commits, window)
                .into_iter()
                .filter(|c| !bots.is_bot(c.author.as_deref()))
                .collect();
            let cutoff_90d = now - Duration::days(90);
            let (contributors_12mo, top_share, top_login) = author_stats(&human);
            // AIDEV-NOTE: reuse the cached author profile when the top login is
            // unchanged — profile data (account age, other-repo star sum) moves
            // slowly, so an actively-committing repo whose top author stayed the
            // same skips the 2 author_info calls most nights.
            let top_author = match top_login {
                Some(login)
                    if prev
                        .and_then(|p| p.data.as_ref())
                        .and_then(|d| d.top_author.as_ref())
                        .is_some_and(|a| a.login == login) =>
                {
                    prev.and_then(|p| p.data.as_ref())
                        .and_then(|d| d.top_author.clone())
                }
                Some(login) => Some(author_info(crab, &login).await?),
                None => None,
            };
            (
                CommitSignals {
                    last_commit_at: human.first().map(|c| c.date),
                    commits_90d: human.iter().filter(|c| c.date > cutoff_90d).count() as u32,
                    contributors_12mo,
                    top_author_commit_share: top_share,
                },
                top_author,
                Some(etag),
            )
        }
    };

    // AIDEV-NOTE: releases 304 approximation — a release can age out of the
    // 365d window without the listing changing, so a reused count may
    // overstate by at most one until the next actual change self-corrects it.
    let (releases_365d, releases_etag) = match count_releases(
        client,
        owner,
        repo,
        prev.and_then(|p| p.releases_etag.as_deref()),
        window,
    )
    .await?
    {
        EtagResponse::NotModified => {
            let d = prev
                .and_then(|p| p.data.as_ref())
                .ok_or_else(|| anyhow::anyhow!("releases 304 but previous cache has no data"))?;
            (d.releases_365d, prev.and_then(|p| p.releases_etag.clone()))
        }
        EtagResponse::Fresh(count, etag) => (count, Some(etag)),
    };

    // Closed issues+PRs sample (issues API mixes both; split by `pull_request` field).
    let (issue_close, pr_close, closed_issues) = fetch_closed_items(crab, owner, repo).await?;

    // AIDEV-NOTE: repo.open_issues_count (issues + PRs combined) replaces two
    // search-API calls: cheaper, and the search bucket (30/min) 422s on
    // renamed/deleted repos. Approximation documented in methodology.
    let open_issues = meta.open_issues_count.unwrap_or(0).into();

    let (slow, slow_fetched_at) = match carry_slow(prev, now) {
        Some(slow) => (slow, prev.and_then(|p| p.slow_fetched_at)),
        None => {
            let has_ci = has_workflows(crab, owner, repo).await?;
            let has_tests = has_test_files(crab, owner, repo).await?;
            let readme_len = readme_len(crab, owner, repo).await;
            (
                SlowSignals {
                    has_ci,
                    has_tests,
                    readme_len,
                },
                Some(now),
            )
        }
    };

    Ok(RepoRecord {
        slug,
        fetched_at: now,
        status: Status::Ok,
        slow_fetched_at,
        commits_etag,
        releases_etag,
        data: Some(RepoSignals {
            archived: meta.archived.unwrap_or(false),
            stars: meta.stargazers_count.unwrap_or(0).into(),
            forks: meta.forks_count.unwrap_or(0).into(),
            description: meta.description,
            license_spdx: meta.license.map(|l| l.spdx_id),
            last_commit_at: cs.last_commit_at,
            commits_90d: cs.commits_90d,
            releases_365d,
            median_issue_close_days: issue_close,
            median_pr_close_days: pr_close,
            open_issues,
            closed_issues_sampled: closed_issues,
            contributors_12mo: cs.contributors_12mo,
            top_author_commit_share: cs.top_author_commit_share,
            top_author,
            has_ci: slow.has_ci,
            has_tests: slow.has_tests,
            readme_len: slow.readme_len,
        }),
    })
}

struct CommitInfo {
    date: DateTime<Utc>,
    author: Option<String>,
}

/// Commit-history-derived signals — freshly computed from a (Fresh) listing or
/// reused verbatim from the previous cache entry on a commits 304.
struct CommitSignals {
    last_commit_at: Option<DateTime<Utc>>,
    commits_90d: u32,
    contributors_12mo: u32,
    top_author_commit_share: f64,
}

// AIDEV-NOTE: stable-URL decision — the commits listing drops the old `since`
// param: it shifted daily, so its URL changed every day and a stored ETag
// would never match. `?per_page=100` is stable, and the 365d sampling window
// is applied client-side via within_window (semantics identical: the
// window-filtered newest ≤1000 commits equal the since-based set). ETags only
// match when URL+content are stable, so this is what makes conditional
// requests work. The releases URL was already stable.
async fn fetch_commits(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    prev_etag: Option<&str>,
) -> anyhow::Result<EtagResponse<Vec<CommitInfo>>> {
    let first = format!("https://api.github.com/repos/{owner}/{repo}/commits?per_page=100");
    let (first_page, mut next) =
        etag_get_with_next::<Vec<octocrab::models::repos::RepoCommit>>(client, &first, prev_etag)
            .await?;
    let EtagResponse::Fresh(first_items, etag) = first_page else {
        return Ok(EtagResponse::NotModified);
    };
    let mut out: Vec<CommitInfo> = first_items.iter().map(commit_info).collect();
    let mut pages = 1u8;
    // Same loop shape as before: up to MAX_COMMIT_PAGES pages of 100, newest
    // first. Sub-pages are fetched unconditionally (their URLs differ, so the
    // stored page-1 ETag is meaningless for them).
    while let Some(url) = next {
        if pages >= MAX_COMMIT_PAGES {
            break;
        }
        let (page, link_next) =
            etag_get_with_next::<Vec<octocrab::models::repos::RepoCommit>>(client, &url, None)
                .await?;
        match page {
            EtagResponse::Fresh(items, _) => out.extend(items.iter().map(commit_info)),
            // No If-None-Match was sent, so a 304 here would be a protocol
            // violation; stop paginating rather than failing the repo.
            EtagResponse::NotModified => break,
        }
        next = link_next;
        pages += 1;
    }
    // Newest first (API order, but sort defensively).
    out.sort_by_key(|c| std::cmp::Reverse(c.date));
    Ok(EtagResponse::Fresh(out, etag))
}

fn commit_info(c: &octocrab::models::repos::RepoCommit) -> CommitInfo {
    CommitInfo {
        date: c
            .commit
            .committer
            .as_ref()
            .and_then(|x| x.date)
            .unwrap_or_else(Utc::now),
        author: c.author.as_ref().map(|a| a.login.clone()),
    }
}

/// Keep only commits strictly newer than `cutoff` (the 365d sampling window).
/// Strictly-newer matches GitHub's old `since` semantics, so stats computed
/// over this set are unchanged.
fn within_window<'a>(commits: &'a [CommitInfo], cutoff: DateTime<Utc>) -> Vec<&'a CommitInfo> {
    commits.iter().filter(|c| c.date > cutoff).collect()
}

/// Conditional GET result: `NotModified` (304 — 0 paid calls; never read the
/// body) or `Fresh` with the parsed body and the response's ETag header value
/// (verbatim, quoted — sent back as-is in If-None-Match).
enum EtagResponse<T> {
    NotModified,
    Fresh(T, String),
}

/// GET `url` against the GitHub API with an If-None-Match header when a
/// previous ETag is known. 304 => NotModified, 200 => Fresh, anything else =>
/// Err (the run loop then warns and keeps the previous cache).
async fn etag_get<T: DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    etag: Option<&str>,
) -> anyhow::Result<EtagResponse<T>> {
    Ok(etag_get_with_next(client, url, etag).await?.0)
}

/// Same as etag_get, plus the `rel="next"` URL parsed from the Link header
/// (commits pagination; releases are single-page and use etag_get).
async fn etag_get_with_next<T: DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    etag: Option<&str>,
) -> anyhow::Result<(EtagResponse<T>, Option<String>)> {
    let mut req = client.get(url);
    if let Some(etag) = etag {
        req = req.header(IF_NONE_MATCH, etag);
    }
    let resp = req.send().await?;
    match resp.status() {
        reqwest::StatusCode::NOT_MODIFIED => Ok((EtagResponse::NotModified, None)),
        reqwest::StatusCode::OK => {
            let resp_etag = resp
                .headers()
                .get(ETAG)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
                .context(format!("200 from {url} without an ETag header"))?;
            let next = resp
                .headers()
                .get(LINK)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| parse_next_page(Some(v)));
            // reqwest's `json` feature is off in this crate; parse manually.
            let body = resp.text().await?;
            let parsed = serde_json::from_str(&body).with_context(|| format!("decode {url}"))?;
            Ok((EtagResponse::Fresh(parsed, resp_etag), next))
        }
        status => {
            // Include the body: GitHub rate-limit errors carry "rate limit" in
            // the message, which run() matches on to stop the whole run.
            let body = resp.text().await.unwrap_or_default();
            let snippet: String = body.chars().take(200).collect();
            Err(anyhow::anyhow!("GitHub {status} for {url}: {snippet}"))
        }
    }
}

/// GitHub Link header (e.g. `<u1>; rel="prev", <u2>; rel="next", <u3>;
/// rel="last"`) => the `rel="next"` URL. Pure fn: unit-tested seam.
fn parse_next_page(link_header: Option<&str>) -> Option<String> {
    link_header?.split(',').find_map(|part| {
        let mut segments = part.split(';');
        let target = segments.next()?.trim();
        let url = target.strip_prefix('<')?.strip_suffix('>')?;
        let is_next = segments
            .map(|p| p.trim().trim_start_matches("rel=").trim_matches('"'))
            .any(|p| p == "next");
        is_next.then(|| url.to_string())
    })
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

/// Releases listing via conditional request (stable URL, single page of 100
/// as before — releases are ordered newest first, so the window count comes
/// from page 1 alone, same as the previous octocrab-based fetch).
async fn count_releases(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    prev_etag: Option<&str>,
    cutoff: DateTime<Utc>,
) -> anyhow::Result<EtagResponse<u32>> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=100");
    match etag_get::<Vec<octocrab::models::repos::Release>>(client, &url, prev_etag).await? {
        EtagResponse::NotModified => Ok(EtagResponse::NotModified),
        EtagResponse::Fresh(releases, etag) => Ok(EtagResponse::Fresh(
            releases
                .iter()
                .filter(|r| r.published_at.is_some_and(|d| d > cutoff))
                .count() as u32,
            etag,
        )),
    }
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

async fn has_workflows(crab: &octocrab::Octocrab, owner: &str, repo: &str) -> anyhow::Result<bool> {
    // Empty .github/workflows (or missing dir) => 404/empty page, both mean "no CI".
    let page = crab.workflows(owner, repo).list().per_page(1).send().await;
    Ok(page.is_ok_and(|p| !p.items.is_empty()))
}

/// Root-listing heuristic: test dirs or obvious test files at repo root,
/// plus one level into `lua/` (standard neovim plugin layout).
// ponytail: two listings max; deeper nesting ignored. Walk the tree
// recursively only if false negatives show up in calibration.
async fn has_test_files(
    crab: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
) -> anyhow::Result<bool> {
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
    let has_lua = items
        .items
        .iter()
        .any(|c| c.name == "lua" && c.r#type == "dir");
    if has_lua
        && let Ok(lua) = crab
            .repos(owner, repo)
            .get_content()
            .path("lua")
            .send()
            .await
    {
        return Ok(lua.items.iter().any(|c| looks_testy(&c.name)));
    }
    Ok(false)
}

async fn readme_len(crab: &octocrab::Octocrab, owner: &str, repo: &str) -> usize {
    let Ok(readme) = crab.repos(owner, repo).get_readme().send().await else {
        return 0;
    };
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
    let star_sum = repos
        .iter()
        .filter_map(|r| r.stargazers_count)
        .map(u64::from)
        .sum();
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
        Self {
            listed: listed.iter().map(|s| s.to_lowercase()).collect(),
        }
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
        let c = |a: &str| CommitInfo {
            date: d,
            author: Some(a.into()),
        };
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

    fn prev_record(slow_age_days: i64, now: DateTime<Utc>, with_data: bool) -> RepoRecord {
        RepoRecord {
            slug: "o/r".into(),
            fetched_at: now - Duration::days(1),
            status: Status::Ok,
            slow_fetched_at: Some(now - Duration::days(slow_age_days)),
            commits_etag: None,
            releases_etag: None,
            data: with_data.then(|| RepoSignals {
                archived: false,
                stars: 1,
                forks: 1,
                description: None,
                license_spdx: None,
                last_commit_at: None,
                commits_90d: 0,
                releases_365d: 0,
                median_issue_close_days: None,
                median_pr_close_days: None,
                open_issues: 0,
                closed_issues_sampled: 0,
                contributors_12mo: 1,
                top_author_commit_share: 1.0,
                top_author: Some(AuthorInfo {
                    login: "alice".into(),
                    account_created_at: None,
                    other_repos_star_sum: 7,
                }),
                has_ci: true,
                has_tests: true,
                readme_len: 1234,
            }),
        }
    }

    #[test]
    fn carry_slow_without_previous_record_returns_none() {
        let now = Utc::now();
        assert!(carry_slow(None, now).is_none());
    }

    #[test]
    fn carry_slow_within_ttl_carries_previous_values() {
        let now = Utc::now();
        let prev = prev_record(29, now, true);
        let carried = carry_slow(Some(&prev), now).expect("fresh slow data carried");
        assert!(carried.has_ci);
        assert!(carried.has_tests);
        assert_eq!(carried.readme_len, 1234);
        // top_author is NOT carried here: it is coupled to the commits ETag
        // (reused on a commits 304, refetched on Fresh), not to SLOW_TTL.
    }

    #[test]
    fn carry_slow_past_ttl_returns_none() {
        let now = Utc::now();
        let prev = prev_record(31, now, true);
        assert!(carry_slow(Some(&prev), now).is_none());
    }

    #[test]
    fn carry_slow_without_previous_data_returns_none() {
        let now = Utc::now();
        let prev = prev_record(1, now, false);
        assert!(carry_slow(Some(&prev), now).is_none());
    }

    #[test]
    fn old_cache_json_without_slow_fetched_at_deserializes_as_none() {
        let json = r#"{"slug":"o/r","fetched_at":"2026-01-01T00:00:00Z","status":"not_found"}"#;
        let rec: RepoRecord = serde_json::from_str(json).expect("old cache still parses");
        assert_eq!(rec.slow_fetched_at, None);
        assert_eq!(rec.commits_etag, None);
        assert_eq!(rec.releases_etag, None);
    }

    #[test]
    fn parse_next_page_absent_header_is_none() {
        assert_eq!(parse_next_page(None), None);
    }

    #[test]
    fn parse_next_page_without_next_rel_is_none() {
        let header = r#"<https://api.github.com/o/r/commits?page=2>; rel="last""#;
        assert_eq!(parse_next_page(Some(header)), None);
    }

    #[test]
    fn parse_next_page_finds_next_among_other_rels() {
        let header = concat!(
            r#"<https://api.github.com/o/r/commits?page=1>; rel="prev","#,
            r#" <https://api.github.com/o/r/commits?page=3>; rel="next","#,
            r#" <https://api.github.com/o/r/commits?page=9>; rel="last""#,
        );
        assert_eq!(
            parse_next_page(Some(header)),
            Some("https://api.github.com/o/r/commits?page=3".to_string())
        );
    }

    #[test]
    fn parse_next_page_malformed_garbage_is_none() {
        assert_eq!(parse_next_page(Some("not a link header")), None);
        assert_eq!(parse_next_page(Some("<unterminated; rel=\"next\"")), None);
    }

    #[test]
    fn within_window_empty_input_is_empty() {
        let cutoff = Utc::now();
        assert!(within_window(&[], cutoff).is_empty());
    }

    #[test]
    fn within_window_keeps_commits_newer_than_cutoff() {
        let cutoff = Utc::now();
        let commits = vec![
            CommitInfo {
                date: cutoff + Duration::seconds(1),
                author: Some("a".into()),
            },
            CommitInfo {
                date: cutoff + Duration::days(30),
                author: Some("b".into()),
            },
        ];
        let kept = within_window(&commits, cutoff);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].author.as_deref(), Some("a"));
    }

    #[test]
    fn within_window_boundary_commit_exactly_at_cutoff_is_excluded() {
        let cutoff = Utc::now();
        let commits = vec![
            CommitInfo {
                date: cutoff,
                author: Some("at".into()),
            },
            CommitInfo {
                date: cutoff - Duration::seconds(1),
                author: None,
            },
            CommitInfo {
                date: cutoff + Duration::seconds(1),
                author: None,
            },
        ];
        let kept = within_window(&commits, cutoff);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].date, cutoff + Duration::seconds(1));
    }
}
