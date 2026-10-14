//! The writer: it moves the status of an entry in the entry and in its row,
//! then derives each count from the rows again. Nobody types a count by hand.
//! The reader then checks the result on its own.

use std::fs;
use std::path::Path;

use super::check::{INDEX, PROGRESS, has_date, read};
use super::model::{Counts, Entry, PRIORITIES, Row, STATUSES, id_number};
use super::parse;

/// Write a text back with the line endings that the file had.
fn write_like(root: &Path, rel: &str, text: &str) -> Result<(), String> {
    let path = root.join(rel);
    let crlf = fs::read(&path).is_ok_and(|b| b.windows(2).any(|w| w == b"\r\n"));
    let out = if crlf { text.replace('\n', "\r\n") } else { text.to_string() };
    fs::write(&path, out).map_err(|e| format!("{rel}: {e}"))
}

/// Replace the value of one header field of one entry in an area file.
fn set_field(text: &str, id: &str, field: &str, value: &str) -> Result<String, String> {
    let prefix = format!("**{field}:**");
    let mut out = String::with_capacity(text.len());
    let mut in_entry = false;
    let mut done = false;
    for (_, line, fenced) in parse::lines_with_fences(text) {
        if !fenced && let Some(head) = line.strip_prefix("# ") {
            in_entry = head.split_once(": ").is_some_and(|(this, _)| this == id);
        }
        if in_entry && !done && !fenced && line.starts_with(prefix.as_str()) {
            out.push_str(&prefix);
            out.push(' ');
            out.push_str(value);
            done = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if done { Ok(out) } else { Err(format!("{id} has no **{field}:** line")) }
}

/// The text with its first counts phrase (outside a fence) written again from
/// `all`.
fn fix_phrase(text: &str, all: Counts) -> String {
    let mut done = false;
    let mut out = String::with_capacity(text.len());
    for (_, line, fenced) in parse::lines_with_fences(text) {
        let found = if fenced || done { None } else { parse::counts_phrase(line) };
        if let Some((_, _, range)) = found {
            out.push_str(&line[..range.start]);
            out.push_str(&all.phrase());
            out.push_str(&line[range.end..]);
            done = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Write the counts phrase, the counts table and the progress line again from
/// the rows. Returns the new index and progress texts.
pub(crate) fn recount(index: &str, progress: &str, rows: &[Row]) -> (String, String) {
    let all = Counts::of(rows, None);
    let mut new_index = String::with_capacity(index.len());
    for (_, line, fenced) in parse::lines_with_fences(&fix_phrase(index, all)) {
        let row = if fenced { None } else { parse::counts_row(line) };
        match row {
            Some((label, ..)) if PRIORITIES.contains(&label.as_str()) => {
                new_index.push_str(&Counts::of(rows, Some(&label)).table_row(&label));
            }
            Some((label, ..)) if label == "All" => new_index.push_str(&all.table_row("**All**")),
            _ => new_index.push_str(line),
        }
        new_index.push('\n');
    }
    (new_index, fix_phrase(progress, all))
}

/// Refuse a status that the entry cannot support yet: `done` needs the
/// evidence first, and `blocked` the blocker.
fn refuse(entry: &Entry, id: &str, status: &str) -> Result<(), String> {
    let blocker = entry.section("Blocker").is_some();
    let done = entry.section("Done");
    match status {
        "done" if done.is_none_or(|d| !has_date(&d.body)) => {
            Err(format!("{id}: write the evidence first, in `## Done` with its date"))
        }
        "blocked" if !blocker => Err(format!("{id}: write what blocks it first, in `## Blocker`")),
        other if other != "blocked" && blocker => {
            Err(format!("{id}: the entry still has `## Blocker`; record how it cleared, then remove it"))
        }
        other if !matches!(other, "done" | "partial") && done.is_some() => {
            Err(format!("{id}: the entry has `## Done`; move that text to `## Correction` to open it again"))
        }
        _ => Ok(()),
    }
}

/// Move the status of `id` to `status`, in its entry and its row, and derive
/// the counts again. The writer refuses a status that the entry cannot
/// support.
pub(crate) fn set_status(root: &Path, id: &str, status: &str) -> Result<(), String> {
    if !STATUSES.contains(&status) {
        return Err(format!("`{status}` is not a status; use one of {}", STATUSES.join(", ")));
    }
    let index = read(root, INDEX).ok_or("TODO/INDEX.md is missing")?;
    let progress = read(root, PROGRESS).ok_or("TODO/PROGRESS.md is missing")?;
    let (mut rows, problems) = parse::rows(INDEX, &index);
    if let Some(p) = problems.first() {
        return Err(format!("the index must be read first: {p}"));
    }
    let row = rows.iter_mut().find(|r| r.id == id).ok_or_else(|| format!("{id} has no row in the index"))?;
    let area = format!("TODO/{}", row.file);
    let text = read(root, &area).ok_or_else(|| format!("{area} is missing"))?;
    let (entries, _) = parse::entries(&area, &text);
    let entry = entries.iter().find(|e| e.id == id).ok_or_else(|| format!("{id} is not an entry of {area}"))?;
    refuse(entry, id, status)?;
    // The row was read with 7 cells, so cell 5 (the status) exists.
    let mut cells = parse::cells(index.lines().nth(row.line - 1).unwrap_or_default());
    cells[5] = status.to_string();
    let new_line = parse::table_line(&cells);
    row.status = status.to_string();
    let row_line = row.line;

    let new_area = set_field(&text, id, "Status", status)?;
    let mut index_with_row = String::with_capacity(index.len());
    for (idx, line) in index.lines().enumerate() {
        index_with_row.push_str(if idx + 1 == row_line { new_line.as_str() } else { line });
        index_with_row.push('\n');
    }
    let (new_index, new_progress) = recount(&index_with_row, &progress, &rows);
    write_like(root, &area, &new_area)?;
    write_like(root, INDEX, &new_index)?;
    write_like(root, PROGRESS, &new_progress)
}

/// Derive the counts again after a person added or removed rows.
pub(crate) fn counts(root: &Path) -> Result<(), String> {
    let index = read(root, INDEX).ok_or("TODO/INDEX.md is missing")?;
    let progress = read(root, PROGRESS).ok_or("TODO/PROGRESS.md is missing")?;
    let (rows, problems) = parse::rows(INDEX, &index);
    if let Some(p) = problems.first() {
        return Err(format!("the index must be read first: {p}"));
    }
    let (new_index, new_progress) = recount(&index, &progress, &rows);
    write_like(root, INDEX, &new_index)?;
    write_like(root, PROGRESS, &new_progress)
}

/// The next free id.
pub(crate) fn next_id(root: &Path) -> Result<String, String> {
    let index = read(root, INDEX).ok_or("TODO/INDEX.md is missing")?;
    let (rows, _) = parse::rows(INDEX, &index);
    let max = rows.iter().map(|r| id_number(&r.id)).max().unwrap_or(0);
    Ok(format!("T-{:03}", max + 1))
}
