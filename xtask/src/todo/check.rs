//! The reader: it reads the index, each entry and the progress record, and
//! checks that they agree. It derives each count from the rows and trusts no
//! number that a file states.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use super::model::{
    CATEGORIES, Counts, EFFORTS, Entry, FIELDS, MILESTONES, PRIORITIES, Problem, RECORD_FILES, REQUIRED_SECTIONS, Row,
    SECTIONS, STATUSES, id_number, is_id,
};
use super::{parse, refs, remap};
use crate::repo::is_markdown;

pub(crate) const INDEX: &str = "TODO/INDEX.md";
pub(crate) const PROGRESS: &str = "TODO/PROGRESS.md";

/// What the reader found, the counts that the rows give, and the number of
/// entries that it read.
#[derive(Debug)]
pub(crate) struct Report {
    pub(crate) problems: Vec<Problem>,
    pub(crate) counts: Counts,
    pub(crate) entries: usize,
}

/// The record as files: the rows of the index and the entries of each area.
#[derive(Debug)]
pub(crate) struct Record {
    pub(crate) rows: Vec<Row>,
    pub(crate) entries: Vec<Entry>,
    pub(crate) index: String,
    pub(crate) progress: String,
}

/// Read a file of the tree as text, with `\r\n` read as `\n`.
pub(crate) fn read(root: &Path, rel: &str) -> Option<String> {
    fs::read_to_string(root.join(rel)).ok().map(|t| t.replace("\r\n", "\n"))
}

/// The area files of `TODO/`: each Markdown file that is not a record file.
pub(crate) fn area_files(root: &Path) -> Vec<String> {
    let Ok(dir) = fs::read_dir(root.join("TODO")) else { return Vec::new() };
    let mut out: Vec<String> = dir
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| is_markdown(n) && !RECORD_FILES.contains(&n.as_str()))
        .collect();
    out.sort();
    out
}

/// Load the record. The problems of form go to `problems`.
pub(crate) fn load(root: &Path, problems: &mut Vec<Problem>) -> Option<Record> {
    let Some(index) = read(root, INDEX) else {
        problems.push(Problem::new(INDEX, 0, "the index is missing"));
        return None;
    };
    let progress = read(root, PROGRESS).unwrap_or_else(|| {
        problems.push(Problem::new(PROGRESS, 0, "the progress record is missing"));
        String::new()
    });
    let (rows, mut found) = parse::rows(INDEX, &index);
    problems.append(&mut found);
    let mut entries = Vec::new();
    for name in area_files(root) {
        let rel = format!("TODO/{name}");
        let text = read(root, &rel).unwrap_or_default();
        let (mut more, mut found) = parse::entries(&rel, &text);
        for entry in &mut more {
            entry.file.clone_from(&name);
        }
        entries.append(&mut more);
        problems.append(&mut found);
    }
    Some(Record { rows, entries, index, progress })
}

/// Run each check of the reader on the tree at `root`. When git can say which
/// files changed since the base, also check that no citation of them was left
/// behind by a forgotten `remap`. The base is `HEAD`, or the revision that
/// `base` names: `HEAD~1` checks a commit that was made without the check, as
/// CI does. A base that is named and that git cannot read is a problem: the
/// check that it asks for did not run.
pub(crate) fn check(root: &Path, base: Option<&str>) -> Report {
    let rev = base.unwrap_or("HEAD");
    let head = |rel: &str| remap::git_at(root, rev, rel);
    let changed = remap::changed_since(root, rev);
    let unread = base.is_some() && changed.is_none();
    let mut report = check_with(root, changed, &head, rev);
    if unread {
        report.problems.push(Problem::new(
            "--base",
            0,
            format!(
                "git cannot compare the tree with `{rev}` here, so no citation that a change left behind can be found"
            ),
        ));
    }
    report
}

