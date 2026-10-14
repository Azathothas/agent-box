//! `remap FILE...`: after an edit of FILE, move each citation of it in the
//! documents. The move follows a line diff of FILE against its version in the
//! base. The base is `HEAD`, or the revision that `--base` names, as `HEAD~1`
//! for a commit that was made without a remap. A citation is `FILE:N` or
//! `FILE:N-M` in a code span. It is also a bare `:N` that follows a citation of
//! FILE in the same paragraph.
//!
//! Each document is compared with the base too. A line of it is built again from
//! its line in the base, where the numbers are the numbers of the base. So a
//! second run after a second edit gives the right numbers, not a second shift. A
//! line that is new in this change stays as it is: its writer used the new
//! numbers.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::ops::Range;
use std::path::Path;

use super::diff::line_map;
use super::parse::lines_with_fences;
use super::refs::{citation, documents};
use crate::proc;

/// The text of a file in the base; `Ok(None)` when the base has no such file.
pub(crate) type Head<'a> = dyn Fn(&str) -> Result<Option<String>, String> + 'a;

/// What a run moved, and what a person must read.
#[derive(Debug, Default)]
pub(crate) struct Report {
    /// Each citation that moved: `doc:line: path old -> new`.
    pub(crate) moved: Vec<String>,
    /// Each citation of a line that this change removed or changed. It does
    /// not move.
    pub(crate) review: Vec<String>,
    /// Each line that is new in this change and cites FILE. Its numbers are
    /// taken to be the new ones.
    pub(crate) kept: Vec<String>,
    pub(crate) notes: Vec<String>,
}

/// One citation on a line. It has the byte range of its numbers in the line, the
/// file, and the first and the last line. `moves` is true when its file is one
/// of the files of this run.
#[derive(Debug, Clone)]
struct Cite {
    range: Range<usize>,
    path: String,
    first: usize,
    last: Option<usize>,
    moves: bool,
}

/// The line map of a file: the new number of each old line, and which old
/// lines are blank. A blank line is no anchor: a diff can match it with any
/// other blank line, also one in a part that the edit added.
#[derive(Debug)]
struct FileMap {
    to: Vec<Option<usize>>,
    blank: Vec<bool>,
}

type Maps = HashMap<String, FileMap>;

/// The text of `rel` in the revision `rev`, read with `git show`. `Ok(None)`
/// means that the revision has no such file, or no such text file. A
/// citation cannot name a line of a file that is not text.
pub(crate) fn git_at(root: &Path, rev: &str, rel: &str) -> Result<Option<String>, String> {
    let spec = format!("{rev}:{rel}");
    let out = proc::run_in(root, "git", &["show", spec.as_str()], proc::QUICK)
        .map_err(|e| format!("git cannot run here ({e}); remap needs each file as it is in {rev}"))?;
    if out.timed_out {
        return Err(format!("git show {spec} gave no answer in {} seconds", proc::QUICK.as_secs()));
    }
    if out.success() {
        // The runner replaces each byte that is not UTF-8 with U+FFFD. The
        // tree is ASCII, so that character marks a file that is not text.
        let text = out.stdout;
        return Ok(if text.contains(char::REPLACEMENT_CHARACTER) { None } else { Some(text) });
    }
    let why = out.stderr;
    if why.contains("does not exist in") || why.contains("exists on disk, but not in") {
        Ok(None)
    } else {
        Err(format!("git show {spec}: {}", why.trim()))
    }
}

/// The files of the tree that differ from the revision `rev` and still exist,
/// by `git diff --name-only REV`. `None` means that git cannot say: there is
/// no git, no repository or no such revision.
pub(crate) fn changed_since(root: &Path, rev: &str) -> Option<Vec<String>> {
    let out = proc::run_in(root, "git", &["diff", "--name-only", rev, "--"], proc::QUICK).ok()?;
    if !out.success() {
        return None;
    }
    Some(out.stdout.lines().filter(|l| !l.is_empty() && root.join(l).is_file()).map(str::to_string).collect())
}

