//! Operations on a member checkout inside a grove.
//!
//! Grove members are clones of the forest's bare repositories, so their `origin`
//! is a local path rather than the upstream remote. Fetching here is therefore
//! offline and cheap: the network hop happens once, during `sync-seed`.

use crate::git::command::{GitCommand, GitCommandError};
use std::path::{Path, PathBuf};

/// A member repository checked out inside a grove.
#[derive(Debug, Clone)]
pub struct MemberRepository {
    path: PathBuf,
}

/// How far two refs have drifted apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AheadBehind {
    /// Commits the local ref has that the other lacks.
    pub ahead: usize,
    /// Commits the other ref has that the local one lacks.
    pub behind: usize,
}

impl AheadBehind {
    /// Whether both refs point at the same history.
    pub fn is_synced(self) -> bool {
        self.ahead == 0 && self.behind == 0
    }

    /// Whether the local ref can reach the other without rewriting history.
    pub fn can_fast_forward(self) -> bool {
        self.ahead == 0 && self.behind > 0
    }

    /// Whether both sides carry commits the other lacks.
    pub fn is_diverged(self) -> bool {
        self.ahead > 0 && self.behind > 0
    }

    /// Whether the local ref is ahead and needs no update.
    pub fn is_only_ahead(self) -> bool {
        self.ahead > 0 && self.behind == 0
    }
}

impl MemberRepository {
    /// References the member checkout rooted at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the checkout path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns true when this looks like a git checkout.
    pub fn exists(&self) -> bool {
        self.path.join(".git").exists()
    }

    /// Fetches from `remote`, pruning deleted branches.
    pub fn fetch(&self, remote: &str, quiet: bool) -> Result<(), GitCommandError> {
        GitCommand::new("fetch")
            .args([remote, "--prune", "--tags"])
            .cwd(&self.path)
            .quiet(quiet)
            .run()
    }

    /// Resolves a revision to a SHA, returning `None` when it does not exist.
    pub fn rev_parse(&self, rev: &str) -> Result<Option<String>, GitCommandError> {
        Ok(GitCommand::new("rev-parse")
            .args(["--verify", "--quiet", rev])
            .cwd(&self.path)
            .quiet(true)
            .run_output()?
            .optional_stdout())
    }

    /// Returns the checked-out branch, or `None` when HEAD is detached.
    pub fn current_branch(&self) -> Result<Option<String>, GitCommandError> {
        Ok(GitCommand::new("symbolic-ref")
            .args(["--quiet", "--short", "HEAD"])
            .cwd(&self.path)
            .quiet(true)
            .run_output()?
            .optional_stdout())
    }

    /// Counts how far `local` and `other` have drifted apart.
    pub fn ahead_behind(&self, local: &str, other: &str) -> Result<AheadBehind, GitCommandError> {
        let range = format!("{local}...{other}");
        let out = GitCommand::new("rev-list")
            .args(["--left-right", "--count", &range])
            .cwd(&self.path)
            .quiet(true)
            .run_capture()?;

        let mut counts = out.split_whitespace();
        let parsed = counts
            .next()
            .and_then(|ahead| ahead.parse().ok())
            .zip(counts.next().and_then(|behind| behind.parse().ok()));

        let Some((ahead, behind)) = parsed else {
            return Err(GitCommandError::UnexpectedOutput {
                command: "rev-list".to_string(),
                detail: format!("expected two commit counts, got '{out}'"),
            });
        };

        Ok(AheadBehind { ahead, behind })
    }

    /// Counts entries with uncommitted changes, including untracked files.
    pub fn dirty_count(&self) -> Result<usize, GitCommandError> {
        let out = GitCommand::new("status")
            .arg("--porcelain")
            .cwd(&self.path)
            .quiet(true)
            .run_capture()?;

        Ok(out.lines().filter(|line| !line.trim().is_empty()).count())
    }

    /// Advances the working tree to `rev`, refusing anything but a fast-forward.
    pub fn merge_ff_only(&self, rev: &str, quiet: bool) -> Result<(), GitCommandError> {
        GitCommand::new("merge")
            .args(["--ff-only", rev])
            .cwd(&self.path)
            .quiet(quiet)
            .run()
    }