/// [`check`], given the files changed since the base `rev` (`None`: not
/// known) and their texts in it, so that a test needs no git.
pub(crate) fn check_with(root: &Path, changed: Option<Vec<String>>, head: &remap::Head, rev: &str) -> Report {
    let mut problems = Vec::new();
    let Some(record) = load(root, &mut problems) else {
        return Report { problems, counts: Counts::default(), entries: 0 };
    };
    if record.rows.is_empty() {
        problems.push(Problem::new(INDEX, 0, "the index has no rows"));
    }
    check_rows(&record.rows, &mut problems);
    check_agreement(&record.rows, &record.entries, &mut problems);
    for e in &record.entries {
        check_entry(e, &mut problems);
    }
    check_counts(&record, &mut problems);
    refs::check(root, &record, &mut problems);
    if let Some(files) = changed.filter(|f| !f.is_empty()) {
        unmoved(root, &files, head, rev, &mut problems);
    }
    Report { problems, counts: Counts::of(&record.rows, None), entries: record.entries.len() }
}

/// Find each citation that a remap would still move in a file that changed
/// since the base `rev`. Such a file was edited and its citations did not move,
/// so they name other lines now. The check of a cited line cannot see this,
/// because the line still exists. A citation that a person must read does not
/// count: only `remap` decides those, and it lists them. A remap that fails is a
/// problem, not a pass.
fn unmoved(root: &Path, files: &[String], head: &remap::Head, rev: &str, p: &mut Vec<Problem>) {
    let report = match remap::remap(root, files, head, false) {
        Ok(report) => report,
        Err(e) => {
            p.push(Problem::new(
                "remap",
                0,
                format!("the check of the citations of the files changed since {rev} could not run: {e}"),
            ));
            return;
        }
    };
    let command = if rev == "HEAD" { "cargo todo remap".to_string() } else { format!("cargo todo remap --base {rev}") };
    for moved in report.moved {
        // Each line is `doc:line: path old -> new`.
        let Some((at, rest)) = moved.split_once(": ") else { continue };
        let path = rest.split(' ').next().unwrap_or(rest);
        let (file, line) = at.rsplit_once(':').map_or((at, 0), |(doc, no)| (doc, no.parse().unwrap_or(0)));
        p.push(Problem::new(
            file,
            line,
            format!("cites {rest}, as {path} changed since {rev}: run `{command} {path}`"),
        ));
    }
}

fn check_rows(rows: &[Row], p: &mut Vec<Problem>) {
    let allowed = |what: &str, value: &str, set: &[&str], line: usize, p: &mut Vec<Problem>| {
        if !set.contains(&value) {
            p.push(Problem::new(INDEX, line, format!("{what} `{value}` is not one of {}", set.join(", "))));
        }
    };
    let mut last = 0;
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for r in rows {
        if !is_id(&r.id) {
            p.push(Problem::new(INDEX, r.line, format!("`{}` is not an id (T-NNN)", r.id)));
        }
        if let Some(first) = seen.insert(&r.id, r.line) {
            p.push(Problem::new(INDEX, r.line, format!("{} has a row already, on line {first}", r.id)));
        }
        let number = id_number(&r.id);
        if number <= last {
            p.push(Problem::new(INDEX, r.line, format!("{} is out of order: the rows are sorted by id", r.id)));
        }
        last = last.max(number);
        allowed("the priority", &r.priority, &PRIORITIES, r.line, p);
        allowed("the effort", &r.effort, &EFFORTS, r.line, p);
        allowed("the milestone", &r.milestone, &MILESTONES, r.line, p);
        allowed("the category", &r.category, &CATEGORIES, r.line, p);
        allowed("the status", &r.status, &STATUSES, r.line, p);
        if RECORD_FILES.contains(&r.file.as_str()) || r.file.contains('/') || !is_markdown(&r.file) {
            p.push(Problem::new(
                INDEX,
                r.line,
                format!("{} links `{}`, which is not an area file of TODO/", r.id, r.file),
            ));
        }
        if r.title.is_empty() {
            p.push(Problem::new(INDEX, r.line, format!("{} has no title", r.id)));
        }
    }
}

