//! Check `prose`: do the documents keep the ASD-STE100 rules that a machine
//! can count?
//!
//! The defect this check exists to catch: prose that a reader cannot take in
//! one pass. A sentence of more than 25 words, or a word that STE does not
//! allow, gets into the documents when nothing counts them. The writers of
//! this repository are agents too, and an agent repeats the style it reads.
//!
//! The scope is every visible Markdown file, and the comments of every
//! visible Rust file. In a Rust comment the check reads the banned words
//! only, because a comment line is often not a full sentence.
//!
//! This is a heuristic. It reads Markdown line by line and builds no syntax
//! tree. [`markdown_units`] gives the block rules, and the `inline` module
//! gives the word and sentence rules.

mod inline;
mod rust;

use std::path::Path;

use crate::repo::{self, Repo};
use crate::report::{CheckReport, Finding};

pub(crate) const DEFECT: &str = "a sentence of more than 25 words, or a word that ASD-STE100 does not allow";

const NAME: &str = "prose";

/// This repository has more Markdown files than this. Fewer means that the
/// file list is wrong, not that the documents are clean.
const FLOOR: usize = 10;

pub(crate) fn check(repo: &Repo) -> CheckReport {
    let files = match repo.visible_files() {
        Ok(files) => files,
        Err(e) => return CheckReport::not_run(NAME, format!("cannot list the files: {e}")),
    };
    let mut findings = Vec::new();
    let mut unreadable = Vec::new();
    let (mut markdown_files, mut rust_files) = (0, 0);
    for rel in &files {
        let is_markdown = repo::is_markdown(rel);
        if !is_markdown && !is_rust(rel) {
            continue;
        }
        let bytes = match repo.read_bytes(rel) {
            Ok(bytes) => bytes,
            Err(e) => {
                unreadable.push(e);
                continue;
            }
        };
        // The `text` check reports a byte that is not UTF-8. Here it becomes U+FFFD.
        let text = String::from_utf8_lossy(&bytes);
        if is_markdown {
            markdown_files += 1;
            findings.extend(markdown(rel, &text));
        } else {
            rust_files += 1;
            findings.extend(rust_comments(rel, &text));
        }
    }
    // The floor counts the Markdown files only. They are the main scope.
    let mut report = super::text::finish(NAME, markdown_files, FLOOR, findings, &unreadable);
    report.scanned = markdown_files + rust_files;
    report.note(format!("read {markdown_files} Markdown files and the comments of {rust_files} Rust files"))
}

fn is_rust(rel: &str) -> bool {
    Path::new(rel).extension().is_some_and(|e| e == "rs")
}

/// The findings for one Markdown file, in line order.
fn markdown(rel: &str, text: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for unit in markdown_units(text) {
        let words = inline::words(&unit.text);
        for long in inline::long_sentences(&words) {
            let message =
                format!("{} words in one sentence, the limit is {}: \"{} ...\"", long.count, inline::LIMIT, long.start);
            findings.push(Finding::new(rel, long.line, message));
        }
        findings.extend(banned(rel, &words));
    }
    findings.sort_by_key(|f| f.line);
    findings
}

/// The findings for the comments of one Rust file: the banned words only.
/// The check skips a fenced example in a doc comment, because it is code.
fn rust_comments(rel: &str, source: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut fence = false;
    for (line, text) in rust::comments(source) {
        if text.starts_with("```") || text.starts_with("~~~") {
            fence = !fence;
        } else if !fence {
            let chars: Vec<(char, usize)> = text.chars().map(|c| (c, line)).collect();
            findings.extend(banned(rel, &inline::words(&chars)));
        }
    }
    findings
}

fn banned(rel: &str, words: &[inline::Word]) -> Vec<Finding> {
    let hits = inline::banned(words);
    hits.into_iter().map(|h| Finding::new(rel, h.line, format!("\"{}\" is not allowed: {}", h.term, h.hint))).collect()
}

