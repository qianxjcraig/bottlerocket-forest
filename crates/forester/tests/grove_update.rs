#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "panics are appropriate in tests"
)]
//! Integration tests for `forester grove update` and `forester update`.
//!
//! Local bare repos stand in for remotes so the whole update path is exercised
//! without network access.

mod common;

use common::{
    commit_in, create_bare_repo, forester_grove, forester_grove_update, forester_seed,
    forester_sync_seed, forester_update, git_ok, push_commit_upstream, rev_parse, temp_forest,
};
use std::fs;
use std::path::{Path, PathBuf};

/// A seeded forest with one member whose upstream is a local bare repo.
struct Fixture {
    forest: tempfile::TempDir,
    upstream: PathBuf,
}

impl Fixture {
    /// Seeds a forest and creates `grove`.
    fn with_grove(grove: &str) -> Self {
        let fixture = Self::seeded();
        let (code, out, err) = forester_grove(fixture.root(), "create", &[grove]);
        assert_eq!(code, 0, "grove create failed: {out} {err}");
        fixture
    }

    fn seeded() -> Self {
        let forest = temp_forest();
        let repos_dir = forest.path().join("repos");
        fs::create_dir_all(&repos_dir).unwrap();
        let upstream = create_bare_repo(&repos_dir, "repo-a");

        let config = format!(
            r#"[forest]
name = "test-forest"

[[forest.member]]
name = "repo-a"
remote = "{}"
path = "repo-a"
default_branch = "main"
"#,
            upstream.display()
        );
        fs::write(forest.path().join("forester.toml"), config).unwrap();

        let (code, out, err) = forester_seed(forest.path(), false);
        assert_eq!(code, 0, "seed failed: {out} {err}");

        Self { forest, upstream }
    }

    fn root(&self) -> &Path {
        self.forest.path()
    }

    fn checkout(&self, grove: &str) -> PathBuf {
        self.root().join("groves").join(grove).join("repo-a")
    }

    /// Adds an upstream commit and fetches it into the forest's bare repos.
    fn publish_upstream(&self, message: &str) -> String {
        let sha = push_commit_upstream(&self.upstream, message);
        let (code, out, err) = forester_sync_seed(self.root(), false);
        assert_eq!(code, 0, "sync-seed failed: {out} {err}");
        sha
    }
}

#[test]
fn grove_update_fast_forwards_a_clean_checkout() {
    // Given a grove that is behind a freshly fetched upstream commit
    let fixture = Fixture::with_grove("develop");
    let upstream_sha = fixture.publish_upstream("upstream work");
    let checkout = fixture.checkout("develop");
    assert_ne!(rev_parse(&checkout, "HEAD"), Some(upstream_sha.clone()));

    // When updating the grove
    let (code, out, err) = forester_grove_update(fixture.root(), &["develop"]);
    assert_eq!(code, 0, "grove update failed: {out} {err}");

    // Then the checkout is advanced to the upstream tip
    assert_eq!(rev_parse(&checkout, "HEAD"), Some(upstream_sha));
}

#[test]
fn grove_update_leaves_a_dirty_checkout_alone() {
    // Given a grove behind upstream with uncommitted work
    let fixture = Fixture::with_grove("develop");
    fixture.publish_upstream("upstream work");
    let checkout = fixture.checkout("develop");
    fs::write(checkout.join("scratch.txt"), "work in progress").unwrap();
    let before = rev_parse(&checkout, "HEAD").unwrap();

    // When updating the grove
    let (code, out, err) = forester_grove_update(fixture.root(), &["develop"]);
    assert_eq!(code, 0, "grove update failed: {out} {err}");

    // Then the checkout is untouched and the in-progress work survives
    assert_eq!(rev_parse(&checkout, "HEAD").unwrap(), before);
    assert!(checkout.join("scratch.txt").exists());

    // And the skip is explained with a way forward
    let combined = format!("{out}{err}");
    assert!(
        combined.contains("uncommitted"),
        "expected the skip to be explained, got: {combined}"
    );
}

#[test]
fn grove_update_refuses_a_diverged_branch_under_ff_only() {
    // Given a grove holding a local commit while upstream also moved on
    let fixture = Fixture::with_grove("develop");
    let checkout = fixture.checkout("develop");
    let local_sha = commit_in(&checkout, "local work");
    fixture.publish_upstream("upstream work");

    // When updating with the default fast-forward-only strategy
    let (code, out, err) = forester_grove_update(fixture.root(), &["develop"]);
    assert_eq!(code, 0, "grove update failed: {out} {err}");

    // Then the local commit is preserved rather than rewritten
    assert_eq!(rev_parse(&checkout, "HEAD").unwrap(), local_sha);
    let combined = format!("{out}{err}");
    assert!(
        combined.contains("diverged"),
        "expected a divergence report, got: {combined}"
    );
}