/// Each row names an entry in the file that it links, with the same fields;
/// each entry has a row; no id is used twice.
fn check_agreement(rows: &[Row], entries: &[Entry], p: &mut Vec<Problem>) {
    let mut by_id: HashMap<&str, &Entry> = HashMap::new();
    for e in entries {
        if let Some(first) = by_id.insert(&e.id, e) {
            p.push(Problem::new(
                &format!("TODO/{}", e.file),
                e.line,
                format!("{} is also an entry in TODO/{}:{}", e.id, first.file, first.line),
            ));
        }
    }
    for r in rows {
        let Some(e) = by_id.get(r.id.as_str()) else {
            p.push(Problem::new(INDEX, r.line, format!("{} has a row and no entry in TODO/{}", r.id, r.file)));
            continue;
        };
        let at = format!("TODO/{}", e.file);
        if e.file != r.file {
            p.push(Problem::new(
                &at,
                e.line,
                format!("{} is in TODO/{}, but its row links TODO/{}", r.id, e.file, r.file),
            ));
        }
        if e.title != r.title {
            p.push(Problem::new(&at, e.line, format!("{}: the title differs from the row's: \"{}\"", r.id, r.title)));
        }
        for (name, want) in [
            ("Priority", &r.priority),
            ("Effort", &r.effort),
            ("Milestone", &r.milestone),
            ("Category", &r.category),
            ("Status", &r.status),
        ] {
            if let Some(have) = e.field(name)
                && have != want.as_str()
            {
                p.push(Problem::new(
                    &at,
                    e.line,
                    format!("{}: the entry says {name} {have}, the index says {want}", r.id),
                ));
            }
        }
    }
    let with_row: HashSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    for e in entries {
        if !with_row.contains(e.id.as_str()) {
            p.push(Problem::new(&format!("TODO/{}", e.file), e.line, format!("{} has no row in the index", e.id)));
        }
    }
}

/// The form of one entry: its fields, its sections, and what its status
/// requires.
fn check_entry(e: &Entry, p: &mut Vec<Problem>) {
    let at = format!("TODO/{}", e.file);
    for name in FIELDS {
        match e.fields.iter().filter(|f| f.name == name).count() {
            0 => p.push(Problem::new(&at, e.line, format!("{}: the field {name} is missing", e.id))),
            1 => {}
            _ => p.push(Problem::new(&at, e.line, format!("{}: the field {name} is given more than once", e.id))),
        }
    }
    for field in &e.fields {
        let set: &[&str] = match field.name.as_str() {
            "Category" => &CATEGORIES,
            "Milestone" => &MILESTONES,
            "Priority" => &PRIORITIES,
            "Effort" => &EFFORTS,
            "Status" => &STATUSES,
            _ => continue,
        };
        if !set.contains(&field.value.as_str()) {
            let what = format!("{}: {} `{}` is not one of {}", e.id, field.name, field.value, set.join(", "));
            p.push(Problem::new(&at, field.line, what));
        }
    }
    if e.field("Source").is_some_and(str::is_empty) {
        p.push(Problem::new(&at, e.line, format!("{}: the source is empty", e.id)));
    }
    check_sections(e, &at, p);
    let status = e.field("Status").unwrap_or("");
    match (status, e.section("Blocker")) {
        ("blocked", None) => p.push(Problem::new(&at, e.line, format!("{}: blocked, with no `## Blocker`", e.id))),
        (other, Some(blocker)) if other != "blocked" => {
            p.push(Problem::new(&at, blocker.line, format!("{}: a `## Blocker` on an entry that is {other}", e.id)));
        }
        _ => {}
    }
    match (status, e.section("Done")) {
        ("done", None) => p.push(Problem::new(&at, e.line, format!("{}: done, with no `## Done` evidence", e.id))),
        ("done" | "partial", Some(done)) if !has_date(&done.body) => {
            p.push(Problem::new(&at, done.line, format!("{}: `## Done` gives no date (YYYY-MM-DD)", e.id)));
        }
        ("open" | "blocked", Some(done)) => {
            p.push(Problem::new(&at, done.line, format!("{}: a `## Done` on an entry that is {status}", e.id)));
        }
        _ => {}
    }
}

