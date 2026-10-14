//! The tests of the record checker, ported from the tests of podssh-todo. A
//! small record that agrees with itself is written into a fresh directory
//! under the temporary directory of the system. Each plant changes one thing
//! in it, and the reader must find that thing. Each tree is removed when its
//! test ends.

mod base;
mod moves;
mod plants;
mod writer;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const INDEX: &str = "# Index

**3 entries: 1 open, 0 partial, 1 blocked, 1 done.**

| Priority | open | partial | blocked | done | total |
| --- | --- | --- | --- | --- | --- |
| P0 | 0 | 0 | 0 | 0 | 0 |
| P1 | 1 | 0 | 0 | 1 | 2 |
| P2 | 0 | 0 | 1 | 0 | 1 |
| P3 | 0 | 0 | 0 | 0 | 0 |
| **All** | 1 | 0 | 1 | 1 | 3 |

| ID | Priority | Effort | Milestone | Category | Status | Item |
| --- | --- | --- | --- | --- | --- | --- |
| [T-001](area.md) | P1 | S | M3 | defect | open | The first entry |
| [T-002](area.md) | P2 | M | M4 | feature | blocked | The second entry |
| [T-003](area.md) | P1 | S | M3 | defect | done | The third entry |
";

const PROGRESS: &str = "# Progress

`TODO/INDEX.md` holds 3 entries: 1 open, 0 partial, 1 blocked, 1 done.

## Work order

1. T-001.

## Questions for the operator

- **Q1.** May the client listen?
";

const AREA: &str = "The entries of one area.

# T-001: The first entry

**Source:** the operator,
on two lines.
**Category:** defect
**Milestone:** M3
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

It fails; `crates/x/src/lib.rs:4` says \"line 4\".

## Premise

Read: `crates/x/src/lib.rs:3`.

## Approach

Change `crates/x/src/lib.rs:2-4` and `docs/roadmap.md`.

## Prove

```sh
cargo test -p x
# a comment in a fence, not an entry
```

# T-002: The second entry

**Source:** GitHub #1.
**Category:** feature
**Milestone:** M4
**Priority:** P2
**Effort:** M
**Status:** blocked

## Problem

It is missing.

## Premise

Read in the report, not verified here: `lablup/bssh:src/x.rs:9`.

## Approach

Add it, after T-001.

## Prove

Run `cargo test -p x -- second`.

## Blocker

The ruling of the operator on question Q1 in `TODO/PROGRESS.md`.

# T-003: The third entry

**Source:** found while measuring T-001.
**Category:** defect
**Milestone:** M3
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

It hung.

## Premise

Measured.

## Approach

Bound the wait.

## Prove

`cargo test -p x -- third`

## Done

2026-10-08, commit `0000000`: `cargo test -p x -- third` passed.
";

const ROADMAP: &str = "# Roadmap

## M3: Beta

- [x] Something that was done.
- T-001: the first entry.

## M4: Next

- T-002.
";

const SOURCE: &str = "line 1\nline 2\nline 3\nline 4\nline 5\n";

/// A fresh directory with the record, for one test. Dropping it removes it.
#[derive(Debug)]
struct Tree {
    root: PathBuf,
}

impl Tree {
    /// Each call has a directory of its own: two tests that run at once with
    /// one name must not remove the tree of the other.
    fn new(name: &str) -> Self {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let call = CALLS.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("xtask-todo-{}-{name}-{call}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let t = Self { root };
        t.write("TODO/INDEX.md", INDEX);
        t.write("TODO/PROGRESS.md", PROGRESS);
        t.write("TODO/area.md", AREA);
        t.write("docs/roadmap.md", ROADMAP);
        t.write("crates/x/src/lib.rs", SOURCE);
        t.write("README.md", "# x\n\nThe work order is in `TODO/PROGRESS.md` (T-001 first).\n");
        t
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().expect("a file of the tree has a parent")).expect("create the directory");
        fs::write(path, text).expect("write the file");
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.root.join(rel)).expect("read the file")
    }

    /// Replace `old` with `new` in one file. The plant must apply.
    fn plant(&self, rel: &str, old: &str, new: &str) {
        let text = self.read(rel);
        assert!(text.contains(old), "the plant does not apply to {rel}: {old:?}");
        self.write(rel, &text.replacen(old, new, 1));
    }

    fn problems(&self) -> Vec<String> {
        super::check::check(&self.root, None).problems.iter().map(ToString::to_string).collect()
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn root_arg(&self) -> String {
        self.root.display().to_string()
    }

    /// Run the command line on this tree (`--root` comes first).
    fn run(&self, args: &[&str]) -> (u8, String, String) {
        let root = self.root_arg();
        let mut all = vec!["--root", root.as_str()];
        all.extend_from_slice(args);
        run_args(&all)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Run the command line with these arguments: the exit code, the output and
/// the error output.
fn run_args(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().copied().map(String::from).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = super::run_to(&args, &mut out, &mut err);
    (code, String::from_utf8(out).expect("UTF-8 output"), String::from_utf8(err).expect("UTF-8 error output"))
}
