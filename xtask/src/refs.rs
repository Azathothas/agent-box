//! `cargo xtask refs list [--json]` and `cargo xtask refs fetch [--json] [owner/repo...]`.
//!
//! The defect this module exists to catch: a citation that nobody can check.
//! A citation names a commit of another repository. `refs fetch` puts that
//! commit in .refs/ and proves that HEAD is the pin (TEMPLATE
//! scripts/common/mine-repo.sh). It clones, fetches and checks out only. It
//! never runs code from a fetched tree, and it never deletes a directory.

mod table;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::json;

use crate::proc;
use crate::repo::Repo;
use crate::report::{self, CheckReport, Finding};
use table::{Row, Table};

/// The file that holds the pins table.
const REFERENCES: &str = "docs/references.md";

/// Where the clones go. git ignores it, so a clone is never staged.
const REFS_DIR: &str = ".refs";

const USAGE: &str = "usage: cargo xtask refs list [--json]
       cargo xtask refs fetch [--json] [owner/repo...]

list   print the pins table of docs/references.md and the state of each clone
fetch  clone each pin (or the named ones) into .refs/<owner>__<repo>, check out
       the pinned commit, and verify that HEAD is the pin. A row whose commit
       is not 40 hex digits is listed and skipped.";

pub(crate) fn run(args: &[String]) -> u8 {
    let rest = args.get(1..).unwrap_or(&[]);
    match args.first().map(String::as_str) {
        Some("list") => list(rest),
        Some("fetch") => fetch(rest),
        Some("-h" | "--help") => {
            println!("{USAGE}");
            report::EXIT_PASS
        }
        Some(other) => {
            eprintln!("xtask refs: unknown subcommand `{other}`\n\n{USAGE}");
            report::EXIT_USAGE
        }
        None => {
            eprintln!("xtask refs: name a subcommand\n\n{USAGE}");
            report::EXIT_USAGE
        }
    }
}

/// Read and parse the table. An error is the reason the command cannot run.
fn load(repo: &Repo) -> Result<Table, String> {
    if !repo.path(REFERENCES).is_file() {
        return Err(format!("{REFERENCES} does not exist yet, so there is no pins table to read"));
    }
    let text = repo.read_text(REFERENCES)?;
    let table = table::parse(&text).map_err(|e| format!("{REFERENCES}: {e}"))?;
    if table.rows.is_empty() && table.problems.is_empty() {
        return Err(format!("{REFERENCES}: the `{}` table has no rows", table::HEADING));
    }
    Ok(table)
}

/// The repository and the table, or the report that says why there are none.
fn setup(as_json: bool) -> Result<(Repo, Table), u8> {
    let not_run = |reason: String| report::emit(&[CheckReport::not_run("refs", reason)], as_json);
    let repo = Repo::find().map_err(not_run)?;
    let table = load(&repo).map_err(not_run)?;
    Ok((repo, table))
}

fn list(args: &[String]) -> u8 {
    let mut as_json = false;
    for a in args {
        if a == "--json" {
            as_json = true;
        } else {
            eprintln!("xtask refs list: unknown argument `{a}`\n\n{USAGE}");
            return report::EXIT_USAGE;
        }
    }
    let (repo, table) = match setup(as_json) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let states: Vec<String> = table.rows.iter().map(|r| local_state(&repo, r)).collect();
    if as_json {
        let doc = json!({
            "file": REFERENCES,
            "pins": table.rows.iter().zip(&states).map(|(r, s)| json!({
                "line": r.line,
                "reference": r.repo,
                "label": r.label,
                "commit": r.commit,
                "commit_cell": r.commit_cell,
                "date": r.date,
                "reason": r.reason,
                "dir": if r.commit.is_some() { format!("{REFS_DIR}/{}", r.dir) } else { String::new() },
                "state": s,
            })).collect::<Vec<_>>(),
            "problems": table.problems.iter().map(|(l, m)| json!({"line": l, "message": m})).collect::<Vec<_>>(),
        });
        println!("{doc:#}");
    } else {
        let fetchable = table.rows.iter().filter(|r| r.commit.is_some()).count();
        println!("refs: {} pins in {REFERENCES}, {fetchable} with a commit", table.rows.len());
        for (r, s) in table.rows.iter().zip(&states) {
            let name = if r.label.is_empty() { r.repo.clone() } else { format!("{} {}", r.repo, r.label) };
            let commit = r.commit.as_deref().unwrap_or("-");
            println!("  {name:40} {commit:40} {:10} {s}", r.date);
        }
        for (line, message) in &table.problems {
            println!("  problem: {REFERENCES}:{line}: {message}");
        }
    }
    if table.problems.is_empty() { report::EXIT_PASS } else { report::EXIT_FAIL }
}

