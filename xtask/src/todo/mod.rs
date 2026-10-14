//! `cargo xtask todo`, and its alias `cargo todo`: the checker and the writer of
//! the work record in `TODO/`. It is a port of the podssh-todo crate of the
//! sibling repository podssh (0BSD): copied from
//! `Azathothas/podssh@3ad320382891d1d5697aadedd689ed07cd962e4f:crates/podssh-todo/src/`
//! and adapted (constants, paths, no interop module). Entry T-016 decides
//! whether the two copies become one shared crate.
//!
//! The defect this module exists to catch: a work record whose files disagree.
//! Closing one entry moves several numbers in two files. A count, a status or a
//! title typed by hand goes stale. A citation of a line goes stale when its file
//! changes.
//!
//! So nobody types a number: the writer moves a status and derives the counts.
//! The reader, which the gate runs, checks that the files agree. It also checks
//! that each id, path and line that a document cites exists. When a cited file
//! changes, `remap` moves its citations by a line diff against `HEAD`, or
//! against the revision that `--base` names.
//!
//! The record is the index (`TODO/INDEX.md`), the progress record with the work
//! order (`TODO/PROGRESS.md`), the rules (`TODO/RULES.md`), the upstream requests
//! (`TODO/upstream.md`) and the entries (`TODO/<area>.md`).

mod check;
mod diff;
mod model;
mod numbers;
mod parse;
mod refs;
mod remap;
mod write;

#[cfg(test)]
mod tests;

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::repo::Repo;
use crate::report::{self, CheckReport, Finding, Status};

pub(crate) const DEFECT: &str =
    "the work record in TODO/ disagrees with itself, or cites an entry, a file or a line that does not exist";

/// The name of the check in the gate.
const NAME: &str = "todo";

/// A record with no entry is not a pass.
const FLOOR: usize = 1;

const USAGE: &str = "usage: cargo xtask todo check [--json]
       cargo xtask todo set T-NNN open|partial|blocked|done
       cargo xtask todo counts
       cargo xtask todo next
       cargo xtask todo remap [--dry-run] FILE...   (after an edit of FILE: move its citations by a diff against the base)
FILE is a path from the root of the repository, as a citation writes it.
Options: --root DIR (default: this repository; for tests)
         --base REV (default: HEAD): check and remap compare each file with its text in REV,
                    as HEAD~1 for a commit that was made without them
`cargo todo ...` is short for `cargo xtask todo ...`.
Exit codes: 0 pass, 1 fail, 2 could not run, 64 usage error.";

/// The reader as a check of the gate. A tree with no `TODO/INDEX.md` has no
/// record to read: the check did not run.
pub(crate) fn check(repo: &Repo) -> CheckReport {
    if !repo.path(check::INDEX).is_file() {
        return CheckReport::not_run(NAME, format!("{} is missing: there is no record to read", check::INDEX));
    }
    as_report(check::check(&repo.root, None))
}

/// The result of the reader as a report: each problem is a finding, and the
/// number of entries read is the scope.
fn as_report(found: check::Report) -> CheckReport {
    let phrase = found.counts.phrase();
    let findings = found.problems.into_iter().map(|p| Finding::new(p.file, p.line, p.what)).collect();
    CheckReport::from_findings(NAME, found.entries, FLOOR, findings).note(format!("the rows give {phrase}"))
}

/// `cargo xtask todo ARGS`: returns the exit code.
pub(crate) fn run(args: &[String]) -> u8 {
    run_to(args, &mut std::io::stdout(), &mut std::io::stderr())
}