/// A run of text that the rules read as one: a paragraph, a list item or a
/// table cell. Each character comes with its line number.
#[derive(Debug, Default)]
struct Unit {
    text: Vec<(char, usize)>,
}

impl Unit {
    fn of(line: usize, text: &str) -> Self {
        let mut unit = Self::default();
        unit.push(line, text);
        unit
    }

    fn push(&mut self, line: usize, text: &str) {
        if !self.text.is_empty() {
            self.text.push((' ', line));
        }
        self.text.extend(text.trim().chars().map(|c| (c, line)));
    }
}

/// The units of a Markdown file so far, and the state of the line reader.
#[derive(Debug, Default)]
struct Reader {
    units: Vec<Unit>,
    /// The unit that the next text line joins.
    open: Option<Unit>,
    /// True when `open` is a paragraph. A setext underline makes it a heading.
    paragraph: bool,
    /// True after a list item, until a heading or a paragraph at column 0.
    list: bool,
    /// True after a table separator row, until a blank line or another block.
    table: bool,
}

impl Reader {
    fn flush(&mut self) {
        self.units.extend(self.open.take());
    }

    fn start(&mut self, line: usize, text: &str, paragraph: bool) {
        self.flush();
        self.open = Some(Unit::of(line, text));
        self.paragraph = paragraph;
    }

    /// Add a text line to the open unit, or start a paragraph.
    fn add(&mut self, line: usize, text: &str) {
        match &mut self.open {
            Some(unit) => unit.push(line, text),
            None => self.start(line, text, true),
        }
    }
}

/// The units of a Markdown text.
///
/// The reader skips a fenced code block, an indented code block outside a
/// list, a block quote, an HTML comment, a heading, a link reference
/// definition, a thematic break and a table separator row. Each cell of a
/// table row is a unit. Paragraph lines join until a blank line. A list item
/// ends at the next list item or a blank line. A line that starts with a bold
/// label such as `**Status:**` starts a new unit, as in the TODO entries.
fn markdown_units(text: &str) -> Vec<Unit> {
    let lines: Vec<&str> = text.lines().collect();
    let mut r = Reader::default();
    let mut fence: Option<(char, usize)> = None;
    let mut comment = false;
    let mut code = false;
    for (index, raw) in lines.iter().enumerate() {
        let number = index + 1;
        if let Some(open) = fence {
            if closes_fence(raw, open) {
                fence = None;
            }
            continue;
        }
        let line = strip_comments(raw, &mut comment);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            r.flush();
            r.table = false;
            continue;
        }
        // An indented code block cannot interrupt a paragraph, and in a list
        // the indent belongs to the item.
        let indent = indent_width(&line);
        if indent >= 4 && (code || (r.open.is_none() && !r.list)) {
            code = true;
            continue;
        }
        code = false;
        if r.paragraph && r.open.is_some() && is_setext_underline(trimmed) {
            // The open paragraph is a heading.
            r.open = None;
            continue;
        }
        let heading = is_heading(trimmed);
        fence = opens_fence(trimmed);
        if fence.is_some()
            || heading
            || trimmed.starts_with('>')
            || is_link_definition(trimmed)
            || is_thematic_break(trimmed)
            || is_table_separator(trimmed)
        {
            r.flush();
            r.table = is_table_separator(trimmed);
            r.list &= !heading;
            continue;
        }
        let header = trimmed.contains('|') && lines.get(index + 1).is_some_and(|next| is_table_separator(next.trim()));
        if r.table || header || trimmed.starts_with('|') {
            r.flush();
            r.units.extend(cells(trimmed).iter().map(|cell| Unit::of(number, cell)));
            continue;
        }
        if let Some(item) = list_item(trimmed) {
            r.start(number, item, false);
            r.list = true;
        } else if is_field(trimmed) {
            r.start(number, trimmed, true);
        } else {
            if indent == 0 && r.open.is_none() {
                r.list = false;
            }
            r.add(number, trimmed);
        }
    }
    r.flush();
    r.units
}