/// What .refs/ holds for one pin. It reads only.
fn local_state(repo: &Repo, row: &Row) -> String {
    let Some(sha) = &row.commit else {
        return format!("skipped: `{}` is not a 40-hex commit", row.commit_cell);
    };
    let dest = repo.path(&format!("{REFS_DIR}/{}", row.dir));
    if !dest.exists() {
        return "absent".to_string();
    }
    if !dest.join(".git").is_dir() {
        return format!("{REFS_DIR}/{} is not a git clone", row.dir);
    }
    match git_in(&dest, &["rev-parse", "HEAD"], proc::QUICK) {
        Ok(head) if head == *sha => "fetched, HEAD is the pin".to_string(),
        Ok(head) => format!("fetched, HEAD {} is not the pin", head.get(..12).unwrap_or(&head)),
        Err(e) => format!("fetched, HEAD unknown: {e}"),
    }
}

fn fetch(args: &[String]) -> u8 {
    let mut as_json = false;
    let mut names: Vec<&str> = Vec::new();
    for a in args {
        match a.as_str() {
            "--json" => as_json = true,
            flag if flag.starts_with('-') => {
                eprintln!("xtask refs fetch: unknown option `{flag}`\n\n{USAGE}");
                return report::EXIT_USAGE;
            }
            name => names.push(name),
        }
    }
    let (repo, table) = match setup(as_json) {
        Ok(v) => v,
        Err(code) => return code,
    };
    for n in &names {
        if !table.rows.iter().any(|r| r.repo.eq_ignore_ascii_case(n)) {
            eprintln!("xtask refs fetch: no pin names `{n}`; `cargo xtask refs list` prints the pins");
            return report::EXIT_USAGE;
        }
    }
    if let Err(reason) = preflight(&repo) {
        return report::emit(&[CheckReport::not_run("refs", reason)], as_json);
    }
    let mut findings = Vec::new();
    let mut notes = Vec::new();
    // A broken row of the table fails a run over the whole table. A run over
    // named pins reads only those rows.
    if names.is_empty() {
        for (line, message) in &table.problems {
            findings.push(Finding::new(REFERENCES, *line, message.clone()));
        }
    }
    let mut scanned = 0;
    let selected =
        table.rows.iter().filter(|r| names.is_empty() || names.iter().any(|n| r.repo.eq_ignore_ascii_case(n)));
    for row in selected {
        let Some(sha) = &row.commit else {
            notes.push(format!("{}: skipped, `{}` is not a 40-hex commit", row.repo, row.commit_cell));
            continue;
        };
        scanned += 1;
        match fetch_one(&repo, row, sha) {
            Ok(note) => notes.push(note),
            Err(message) => findings.push(Finding::new(format!("{REFS_DIR}/{}", row.dir), 0, message)),
        }
    }
    let mut report = CheckReport::from_findings("refs", scanned, 1, findings);
    report.notes.extend(notes);
    report::emit(&[report], as_json)
}

/// git must answer, and git must ignore .refs/: a clone that is not ignored
/// is one `git add -A` away from the index of this repository.
fn preflight(repo: &Repo) -> Result<(), String> {
    if proc::probe_version("git", &["--version"]).is_none() {
        return Err("git does not answer `git --version`".to_string());
    }
    let pattern = format!("{REFS_DIR}/");
    let out = proc::run_in(&repo.root, "git", &["check-ignore", "-q", &pattern], proc::QUICK)?;
    match out.code {
        Some(0) => Ok(()),
        Some(1) => Err(format!("git does not ignore {pattern}; add `{pattern}` to .gitignore before a fetch")),
        _ => Err(format!("git check-ignore failed: {}", out.stderr.trim())),
    }
}