/// [`run`], with the two output streams given, so that a test can read them.
fn run_to(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut words: Vec<&str> = Vec::new();
    let mut root: Option<PathBuf> = None;
    let mut base: Option<&str> = None;
    let mut as_json = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--root" => {
                let Some(dir) = it.next() else { return usage(err, "--root needs a directory") };
                root = Some(PathBuf::from(dir));
            }
            // A revision that starts with `-` would reach git as an option.
            "--base" => match it.next() {
                Some(rev) if !rev.is_empty() && !rev.starts_with('-') => base = Some(rev.as_str()),
                _ => return usage(err, "--base needs a revision, as HEAD~1"),
            },
            "--json" => as_json = true,
            "-h" | "--help" | "help" => {
                let _ = writeln!(out, "{USAGE}");
                return report::EXIT_PASS;
            }
            word => words.push(word),
        }
    }
    let found = root.map_or_else(|| Repo::find().map(|r| r.root), Ok);
    let root = match found {
        Ok(root) => root,
        Err(e) => {
            let _ = writeln!(err, "xtask todo: {e}");
            return report::EXIT_NOT_RUN;
        }
    };
    if as_json && words.as_slice() != ["check"] {
        return usage(err, "--json goes with check only");
    }
    match words.as_slice() {
        ["check"] => print_report(&root, base, as_json, out, err),
        ["set", id, status] => match write::set_status(&root, id, status) {
            Ok(()) => print_report(&root, base, false, out, err),
            Err(e) => refused(err, &e),
        },
        ["counts"] => match write::counts(&root) {
            Ok(()) => print_report(&root, base, false, out, err),
            Err(e) => refused(err, &e),
        },
        ["next"] => match write::next_id(&root) {
            Ok(id) => {
                let _ = writeln!(out, "{id}");
                report::EXIT_PASS
            }
            Err(e) => refused(err, &e),
        },
        ["remap", rest @ ..] => {
            let dry = rest.iter().any(|w| *w == "--dry-run" || *w == "-n");
            let files: Vec<String> = rest.iter().copied().filter(|w| !w.starts_with('-')).map(String::from).collect();
            if files.is_empty() || rest.iter().any(|w| w.starts_with('-') && *w != "--dry-run" && *w != "-n") {
                return usage(err, "remap needs one FILE or more, and takes only --dry-run");
            }
            remap_command(&root, &files, dry, base, out, err)
        }
        [] => usage(err, "name a command"),
        _ => usage(err, "unknown command"),
    }
}

/// The writer refused: print why, and fail.
fn refused(err: &mut dyn Write, why: &str) -> u8 {
    let _ = writeln!(err, "xtask todo: {why}");
    report::EXIT_FAIL
}

/// Move the citations of `files`, print what moved and what a person must
/// read, then check the record (unless nothing was written).
fn remap_command(
    root: &Path,
    files: &[String],
    dry: bool,
    base: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let rev = base.unwrap_or("HEAD");
    let head = |rel: &str| remap::git_at(root, rev, rel);
    let result = match remap::remap(root, files, &head, !dry) {
        Ok(result) => result,
        Err(e) => {
            let _ = writeln!(err, "xtask todo: remap could not run: {e}");
            return report::EXIT_NOT_RUN;
        }
    };
    for line in &result.notes {
        let _ = writeln!(out, "note: {line}");
    }
    for line in &result.moved {
        let _ = writeln!(out, "moved: {line}");
    }
    for line in &result.kept {
        let _ = writeln!(out, "kept (new in this change): {line}");
    }
    for line in &result.review {
        let _ = writeln!(out, "REVIEW: {line}");
    }
    let verb = if dry { "would move" } else { "moved" };
    let _ = writeln!(
        out,
        "xtask todo: remap: {verb} {} citations; {} to review; {} on new lines kept",
        result.moved.len(),
        result.review.len(),
        result.kept.len()
    );
    if dry { report::EXIT_PASS } else { print_report(root, base, false, out, err) }
}

/// Run the reader and print what it found: as text, or as JSON with
/// `as_json`. Returns the exit code.
fn print_report(root: &Path, base: Option<&str>, as_json: bool, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let (rep, phrase) = if root.join(check::INDEX).is_file() {
        let found = check::check(root, base);
        let phrase = found.counts.phrase();
        (as_report(found), phrase)
    } else {
        let why = format!("{} has no {}: there is no record to read", root.display(), check::INDEX);
        (CheckReport::not_run(NAME, why), String::new())
    };
    if as_json {
        return report::emit(&[rep], true);
    }
    for f in &rep.findings {
        let _ = writeln!(err, "{}: {}", f.location(), f.message);
    }
    let _ = match rep.status {
        Status::Pass => writeln!(out, "xtask todo: the record agrees: {phrase}"),
        Status::Fail => writeln!(err, "xtask todo: {} problems in the record", rep.findings.len()),
        Status::NotRun => writeln!(err, "xtask todo: could not run: {}", rep.notes.join("; ")),
    };
    rep.status.exit_code()
}

fn usage(err: &mut dyn Write, why: &str) -> u8 {
    let _ = writeln!(err, "xtask todo: {why}\n{USAGE}");
    report::EXIT_USAGE
}
