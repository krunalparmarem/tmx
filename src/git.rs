//! Thin, dependency-free wrapper around the `git` CLI.
//!
//! Every function shells out to `git` (per the project's "no heavy dependencies"
//! rule) and returns user-friendly `Result<_, String>` errors. This module powers
//! the Worktree Swarm: each agent works in its own `git worktree` + branch so
//! parallel agents never clobber each other's files.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Run git in `dir`, returning trimmed stdout on success.
fn git_out(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "git is not installed or not in PATH".to_string()
            } else {
                format!("failed to run git: {e}")
            }
        })?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Run git in `dir`, ignoring stdout.
fn git_ok(dir: &Path, args: &[&str]) -> Result<(), String> {
    git_out(dir, args).map(|_| ())
}

/// Result of a git operation that may hit merge conflicts.
pub enum MergeOutcome {
    Clean,
    Conflict(Vec<String>),
}

/// Files changed / lines added / lines removed for a diff.
pub struct DiffStat {
    pub files: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub untracked: usize,
}

pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn is_repo(dir: &Path) -> bool {
    git_out(dir, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s == "true")
        .unwrap_or(false)
}

pub fn repo_root(dir: &Path) -> Result<PathBuf, String> {
    git_out(dir, &["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

pub fn current_branch(dir: &Path) -> Result<String, String> {
    let b = git_out(dir, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if b == "HEAD" {
        // Detached head — fall back to a stable ref.
        Ok(git_out(dir, &["rev-parse", "--short", "HEAD"])?)
    } else {
        Ok(b)
    }
}

pub fn has_commits(dir: &Path) -> bool {
    git_out(dir, &["rev-parse", "--verify", "HEAD"]).is_ok()
}

/// Initialize a new git repo and make a baseline commit so worktrees can branch.
pub fn init_repo(dir: &Path) -> Result<(), String> {
    git_ok(dir, &["init"])?;
    ensure_baseline_commit(dir)
}

/// Ensure the repo has at least one commit (worktree branching needs a ref).
pub fn ensure_baseline_commit(dir: &Path) -> Result<(), String> {
    if has_commits(dir) {
        return Ok(());
    }
    git_ok(
        dir,
        &["commit", "--allow-empty", "-m", "tmx: baseline commit"],
    )
}

pub fn is_dirty(dir: &Path) -> Result<bool, String> {
    Ok(!git_out(dir, &["status", "--porcelain"])?.is_empty())
}

/// Create a worktree at `path` on a new `branch` starting from `base`.
pub fn worktree_add(repo: &Path, path: &Path, branch: &str, base: &str) -> Result<(), String> {
    let path_str = path.to_string_lossy();
    git_ok(repo, &["worktree", "add", "-b", branch, &path_str, base])
}

pub fn worktree_remove(repo: &Path, path: &Path) -> Result<(), String> {
    let path_str = path.to_string_lossy();
    git_ok(repo, &["worktree", "remove", "--force", &path_str])
}

pub fn delete_branch(repo: &Path, branch: &str) -> Result<(), String> {
    git_ok(repo, &["branch", "-D", branch])
}

pub fn branch_exists(repo: &Path, branch: &str) -> bool {
    git_out(repo, &["rev-parse", "--verify", branch]).is_ok()
}

/// Branches whose names start with `prefix` (e.g. `tmx/my-session/`).
pub fn list_branches_with_prefix(repo: &Path, prefix: &str) -> Result<Vec<String>, String> {
    let pattern = format!("{prefix}*");
    let out = git_out(repo, &["branch", "--list", &pattern])?;
    Ok(out
        .lines()
        .map(|l| l.trim().trim_start_matches('*').trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Commit everything in a worktree (used when "keeping" an agent's uncommitted work).
pub fn commit_all(worktree: &Path, msg: &str) -> Result<(), String> {
    git_ok(worktree, &["add", "-A"])?;
    git_ok(worktree, &["commit", "-m", msg])
}

pub fn checkout(repo: &Path, branch: &str) -> Result<(), String> {
    git_ok(repo, &["checkout", branch])
}

/// Merge `branch` into the currently checked-out branch (no fast-forward edit prompt).
pub fn merge_branch(repo: &Path, branch: &str) -> Result<MergeOutcome, String> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["merge", "--no-edit", branch])
        .output()
        .map_err(|e| format!("failed to run git merge: {e}"))?;
    if out.status.success() {
        return Ok(MergeOutcome::Clean);
    }
    // Distinguish a conflict from a hard error.
    let conflicts = git_out(repo, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
    if conflicts.is_empty() {
        Err(format!(
            "git merge failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    } else {
        Ok(MergeOutcome::Conflict(
            conflicts.lines().map(|s| s.to_string()).collect(),
        ))
    }
}

/// Diff stats for a worktree's *current working state* (committed + uncommitted)
/// against `base`. This reflects what an agent actually did, even before it commits.
pub fn working_diff_stat(worktree: &Path, base: &str) -> Result<DiffStat, String> {
    let numstat = git_out(worktree, &["diff", "--numstat", base])?;
    let mut files = 0;
    let mut insertions = 0;
    let mut deletions = 0;
    for line in numstat.lines() {
        let mut cols = line.split('\t');
        let ins = cols.next().unwrap_or("0");
        let del = cols.next().unwrap_or("0");
        files += 1;
        insertions += ins.parse::<usize>().unwrap_or(0);
        deletions += del.parse::<usize>().unwrap_or(0);
    }
    let untracked = git_out(worktree, &["ls-files", "--others", "--exclude-standard"])
        .map(|s| s.lines().filter(|l| !l.is_empty()).count())
        .unwrap_or(0);
    Ok(DiffStat {
        files,
        insertions,
        deletions,
        untracked,
    })
}