/// Clone (or reuse) one pin, check out its commit and verify HEAD.
fn fetch_one(repo: &Repo, row: &Row, sha: &str) -> Result<String, String> {
    let rel = format!("{REFS_DIR}/{}", row.dir);
    let dest = repo.path(&rel);
    let url = format!("https://github.com/{}", row.repo);
    eprintln!("xtask refs: {} at {} into {rel}", row.repo, &sha[..12]);
    let action = if dest.exists() {
        if !dest.join(".git").is_dir() {
            return Err(format!("{rel} exists and is not a git clone; move it away (this command deletes nothing)"));
        }
        let origin = git_in(&dest, &["config", "--get", "remote.origin.url"], proc::QUICK)?;
        if !same_url(&origin, &url) {
            return Err(format!("{rel} is a clone of {origin}, not of {url}; move it away"));
        }
        "reused the clone"
    } else {
        let parent = repo.path(REFS_DIR);
        std::fs::create_dir_all(&parent).map_err(|e| format!("{REFS_DIR}: {e}"))?;
        git(&parent, &["clone", "--filter=blob:none", "--no-checkout", &url, &row.dir], proc::LONG)?;
        "cloned"
    };
    git_in(&dest, &["fetch", "--depth", "1", "origin", sha], proc::LONG)?;
    git_in(&dest, &["-c", "advice.detachedHead=false", "checkout", "--detach", sha], proc::LONG)?;
    let head = git_in(&dest, &["rev-parse", "HEAD"], proc::QUICK)?;
    if head != sha {
        return Err(format!("HEAD is {head} after the checkout, not the pin {sha}"));
    }
    Ok(format!("{}: {action}, HEAD is the pin {sha}", row.repo))
}

/// git in one clone. `--git-dir` and `--work-tree` name the clone, so git
/// never walks up into the repository that holds .refs/ and acts on it.
fn git_in(dest: &Path, args: &[&str], limit: Duration) -> Result<String, String> {
    let git_dir = format!("--git-dir={}", dest.join(".git").display());
    let work_tree = format!("--work-tree={}", dest.display());
    let mut all = vec![git_dir.as_str(), work_tree.as_str()];
    all.extend_from_slice(args);
    git(dest, &all, limit)
}

/// Run git with no prompt: a credential prompt (or a credential window on
/// Windows) waits for a person, and nobody is there.
fn git(dir: &Path, args: &[&str], limit: Duration) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir).env("GIT_TERMINAL_PROMPT", "0").env("GCM_INTERACTIVE", "never");
    let shown = args.iter().filter(|a| !a.starts_with("--git-dir=") && !a.starts_with("--work-tree=")).copied();
    let shown = shown.collect::<Vec<_>>().join(" ");
    let out = proc::run_command(&mut cmd, limit).map_err(|e| format!("git does not start: {e}"))?;
    if out.timed_out {
        return Err(format!("git {shown} gave no answer in {}s", limit.as_secs()));
    }
    if !out.success() {
        let why = out.stderr.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("no message");
        return Err(format!("git {shown} failed: {why}"));
    }
    Ok(out.stdout.trim().to_string())
}

/// Two remote URLs name the same repository: no case, no `.git`, no `/` at the end.
fn same_url(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let s = s.trim().trim_end_matches('/');
        s.strip_suffix(".git").unwrap_or(s).to_ascii_lowercase()
    };
    norm(a) == norm(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_compare_without_case_suffix_or_slash() {
        assert!(same_url("https://github.com/QaidVoid/kage.git", "https://github.com/qaidvoid/kage"));
        assert!(same_url("https://github.com/a/b/", "https://github.com/a/b"));
        assert!(!same_url("https://github.com/a/b", "https://github.com/a/c"));
    }
}