    /// Rebases the current branch onto `rev`.
    ///
    /// Returns `false` when the rebase could not complete. A conflicted rebase is
    /// aborted rather than left in progress, so an interrupted grove update never
    /// hands back a checkout stuck mid-rebase.
    pub fn rebase_onto(&self, rev: &str) -> Result<bool, GitCommandError> {
        let outcome = GitCommand::new("rebase")
            .arg(rev)
            .cwd(&self.path)
            .quiet(true)
            .run_output()?;

        if outcome.success {
            return Ok(true);
        }

        GitCommand::new("rebase")
            .arg("--abort")
            .cwd(&self.path)
            .quiet(true)
            .run_output()?;
        Ok(false)
    }
}

/// Truncates a SHA to the length used in reports.
pub fn short_sha(sha: &str) -> &str {
    let end = sha.char_indices().nth(8).map_or(sha.len(), |(idx, _)| idx);
    &sha[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    fn repo_with_commit() -> (tempfile::TempDir, MemberRepository) {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = MemberRepository::new(dir.path());
        let git = |args: &[&str]| {
            GitCommand::new(args[0])
                .args(&args[1..])
                .cwd(dir.path())
                .quiet(true)
                .run()
                .unwrap();
        };
        git(&["init", "--initial-branch", "main"]);
        git(&["config", "user.email", "test@test.com"]);
        git(&["config", "user.name", "Test"]);
        git(&["commit", "--allow-empty", "-m", "initial"]);
        (dir, repo)
    }

    #[test]
    fn rev_parse_returns_none_for_a_missing_ref() {
        // Given a repository with one commit
        let (_dir, repo) = repo_with_commit();

        // When resolving a ref that does not exist
        let resolved = repo.rev_parse("refs/remotes/origin/absent").unwrap();

        // Then no SHA is reported
        assert_eq!(resolved, None);
    }

    #[test]
    fn ahead_behind_counts_both_directions() {
        // Given a branch that diverged from main by one commit each way
        let (dir, repo) = repo_with_commit();
        let git = |args: &[&str]| {
            GitCommand::new(args[0])
                .args(&args[1..])
                .cwd(dir.path())
                .quiet(true)
                .run()
                .unwrap();
        };
        git(&["checkout", "-b", "feature"]);
        git(&["commit", "--allow-empty", "-m", "feature work"]);
        git(&["checkout", "main"]);
        git(&["commit", "--allow-empty", "-m", "main work"]);

        // When comparing feature against main
        let drift = repo
            .ahead_behind("refs/heads/feature", "refs/heads/main")
            .unwrap();

        // Then each side reports one exclusive commit
        assert_eq!(
            drift,
            AheadBehind {
                ahead: 1,
                behind: 1
            }
        );
    }

    #[test]
    fn dirty_count_includes_untracked_files() {
        // Given a repository with an untracked file
        let (dir, repo) = repo_with_commit();
        std::fs::write(dir.path().join("scratch.txt"), "wip").unwrap();

        // When counting dirty entries
        let count = repo.dirty_count().unwrap();

        // Then the untracked file is counted
        assert_eq!(count, 1);
    }

    #[test]
    fn current_branch_is_none_when_head_is_detached() {
        // Given a repository with a detached HEAD
        let (dir, repo) = repo_with_commit();
        GitCommand::new("checkout")
            .args(["--detach", "HEAD"])
            .cwd(dir.path())
            .quiet(true)
            .run()
            .unwrap();

        // When reading the current branch
        let branch = repo.current_branch().unwrap();

        // Then no branch is reported
        assert_eq!(branch, None);
    }

    #[test_case(0, 0, (true, false, false, false) ; "identical refs are synced")]
    #[test_case(0, 3, (false, true, false, false) ; "behind only can fast forward")]
    #[test_case(2, 3, (false, false, true, false) ; "both sides ahead is diverged")]
    #[test_case(2, 0, (false, false, false, true) ; "ahead only needs no update")]
    fn drift_classification(ahead: usize, behind: usize, expected: (bool, bool, bool, bool)) {
        // Given a drift measurement
        let drift = AheadBehind { ahead, behind };

        // When classifying it as synced, fast-forwardable, diverged, or only ahead
        let actual = (
            drift.is_synced(),
            drift.can_fast_forward(),
            drift.is_diverged(),
            drift.is_only_ahead(),
        );

        // Then every predicate matches the expected outcome
        assert_eq!(actual, expected);
    }

    #[test_case("abcdef1234567890", "abcdef12" ; "truncates long sha")]
    #[test_case("abc", "abc" ; "leaves short sha intact")]
    #[test_case("", "" ; "handles empty input")]
    fn short_sha_truncates_for_display(input: &str, expected: &str) {
        // Given a SHA of some length
        // When shortening it for display
        // Then at most eight characters are kept
        assert_eq!(short_sha(input), expected);
    }
}
