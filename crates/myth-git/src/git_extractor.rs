// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Git execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Directory is not a valid git repository: {0}")]
    NotAGitRepo(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// 某段时间窗口内的 Git 活动摘要
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitActivitySummary {
    /// 提交记录列表（如 "feat: add payment refund [a1b2c3d]"）
    pub commits: Vec<String>,
    /// git diff --stat 摘要信息
    pub diff_stat: String,
    /// 是否存在未提交的本地代码修改
    pub has_uncommitted_changes: bool,
    /// 未提交改动的文件列表
    pub uncommitted_files: Vec<String>,
    /// 精简的代码变更片段 (用于 AI 语义级交付物提炼，截断至安全长度)
    #[serde(default)]
    pub diff_snippet: String,
}

/// 单条 Git 提交结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommitRecord {
    pub hash: String,
    pub author: String,
    pub datetime: String,
    pub time_str: String,
    pub message: String,
}

/// 智能工时填充报告
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitWorkSummary {
    pub commits: Vec<GitCommitRecord>,
    pub summary_text: String,
    pub diff_stat: Option<String>,
    pub earliest_time: Option<String>,
    pub latest_time: Option<String>,
    pub is_fallback_to_day: bool,
}

pub struct GitScanner;

impl GitScanner {
    /// 检查指定路径是否为 Git 仓库
    pub fn is_git_repo(path: impl AsRef<Path>) -> bool {
        let output = Command::new("git")
            .arg("-C")
            .arg(path.as_ref())
            .arg("rev-parse")
            .arg("--is-inside-work-tree")
            .output();

        match output {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    /// 提取指定时间区间内的 Git 提交和代码变动
    pub fn extract_activity(
        repo_path: impl AsRef<Path>,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<GitActivitySummary, GitError> {
        let path = repo_path.as_ref();
        if !Self::is_git_repo(path) {
            return Err(GitError::NotAGitRepo(path.display().to_string()));
        }

        let mut summary = GitActivitySummary::default();

        // 1. 抓取时间窗口内的 Commit 列表
        let since_str = since.to_rfc3339();
        let until_str = until.to_rfc3339();
        let log_out = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("log")
            .arg(format!("--since={}", since_str))
            .arg(format!("--until={}", until_str))
            .arg("--pretty=format:%s [%h]")
            .output()?;

        if log_out.status.success() {
            let log_text = String::from_utf8_lossy(&log_out.stdout);
            summary.commits = log_text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
                .collect();
        }

        // 2. 检查未提交的本地工作区改动 (git status --porcelain)
        let status_out = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("status")
            .arg("--porcelain")
            .output()?;

        if status_out.status.success() {
            let status_text = String::from_utf8_lossy(&status_out.stdout);
            let uncommitted: Vec<String> = status_text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
                .collect();

            summary.has_uncommitted_changes = !uncommitted.is_empty();
            summary.uncommitted_files = uncommitted;
        }

        // 3. 提取未提交的 diff stat (git diff HEAD --stat)
        let diff_out = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("diff")
            .arg("HEAD")
            .arg("--stat")
            .output()?;

        if diff_out.status.success() {
            summary.diff_stat = String::from_utf8_lossy(&diff_out.stdout).trim().to_string();
        }

        // 4. 提取精简的代码变更内容 (优先未提交变更，若无则取区间内最近一次 commit 的 patch)，用于 AI 语义提炼
        let patch_out = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("diff")
            .arg("HEAD")
            .arg("--unified=1")
            .output()?;

        let mut raw_patch = if patch_out.status.success() {
            String::from_utf8_lossy(&patch_out.stdout)
                .trim()
                .to_string()
        } else {
            String::new()
        };

        if raw_patch.is_empty() && !summary.commits.is_empty() {
            if let Ok(log_patch) = Command::new("git")
                .arg("-C")
                .arg(path)
                .arg("log")
                .arg("-n")
                .arg("3")
                .arg("-p")
                .arg("--unified=1")
                .arg(format!("--since={}", since_str))
                .arg(format!("--until={}", until_str))
                .output()
            {
                if log_patch.status.success() {
                    raw_patch = String::from_utf8_lossy(&log_patch.stdout)
                        .trim()
                        .to_string();
                }
            }
        }

        if !raw_patch.is_empty() {
            let max_chars = 3000;
            summary.diff_snippet = raw_patch.chars().take(max_chars).collect();
        }

        Ok(summary)
    }

    /// 嗅探仓库近期是否有活动（用于后台静止预警）
    pub fn has_recent_activity(repo_path: impl AsRef<Path>, window_minutes: u32) -> bool {
        let path = repo_path.as_ref();
        if !Self::is_git_repo(path) {
            return false;
        }

        let since = Utc::now() - chrono::Duration::minutes(window_minutes as i64);
        let since_str = since.to_rfc3339();

        let log_out = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("log")
            .arg(format!("--since={}", since_str))
            .arg("--oneline")
            .output();

        if let Ok(out) = log_out {
            if out.status.success() && !out.stdout.is_empty() {
                return true;
            }
        }

        // 检查是否有任何文件状态变化
        let status_out = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("status")
            .arg("--porcelain")
            .output();

        if let Ok(out) = status_out {
            if out.status.success() && !out.stdout.is_empty() {
                return true;
            }
        }

        false
    }

    /// 提取指定时间区间（若无匹配则回退至当天）的 Git 活动并智能排版总结
    pub fn fetch_smart_work_summary(
        repo_path: impl AsRef<Path>,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
        day_since: DateTime<Utc>,
        day_until: DateTime<Utc>,
    ) -> Result<GitWorkSummary, GitError> {
        let path = repo_path.as_ref();
        if !Self::is_git_repo(path) {
            return Err(GitError::NotAGitRepo(path.display().to_string()));
        }

        // 1. 先尝试在特定时段窗口内拉取
        let (mut commits, is_fallback) = {
            let list = Self::read_commit_records(path, since, until)?;
            if list.is_empty() {
                let whole_day_list = Self::read_commit_records(path, day_since, day_until)?;
                (whole_day_list, true)
            } else {
                (list, false)
            }
        };

        if commits.is_empty() {
            return Ok(GitWorkSummary::default());
        }

        // 按时间从早到晚排序
        commits.sort_by(|a, b| a.datetime.cmp(&b.datetime));

        let earliest_time = commits.first().map(|c| c.time_str.clone());
        let latest_time = commits.last().map(|c| c.time_str.clone());

        // 2. 提取 diff stat 摘要
        let diff_stat = if commits.len() == 1 {
            let hash = &commits[0].hash;
            let out = Command::new("git")
                .arg("-C")
                .arg(path)
                .arg("show")
                .arg("--stat")
                .arg("--oneline")
                .arg(hash)
                .output()
                .ok();
            out.and_then(|o| {
                if o.status.success() {
                    let text = String::from_utf8_lossy(&o.stdout);
                    text.lines().last().map(|l| l.trim().to_string())
                } else {
                    None
                }
            })
        } else if commits.len() > 1 {
            let first_hash = format!("{}^", commits.first().unwrap().hash);
            let last_hash = &commits.last().unwrap().hash;
            let out = Command::new("git")
                .arg("-C")
                .arg(path)
                .arg("diff")
                .arg("--stat")
                .arg(format!("{}..{}", first_hash, last_hash))
                .output()
                .ok();
            out.and_then(|o| {
                if o.status.success() {
                    let text = String::from_utf8_lossy(&o.stdout);
                    text.lines().last().map(|l| l.trim().to_string())
                } else {
                    None
                }
            })
        } else {
            None
        };

        // 3. 组织人类友好的工作内容说明
        let mut summary_lines = Vec::new();
        if commits.len() == 1 {
            let c = &commits[0];
            summary_lines.push(format!("{} [{}]", c.message, c.hash));
        } else {
            summary_lines.push(format!(
                "完成代码研发与提交 (共 {} 次提交)：",
                commits.len()
            ));
            for (idx, c) in commits.iter().enumerate() {
                summary_lines.push(format!(
                    "{}. {} [{}] ({})",
                    idx + 1,
                    c.message,
                    c.hash,
                    c.time_str
                ));
            }
        }

        if let Some(ref stat) = diff_stat {
            if !stat.is_empty() {
                summary_lines.push(format!("• 变更规模: {}", stat));
            }
        }

        let summary_text = summary_lines.join("\n");

        Ok(GitWorkSummary {
            commits,
            summary_text,
            diff_stat,
            earliest_time,
            latest_time,
            is_fallback_to_day: is_fallback,
        })
    }

    fn read_commit_records(
        path: &Path,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<GitCommitRecord>, GitError> {
        let since_str = since.to_rfc3339();
        let until_str = until.to_rfc3339();
        let log_out = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("log")
            .arg(format!("--since={}", since_str))
            .arg(format!("--until={}", until_str))
            .arg("--pretty=format:%h|%an|%ad|%s")
            .arg("--date=iso-strict")
            .output()?;

        if !log_out.status.success() {
            return Ok(Vec::new());
        }

        let text = String::from_utf8_lossy(&log_out.stdout);
        let mut list = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() == 4 {
                let hash = parts[0].to_string();
                let author = parts[1].to_string();
                let datetime = parts[2].to_string();
                let message = parts[3].to_string();

                let time_str = if let Ok(dt) = DateTime::parse_from_rfc3339(&datetime) {
                    dt.format("%H:%M").to_string()
                } else if datetime.len() >= 16 {
                    datetime[11..16].to_string()
                } else {
                    "".to_string()
                };

                list.push(GitCommitRecord {
                    hash,
                    author,
                    datetime,
                    time_str,
                    message,
                });
            }
        }

        Ok(list)
    }

    /// 拉取指定起止日期区间内的所有提交，并按本地日期 (YYYY-MM-DD) 分组返回
    pub fn fetch_grouped_commits_by_date(
        repo_path: impl AsRef<Path>,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<std::collections::BTreeMap<String, Vec<GitCommitRecord>>, GitError> {
        let path = repo_path.as_ref();
        if !Self::is_git_repo(path) {
            return Err(GitError::NotAGitRepo(path.display().to_string()));
        }

        let commits = Self::read_commit_records(path, since, until)?;
        let mut map: std::collections::BTreeMap<String, Vec<GitCommitRecord>> =
            std::collections::BTreeMap::new();

        for c in commits {
            let date_key = if let Ok(dt) = DateTime::parse_from_rfc3339(&c.datetime) {
                dt.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d")
                    .to_string()
            } else if c.datetime.len() >= 10 {
                c.datetime[0..10].to_string()
            } else {
                continue;
            };

            map.entry(date_key).or_default().push(c);
        }

        // 确保每天内的提交按时间升序排列
        for list in map.values_mut() {
            list.sort_by(|a, b| a.datetime.cmp(&b.datetime));
        }

        Ok(map)
    }
}
