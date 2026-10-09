// Copyright (C) 2026 MythManDay Authors. All Rights Reserved.
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
}