/// The line without its HTML comments. `inside` carries an open comment to
/// the next line. A comment opener after an odd number of backticks is in a
/// code span, so it is text.
fn strip_comments(line: &str, inside: &mut bool) -> String {
    let mut out = String::new();
    let mut rest = line;
    loop {
        if *inside {
            let Some(end) = rest.find("-->") else {
                return out;
            };
            rest = &rest[end + 3..];
            *inside = false;
            out.push(' ');
        }
        let ticks = out.matches('`').count();
        let open = rest
            .match_indices("<!--")
            .map(|(at, _)| at)
            .find(|&at| (ticks + rest[..at].matches('`').count()).is_multiple_of(2));
        let Some(start) = open else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..start]);
        rest = &rest[start + 4..];
        *inside = true;
    }
}

/// The width of the indent of a line. A tab goes to the next multiple of four.
fn indent_width(line: &str) -> usize {
    let mut width = 0;
    for c in line.chars() {
        match c {
            ' ' => width += 1,
            '\t' => width += 4 - width % 4,
            _ => break,
        }
    }
    width
}

/// The mark and the length of a fence that opens on this line.
fn opens_fence(trimmed: &str) -> Option<(char, usize)> {
    let mark = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let run = trimmed.chars().take_while(|&c| c == mark).count();
    // The info string of a backtick fence has no backtick: a line that holds
    // one more is a code span.
    let info_ok = mark == '~' || !trimmed[run..].contains('`');
    (run >= 3 && info_ok).then_some((mark, run))
}

fn closes_fence(raw: &str, (mark, run): (char, usize)) -> bool {
    let line = raw.trim();
    line.chars().count() >= run && line.chars().all(|c| c == mark)
}

fn is_heading(trimmed: &str) -> bool {
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    (1..=6).contains(&hashes) && trimmed[hashes..].chars().next().is_none_or(char::is_whitespace)
}

/// `[label]: target`, but not a footnote `[^1]: text`, which is prose.
fn is_link_definition(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix('[') else {
        return false;
    };
    !rest.starts_with('^') && rest.find("]:").is_some_and(|end| !rest[..end].contains(['[', ']']))
}

fn is_setext_underline(trimmed: &str) -> bool {
    !trimmed.is_empty() && (trimmed.chars().all(|c| c == '=') || trimmed.chars().all(|c| c == '-'))
}

fn is_thematic_break(trimmed: &str) -> bool {
    let marks: Vec<char> = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    marks.len() >= 3 && ['-', '*', '_'].iter().any(|m| marks.iter().all(|c| c == m))
}

fn is_table_separator(trimmed: &str) -> bool {
    trimmed.contains('|') && trimmed.contains('-') && trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
}

/// A line that starts with a bold label, such as `**Source:** text`.
fn is_field(trimmed: &str) -> bool {
    trimmed.strip_prefix("**").and_then(|rest| rest.find(":**")).is_some_and(|end| end <= 40)
}

/// The cells of a table row, without the empty ones. A `\|` is a pipe in a
/// cell, not a cell edge.
fn cells(row: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    for c in row.chars() {
        if c == '|' && !escaped {
            cells.push(std::mem::take(&mut cell));
        } else {
            cell.push(c);
        }
        escaped = c == '\\' && !escaped;
    }
    cells.push(cell);
    cells.into_iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect()
}

/// The text of a list item, without its marker and a task box.
fn list_item(trimmed: &str) -> Option<&str> {
    let rest = if let Some(rest) = trimmed.strip_prefix(['-', '*', '+']) {
        rest
    } else {
        let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
        if !(1..=9).contains(&digits) {
            return None;
        }
        trimmed[digits..].strip_prefix(['.', ')'])?
    };
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    let rest = rest.trim_start();
    Some(["[ ] ", "[x] ", "[X] "].iter().find_map(|mark| rest.strip_prefix(mark)).unwrap_or(rest))
}

#[cfg(test)]
mod tests;
