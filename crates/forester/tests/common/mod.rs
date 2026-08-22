#![expect(dead_code, reason = "test utilities may not all be used")]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "panics are appropriate in tests"
)]
//! Common test utilities for forester integration tests.

use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

/// Get path to forester binary
pub fn forester_bin() -> &'static Path {
    assert_cmd::cargo::cargo_bin!("forester")
}

/// Run forester command in the given directory
pub fn forester_cmd(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(forester_bin())
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("Failed to execute forester");

    output_to_tuple(output)
}

/// Run forester init in the given directory
pub fn forester_init(cwd: &Path, name: Option<&str>) -> (i32, String, String) {
    let mut args = vec!["init"];
    if let Some(n) = name {
        args.push("--name");
        args.push(n);
    }
    forester_cmd(cwd, &args)
}

/// Run forester seed in the given directory
pub fn forester_seed(cwd: &Path, verbose: bool) -> (i32, String, String) {
    let mut args = vec!["seed"];
    if verbose {
        args.push("--verbose");
    }
    forester_cmd(cwd, &args)
}

/// Run forester sync-seed in the given directory
pub fn forester_sync_seed(cwd: &Path, verbose: bool) -> (i32, String, String) {
    let mut args = vec!["sync-seed"];
    if verbose {
        args.push("--verbose");
    }
    forester_cmd(cwd, &args)
}

/// Run forester grove command
pub fn forester_grove(cwd: &Path, subcmd: &str, args: &[&str]) -> (i32, String, String) {
    let mut full_args = vec!["grove", subcmd];
    full_args.extend(args);
    forester_cmd(cwd, &full_args)
}

/// Run forester grove update in the given directory
pub fn forester_grove_update(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    forester_grove(cwd, "update", args)
}

/// Run the top-level forester update in the given directory
pub fn forester_update(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let mut full_args = vec!["update"];
    full_args.extend(args);
    forester_cmd(cwd, &full_args)
}

/// Create a temp directory for testing
pub fn temp_forest() -> TempDir {
    TempDir::new().expect("Failed to create temp directory")
}

/// Run a git command in the given repo, returning trimmed stdout
pub fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("Failed to execute git");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Run a git command, asserting that it succeeded
pub fn git_ok(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("Failed to execute git");
    assert!(
        output.status.success(),
        "git {:?} failed in {}: {}",
        args,
        repo.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Resolve a revision to a full SHA, or None when it does not exist
pub fn rev_parse(repo: &Path, rev: &str) -> Option<String> {
    let sha = git(repo, &["rev-parse", "--verify", "--quiet", rev]);
    (!sha.is_empty()).then_some(sha)
}

/// Commit in a checkout without relying on ambient git identity config
pub fn commit_in(repo: &Path, message: &str) -> String {
    git_ok(
        repo,
        &[
            "-c",
            "user.email=test@test.com",
            "-c",
            "user.name=Test",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ],
    );
    git_ok(repo, &["rev-parse", "HEAD"])
}

/// Add a commit to a bare "upstream" repo by cloning, committing, and pushing
pub fn push_commit_upstream(bare_upstream: &Path, message: &str) -> String {
    let temp = TempDir::new().expect("Failed to create temp directory");
    let clone = temp.path().join("clone");
    Command::new("git")
        .args(["clone"])
        .arg(bare_upstream)
        .arg(&clone)
        .output()
        .expect("Failed to clone upstream");

    let sha = commit_in(&clone, message);
    git_ok(&clone, &["push", "origin", "HEAD:main"]);
    sha
}

/// Initialize a git repo in the given directory
pub fn git_init(path: &Path) {
    Command::new("git")
        .current_dir(path)
        .args(["init"])
        .output()
        .expect("Failed to git init");
}

/// Create a bare git repo with a commit
pub fn create_bare_repo(path: &Path, name: &str) -> std::path::PathBuf {
    let repo_path = path.join(format!("{}.git", name));

    // Create a temp repo, add a commit, then clone as bare
    let temp = TempDir::new().unwrap();
    git_init(temp.path());

    // Configure git user for commit
    Command::new("git")
        .current_dir(temp.path())
        .args(["config", "user.email", "test@test.com"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(temp.path())
        .args(["config", "user.name", "Test"])
        .output()
        .unwrap();

    // Create initial commit
    std::fs::write(temp.path().join("README.md"), "# Test").unwrap();
    Command::new("git")
        .current_dir(temp.path())
        .args(["add", "."])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(temp.path())
        .args(["commit", "-m", "Initial commit"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(temp.path())
        .args(["branch", "-M", "main"])
        .output()
        .unwrap();

    // Clone as bare
    Command::new("git")
        .args(["clone", "--bare"])
        .arg(temp.path())
        .arg(&repo_path)
        .output()
        .expect("Failed to create bare repo");

    repo_path
}

fn output_to_tuple(output: Output) -> (i32, String, String) {
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (exit_code, stdout, stderr)
}