#[test]
fn grove_update_rebases_local_commits_when_asked() {
    // Given a grove holding a local commit while upstream also moved on
    let fixture = Fixture::with_grove("develop");
    let checkout = fixture.checkout("develop");
    commit_in(&checkout, "local work");
    let upstream_sha = fixture.publish_upstream("upstream work");

    // When updating with the rebase strategy
    let (code, out, err) =
        forester_grove_update(fixture.root(), &["develop", "--strategy", "rebase"]);
    assert_eq!(code, 0, "grove update failed: {out} {err}");

    // Then the local commit sits directly on the new upstream tip
    assert_eq!(git_ok(&checkout, &["rev-parse", "HEAD~1"]), upstream_sha);
    assert_eq!(
        git_ok(&checkout, &["log", "-1", "--format=%s"]),
        "local work"
    );
}

#[test]
fn grove_update_adds_a_member_introduced_after_the_grove_existed() {
    // Given an existing grove
    let fixture = Fixture::with_grove("develop");

    // And a second member added to the forest afterwards, already ahead upstream
    let second = create_bare_repo(&fixture.root().join("repos"), "repo-b");
    let second_sha = push_commit_upstream(&second, "upstream work on repo-b");
    let config = fs::read_to_string(fixture.root().join("forester.toml")).unwrap();
    fs::write(
        fixture.root().join("forester.toml"),
        format!(
            r#"{config}
[[forest.member]]
name = "repo-b"
remote = "{}"
path = "nested/repo-b"
default_branch = "main"
"#,
            second.display()
        ),
    )
    .unwrap();
    forester_seed(fixture.root(), false);
    forester_sync_seed(fixture.root(), false);

    // When updating the grove
    let (code, out, err) = forester_grove_update(fixture.root(), &["develop"]);
    assert_eq!(code, 0, "grove update failed: {out} {err}");

    // Then the new member is checked out into the existing grove
    let added = fixture.root().join("groves/develop/nested/repo-b");
    assert!(
        added.join(".git").exists(),
        "expected checkout at {added:?}"
    );

    // And it is brought current rather than left at whatever the bare repo held
    assert_eq!(
        rev_parse(&added, "HEAD"),
        Some(second_sha),
        "a newly added checkout should also be advanced to upstream"
    );
}

#[test]
fn grove_update_dry_run_writes_nothing() {
    // Given a grove that is behind upstream
    let fixture = Fixture::with_grove("develop");
    fixture.publish_upstream("upstream work");
    let checkout = fixture.checkout("develop");
    let before = rev_parse(&checkout, "HEAD").unwrap();

    // When updating with --dry-run
    let (code, out, err) = forester_grove_update(fixture.root(), &["develop", "--dry-run"]);
    assert_eq!(code, 0, "grove update failed: {out} {err}");

    // Then the checkout stays exactly where it was
    assert_eq!(rev_parse(&checkout, "HEAD").unwrap(), before);
}

#[test]
fn grove_update_rejects_an_unknown_grove() {
    // Given a seeded forest with no such grove
    let fixture = Fixture::seeded();

    // When updating a grove that does not exist
    let (code, _out, err) = forester_grove_update(fixture.root(), &["absent"]);

    // Then the command fails and names the missing grove
    assert_ne!(code, 0, "expected a nonzero exit");
    assert!(
        err.contains("absent"),
        "expected the grove name, got: {err}"
    );
}

#[test]
fn feature_grove_update_advances_its_namespaced_branch() {
    // Given a feature grove, whose members sit on <grove>/<member> branches
    let fixture = Fixture::with_grove("my-feature");
    let checkout = fixture.checkout("my-feature");
    assert_eq!(
        git_ok(&checkout, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "my-feature/repo-a"
    );

    // And a new upstream commit
    let upstream_sha = fixture.publish_upstream("upstream work");

    // When updating the grove
    let (code, out, err) = forester_grove_update(fixture.root(), &["my-feature"]);
    assert_eq!(code, 0, "grove update failed: {out} {err}");

    // Then the grove-local branch is rebased forward onto the new base
    assert_eq!(rev_parse(&checkout, "HEAD"), Some(upstream_sha));
    assert_eq!(
        git_ok(&checkout, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "my-feature/repo-a"
    );
}

#[test]
fn update_command_fetches_upstream_then_advances_the_grove() {
    // Given a grove and an upstream commit that has not been fetched
    let fixture = Fixture::with_grove("develop");
    let upstream_sha = push_commit_upstream(&fixture.upstream, "upstream work");
    let checkout = fixture.checkout("develop");

    // When running the combined update command
    let (code, out, err) = forester_update(fixture.root(), &["develop"]);
    assert_eq!(code, 0, "update failed: {out} {err}");

    // Then one command both fetched and advanced the checkout
    assert_eq!(rev_parse(&checkout, "HEAD"), Some(upstream_sha));
}