/// The sections of one entry. Each is known, in the order of [`SECTIONS`], and
/// not empty. The required sections are there, and a Prove holds a command.
fn check_sections(e: &Entry, at: &str, p: &mut Vec<Problem>) {
    let mut last = None;
    for section in &e.sections {
        let (name, line) = (&section.name, section.line);
        let Some(pos) = SECTIONS.iter().position(|known| known == name) else {
            p.push(Problem::new(at, line, format!("{}: `## {name}` is not a section of an entry", e.id)));
            continue;
        };
        if last.is_some_and(|before| pos <= before) {
            p.push(Problem::new(at, line, format!("{}: `## {name}` is out of order or repeated", e.id)));
        }
        last = Some(pos);
        if section.body.trim().is_empty() {
            p.push(Problem::new(at, line, format!("{}: `## {name}` is empty", e.id)));
        }
    }
    for name in REQUIRED_SECTIONS {
        if e.section(name).is_none() {
            p.push(Problem::new(at, e.line, format!("{}: the section `## {name}` is missing", e.id)));
        }
    }
    if let Some(prove) = e.section("Prove")
        && !prove.body.contains('`')
    {
        p.push(Problem::new(at, prove.line, format!("{}: a Prove with no command is a paragraph", e.id)));
    }
}

/// True when a text holds a date of the form YYYY-MM-DD.
pub(crate) fn has_date(text: &str) -> bool {
    text.as_bytes().windows(10).any(|w| {
        w[..4].iter().all(u8::is_ascii_digit)
            && w[4] == b'-'
            && w[5..7].iter().all(u8::is_ascii_digit)
            && w[7] == b'-'
            && w[8..].iter().all(u8::is_ascii_digit)
    })
}

/// The counts that the index and the progress record state, against the rows.
fn check_counts(r: &Record, p: &mut Vec<Problem>) {
    let all = Counts::of(&r.rows, None);
    check_phrase(INDEX, &r.index, all, p);
    for label in PRIORITIES.iter().copied().chain(["All"]) {
        let want = Counts::of(&r.rows, (label != "All").then_some(label));
        let found = parse::lines_with_fences(&r.index).into_iter().find_map(|(no, line, fenced)| {
            let row = if fenced { None } else { parse::counts_row(line) };
            row.filter(|(name, ..)| name == label).map(|hit| (no, hit))
        });
        match found {
            Some((no, (_, stated, total))) if stated != want || total != want.total() => p.push(Problem::new(
                INDEX,
                no,
                format!(
                    "the {label} row of the counts table disagrees with the rows; it reads: {}",
                    want.table_row(label)
                ),
            )),
            Some(_) => {}
            None => p.push(Problem::new(INDEX, 0, format!("the counts table has no {label} row"))),
        }
    }
    check_phrase(PROGRESS, &r.progress, all, p);
}

/// The counts line of one file, against the counts of all the rows.
fn check_phrase(file: &str, text: &str, all: Counts, p: &mut Vec<Problem>) {
    match parse::find_counts(text) {
        Some((line, total, c)) if total != all.total() || c != all => p.push(Problem::new(
            file,
            line,
            format!("the counts line says {total} entries ({c:?}); the rows give {}", all.phrase()),
        )),
        Some(_) => {}
        None => p.push(Problem::new(file, 0, format!("no counts line; it reads: {}", all.phrase()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_is_four_two_and_two_digits() {
        assert!(has_date("2026-10-14, commit 0000000"));
        assert!(has_date("on 2026-10-14"));
        assert!(!has_date("2026-1-14"));
        assert!(!has_date("today"));
        assert!(!has_date(""));
    }
}
