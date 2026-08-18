use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One cached raw record per repo: `data/<cat>/raw/<owner>__<repo>.json`.
/// Scoring consumes only this struct — never the GitHub API.
#[derive(Debug, Serialize, Deserialize)]
pub struct RepoRecord {
    pub slug: String,
    pub fetched_at: DateTime<Utc>,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<RepoSignals>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    NotFound,
    Redirected,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepoSignals {
    // metadata
    pub archived: bool,
    pub stars: u64,
    pub forks: u64,
    pub description: Option<String>,
    pub license_spdx: Option<String>,

    // maintenance (already bot-filtered at collection time)
    pub last_commit_at: Option<DateTime<Utc>>,
    pub commits_90d: u32,
    pub releases_365d: u32,
    pub median_issue_close_days: Option<f64>,
    pub median_pr_close_days: Option<f64>,
    pub open_issues: u64,
    pub closed_issues_sampled: u32,

    // community
    pub contributors_12mo: u32,
    /// Share (0..1) of sampled commits by the top author.
    pub top_author_commit_share: f64,
    pub top_author: Option<AuthorInfo>,

    // quality
    pub has_ci: bool,
    pub has_tests: bool,
    pub readme_len: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthorInfo {
    pub login: String,
    pub account_created_at: Option<DateTime<Utc>>,
    /// Summed stargazers over the author's first 100 owned repos (log-scaled later).
    pub other_repos_star_sum: u64,
}