/// Move the citations of `files` in each document. With `write` false, only
/// report what would move.
pub(crate) fn remap(root: &Path, files: &[String], head: &Head, write: bool) -> Result<Report, String> {
    let mut report = Report::default();
    let mut maps: Maps = HashMap::new();
    for f in files {
        let rel = f.replace('\\', "/");
        let rel = rel.trim_start_matches("./").to_string();
        let bytes = fs::read(root.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
        // A file that is not text, as a binary fixture, has no lines to cite.
        // The run passes over it, so that the other files still move and the
        // check of a commit that changed it still runs.
        let Ok(now) = String::from_utf8(bytes) else {
            report.notes.push(format!("{rel} is not UTF-8 text: it has no lines to move"));
            continue;
        };
        if let Some(old) = head(&rel)? {
            let old: Vec<&str> = old.lines().collect();
            let now: Vec<&str> = now.lines().collect();
            let blank = old.iter().map(|l| l.trim().is_empty()).collect();
            maps.insert(rel, FileMap { to: line_map(&old, &now), blank });
        } else {
            report.notes.push(format!("{rel} has no text in the base: a new file has no old line numbers"));
        }
    }
    let names: HashSet<&str> = maps.keys().map(String::as_str).collect();
    if names.is_empty() {
        return Ok(report);
    }
    for doc in documents(root) {
        let Ok(now) = fs::read_to_string(root.join(&doc)) else { continue };
        // A bare `:N` needs a full citation earlier in its paragraph, so a
        // document that names no file has nothing to move.
        if !names.iter().any(|f| now.contains(f)) {
            continue;
        }
        let old = head(&doc)?.unwrap_or_default();
        if let Some(text) = remap_doc(&doc, &now, &old, &names, &maps, &mut report)
            && write
        {
            let crlf = now.contains("\r\n");
            let mut out = if crlf { text.replace('\n', "\r\n") } else { text };
            if !now.ends_with('\n') && out.ends_with('\n') {
                out.pop();
                if crlf {
                    out.pop();
                }
            }
            fs::write(root.join(&doc), out).map_err(|e| format!("{doc}: {e}"))?;
        }
    }
    Ok(report)
}

/// The new text of one document, when a citation in it moved.
fn remap_doc(
    doc: &str,
    now: &str,
    old: &str,
    names: &HashSet<&str>,
    maps: &Maps,
    report: &mut Report,
) -> Option<String> {
    let now_lines: Vec<&str> = now.lines().collect();
    let old_lines: Vec<&str> = old.lines().collect();
    let now_cites = cites(now, names);
    let old_cites = cites(old, names);
    // For each line of the document now, its line in the base when unchanged.
    let mut from: Vec<Option<usize>> = vec![None; now_lines.len()];
    for (old_no, mapped) in line_map(&old_lines, &now_lines).into_iter().enumerate() {
        if let Some(new_no) = mapped {
            from[new_no - 1] = Some(old_no);
        }
    }
    let mut used = HashSet::new();
    let mut moved = false;
    let mut out = String::with_capacity(now.len());
    for (idx, line) in now_lines.iter().enumerate() {
        let mine = &now_cites[idx];
        let source = if mine.iter().any(|c| c.moves) {
            from[idx].or_else(|| same_line(idx, &from, &old_lines, &old_cites, &key(line, mine), &mut used))
        } else {
            None
        };
        let new_line = if let Some(was) = source {
            let at = format!("{doc}:{}", idx + 1);
            rebuild(line, mine, old_lines[was], &old_cites[was], maps, &at, report)
        } else {
            for c in mine.iter().filter(|c| c.moves) {
                report.kept.push(format!("{doc}:{}: {} {}", idx + 1, c.path, &line[c.range.clone()]));
            }
            (*line).to_string()
        };
        moved |= new_line != *line;
        out.push_str(&new_line);
        out.push('\n');
    }
    moved.then_some(out)
}

/// A line of the base for a changed line `idx`. The search is in the block of the
/// base between the unchanged lines around `idx`. The result is the first unused
/// line with the same text, with the numbers of the citations set aside. Such a
/// line is one that a remap moved before. Another kind of change makes it a new
/// line.
fn same_line(
    idx: usize,
    from: &[Option<usize>],
    old_lines: &[&str],
    old_cites: &[Vec<Cite>],
    want: &str,
    used: &mut HashSet<usize>,
) -> Option<usize> {
    let lo = from[..idx].iter().rev().find_map(|o| *o).map_or(0, |o| o + 1);
    let hi = from[idx + 1..].iter().find_map(|o| *o).unwrap_or(old_lines.len());
    let found = (lo..hi)
        .find(|&o| !used.contains(&o) && !old_cites[o].is_empty() && key(old_lines[o], &old_cites[o]) == want)?;
    used.insert(found);
    Some(found)
}

/// A line with the numbers of each citation set aside. This includes the files
/// that this run does not move, because an earlier run in the same change can
/// have moved those.
fn key(line: &str, cites: &[Cite]) -> String {
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    for c in cites {
        out.push_str(&line[at..c.range.start]);
        out.push('#');
        at = c.range.end;
    }
    out.push_str(&line[at..]);
    out
}

/// A line with each citation moved by its numbers in the base. `was` is the same
/// line in the base. A citation of a line that this change removed or changed
/// keeps the numbers that it has now. When they are still the numbers of the
/// base, it is listed for review. Else a person moved it already.
fn rebuild(
    line: &str,
    mine: &[Cite],
    was: &str,
    theirs: &[Cite],
    maps: &Maps,
    at: &str,
    report: &mut Report,
) -> String {
    let mut out = String::with_capacity(line.len());
    let mut from = 0;
    for (c, h) in mine.iter().zip(theirs) {
        out.push_str(&line[from..c.range.start]);
        let now = &line[c.range.clone()];
        from = c.range.end;
        if !c.moves {
            out.push_str(now);
            continue;
        }
        if let Some((start, last)) = moved_range(h, maps) {
            let new = last.map_or_else(|| start.to_string(), |end| format!("{start}-{end}"));
            if new != now {
                report.moved.push(format!("{at}: {} {now} -> {new}", c.path));
            }
            if changed_inside(h, maps) {
                report
                    .review
                    .push(format!("{at}: {} {new}: moved by its ends, but a line inside it changed; read it", c.path));
            }
            out.push_str(&new);
        } else {
            let then = &was[h.range.clone()];
            if now == then {
                report.review.push(format!(
                    "{at}: {} {now}: names a line that this change removed or changed; read it, and move the citation by hand",
                    c.path
                ));
            }
            out.push_str(now);
        }
    }
    out.push_str(&line[from..]);
    out
}

/// The new first and last line of a citation, when the lines at its ends are
/// unchanged. A range moves by its ends, also when a line inside it changed, and
/// `changed_inside` lists that for review. So the range keeps naming the same
/// part of the file.
fn moved_range(c: &Cite, maps: &Maps) -> Option<(usize, Option<usize>)> {
    let map = maps.get(&c.path)?;
    let end = c.last.unwrap_or(c.first);
    if c.first == 0 || end < c.first || end > map.to.len() {
        return None;
    }
    // A range moves by its first and last lines with text, and keeps its
    // blank margins: a blank end could have matched any blank line.
    let text = |n: &usize| !map.blank[n - 1];
    let first = (c.first..=end).find(text).unwrap_or(c.first);
    let start = map.to[first - 1]?.checked_sub(first - c.first).filter(|n| *n > 0)?;
    let last = if let Some(last) = c.last {
        let at = (c.first..=last).rev().find(text).unwrap_or(last);
        Some(map.to[at - 1]? + (last - at))
    } else {
        None
    };
    Some((start, last))
}

/// Whether the change removed or changed a line inside a cited range.
fn changed_inside(c: &Cite, maps: &Maps) -> bool {
    let Some(map) = maps.get(&c.path) else { return false };
    let end = c.last.unwrap_or(c.first).min(map.to.len());
    (c.first.max(1)..=end).any(|n| map.to[n - 1].is_none())
}

/// The citations on each line of a text; those of `names` move. A blank
/// line, a heading or a fence ends a paragraph, and with it the file that a
/// bare `:N` means.
fn cites(text: &str, names: &HashSet<&str>) -> Vec<Vec<Cite>> {
    let mut out = Vec::new();
    let mut context: Option<String> = None;
    for (_, line, fenced) in lines_with_fences(text) {
        let mut here = Vec::new();
        if fenced || line.trim().is_empty() || line.starts_with('#') {
            context = None;
            out.push(here);
            continue;
        }
        for (start, span) in spans(line) {
            if let Some(nums) = span.strip_prefix(':') {
                if let (Some(path), Some((first, last))) = (context.as_ref(), numbers(nums)) {
                    let moves = names.contains(path.as_str());
                    here.push(Cite { range: start + 1..start + span.len(), path: path.clone(), first, last, moves });
                }
                continue;
            }
            let Some((path, _)) = citation(span) else { continue };
            context = Some(path.to_string());
            let Some((_, nums)) = span.split_once(':') else { continue };
            if let Some((first, last)) = numbers(nums) {
                let end = start + span.len();
                let moves = names.contains(path);
                here.push(Cite { range: end - nums.len()..end, path: path.to_string(), first, last, moves });
            }
        }
        out.push(here);
    }
    out
}

/// `N` or `N-M`, digits only.
fn numbers(s: &str) -> Option<(usize, Option<usize>)> {
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    match s.split_once('-') {
        Some((first, last)) if digits(first) && digits(last) => Some((first.parse().ok()?, Some(last.parse().ok()?))),
        None if digits(s) => Some((s.parse().ok()?, None)),
        _ => None,
    }
}

/// The code spans of a line, each with the byte offset of its text.
fn spans(line: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(open) = line[at..].find('`') {
        let start = at + open + 1;
        let Some(close) = line[start..].find('`') else { break };
        out.push((start, &line[start..start + close]));
        at = start + close + 1;
    }
    out
}
