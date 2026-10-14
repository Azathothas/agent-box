//! `--base REV` against a real repository. A commit made without a remap leaves
//! citations that the check against `HEAD` cannot see, because the tree has no
//! change then. Against the parent of the commit, the check finds them, and
//! `remap --base` moves them. With no git on PATH, the tests say so and pass.

use std::path::Path;
use std::process::Command;

use super::{SOURCE, Tree, run_args};
use crate::proc;

const LIB: &str = "crates/x/src/lib.rs";

/// Run git with no global or system configuration. So no hook, signing key or
/// line-end rule of this machine reaches the repository of the test.
fn git(root: &Path, args: &[&str]) {
    let mut cmd = Command::new("git");
    cmd.current_dir(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", root.join("no-such-config"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "xtask todo test")
        .env("GIT_AUTHOR_EMAIL", concat!("test", "@", "example.invalid"))
        .env("GIT_COMMITTER_NAME", "xtask todo test")
        .env("GIT_COMMITTER_EMAIL", concat!("test", "@", "example.invalid"))
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    let out = proc::run_command(&mut cmd, proc::QUICK).expect("git runs");
    assert!(out.success(), "git {args:?}: {out:?}");
}

fn have_git() -> bool {
    let found = proc::probe_version("git", &["--version"]).is_some();
    if !found {
        eprintln!("git is not on PATH: this test needs it, and did not run");
    }
    found
}

/// A tree of the record in a repository of its own, with one commit.
fn committed(name: &str) -> Tree {
    let t = Tree::new(name);
    git(t.path(), &["init", "-q"]);
    git(t.path(), &["add", "-A"]);
    git(t.path(), &["commit", "-q", "-m", "the record"]);
    t
}

/// The lines of the report of the check that ask for the remap of LIB.
fn asks(err: &str, command: &str) -> usize {
    let want = format!("run `{command} {LIB}`");
    err.lines().filter(|l| l.contains(want.as_str())).count()
}

#[test]
fn a_commit_made_without_a_remap_is_found_against_its_parent() {
    if !have_git() {
        return;
    }
    let t = committed("base-commit");
    let root = t.root_arg();
    t.write(LIB, &format!("line 0\n{SOURCE}"));
    git(t.path(), &["commit", "-q", "-a", "-m", "an edit with no remap"]);

    // Against HEAD the tree has no change, so no citation left behind shows.
    let (_, _, err) = run_args(&["check", "--root", root.as_str()]);
    assert_eq!(asks(&err, "cargo todo remap"), 0, "{err}");
    // Against the parent of the commit, each of the three.
    let (code, _, err) = run_args(&["check", "--root", root.as_str(), "--base", "HEAD~1"]);
    assert_eq!(code, 1, "{err}");
    assert_eq!(asks(&err, "cargo todo remap --base HEAD~1"), 3, "{err}");

    // remap --base moves them, and the check against the parent agrees.
    let (code, out, err) = run_args(&["remap", "--root", root.as_str(), "--base", "HEAD~1", LIB]);
    assert_eq!(code, 0, "{out}{err}");
    let area = t.read("TODO/area.md");
    assert!(area.contains("`crates/x/src/lib.rs:3-5`"), "{area}");
    let (code, out, err) = run_args(&["check", "--root", root.as_str(), "--base", "HEAD~1"]);
    assert_eq!(code, 0, "{out}{err}");
}

#[test]
fn a_base_that_git_cannot_read_fails_the_check() {
    if !have_git() {
        return;
    }
    let t = committed("base-unread");
    let root = t.root_arg();
    let (code, _, err) = run_args(&["check", "--root", root.as_str()]);
    assert_eq!(code, 0, "{err}");
    let (code, _, err) = run_args(&["check", "--root", root.as_str(), "--base", "no-such-revision"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("--base: git cannot compare the tree with `no-such-revision`"), "{err}");
    // A revision that would reach git as an option is refused.
    let (code, _, err) = run_args(&["check", "--root", root.as_str(), "--base", "--output=x"]);
    assert_eq!(code, 64, "{err}");
}
