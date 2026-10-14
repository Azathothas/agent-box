//! Reading the Markdown of the record. Each shape is a fixed line. So the reader
//! can compare a number or a status that one file quotes with the file that owns
//! it.

use std::ops::Range;

use super::model::{Counts, Entry, FIELDS, Field, Problem, Row, STATUSES, Section, is_id};

/// The lines of a text with their numbers, and whether each line is inside a
/// fenced code block (a fence line itself counts as inside).
pub(crate) fn lines_with_fences(text: &str) -> Vec<(usize, &str, bool)> {
    let mut out = Vec::new();
    let mut fenced = false;
    for (idx, line) in text.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
            out.push((idx + 1, line, true));
        } else {
            out.push((idx + 1, line, fenced));
        }
    }
    out
}

/// The cells of a Markdown table line, trimmed, without the outer pipes. As
/// GitHub reads a table, a backslash before a `|` makes it a `|` inside a cell.
pub(crate) fn cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let inner = t.strip_prefix('|').unwrap_or(t);
    let inner = match inner.strip_suffix('|') {
        Some(rest) if !rest.ends_with('\\') => rest,
        _ => inner,
    };
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            cell.push('|');
            chars.next();
        } else if c == '|' {
            out.push(cell.trim().to_string());
            cell.clear();
        } else {
            cell.push(c);
        }
    }
    out.push(cell.trim().to_string());
    out
}

/// A `|` inside a table cell, as the index writes it.
const ESCAPED_PIPE: &str = "\\|";

/// A table line from its cells: the inverse of [`cells`].
pub(crate) fn table_line(cells: &[String]) -> String {
    let escaped: Vec<String> = cells.iter().map(|c| c.replace('|', ESCAPED_PIPE)).collect();
    format!("| {} |", escaped.join(" | "))
}

/// The rows of the table of entries: each line that starts with `| [T-`.
pub(crate) fn rows(file: &str, index: &str) -> (Vec<Row>, Vec<Problem>) {
    let mut rows = Vec::new();
    let mut problems = Vec::new();
    for (line_no, line, fenced) in lines_with_fences(index) {
        if fenced || !line.trim_start().starts_with("| [T-") {
            continue;
        }
        let c = cells(line);
        let link = c.first().map_or("", String::as_str);
        let parsed = link
            .strip_prefix('[')
            .and_then(|s| s.split_once("]("))
            .and_then(|(id, rest)| rest.strip_suffix(')').map(|f| (id.to_string(), f.to_string())));
        if let Some((id, target)) = parsed
            && c.len() == 7
        {
            rows.push(Row {
                id,
                file: target,
                priority: c[1].clone(),
                effort: c[2].clone(),
                milestone: c[3].clone(),
                category: c[4].clone(),
                status: c[5].clone(),
                title: c[6].clone(),
                line: line_no,
            });
        } else {
            problems.push(Problem::new(
                file,
                line_no,
                "a row must be | [T-NNN](area.md) | priority | effort | milestone | category | status | title | (7 cells; a `|` in the title is written `\\|`)",
            ));
        }
    }
    (rows, problems)
}

/// The entries of an area file. The file can start with one H1 that is its
/// title, as `# Repository and tooling`. The problems are another H1 that is
/// not an entry, and a header line that is not a known field.
pub(crate) fn entries(file: &str, text: &str) -> (Vec<Entry>, Vec<Problem>) {
    let mut entries: Vec<Entry> = Vec::new();
    let mut problems = Vec::new();
    // Inside an entry: in the header (before the first `##`), or in a section.
    let mut in_header = false;
    let mut titled = false;
    for (line_no, line, fenced) in lines_with_fences(text) {
        if !fenced && let Some(head) = line.strip_prefix("# ") {
            if let Some((id, title)) = head.split_once(": ")
                && is_id(id)
            {
                entries.push(Entry {
                    id: id.to_string(),
                    title: title.trim().to_string(),
                    file: file.to_string(),
                    line: line_no,
                    fields: Vec::new(),
                    sections: Vec::new(),
                });
                in_header = true;
            } else if entries.is_empty() && !titled {
                titled = true;
            } else {
                problems.push(Problem::new(
                    file,
                    line_no,
                    "an H1 that is not an entry (`# T-NNN: Title`); only the title of the file comes before the first entry",
                ));
            }
            continue;
        }
        let Some(entry) = entries.last_mut() else { continue };
        if !fenced && let Some(name) = line.strip_prefix("## ") {
            entry.sections.push(Section { name: name.trim().to_string(), line: line_no, body: String::new() });
            in_header = false;
            continue;
        }
        if in_header {
            header_line(entry, line, line_no, file, &mut problems);
        } else if let Some(section) = entry.sections.last_mut() {
            section.body.push_str(line);
            section.body.push('\n');
        }
    }
    (entries, problems)
}

/// One line of the header of an entry: `**Name:** value`, or the continuation
/// of the field above it.
fn header_line(entry: &mut Entry, line: &str, line_no: usize, file: &str, problems: &mut Vec<Problem>) {
    if line.trim().is_empty() {
        return;
    }
    if let Some(rest) = line.strip_prefix("**")
        && let Some((name, value)) = rest.split_once(":**")
    {
        if FIELDS.contains(&name) {
            entry.fields.push(Field { name: name.to_string(), value: value.trim().to_string(), line: line_no });
        } else {
            problems.push(Problem::new(file, line_no, format!("`{name}` is not a field of an entry")));
        }
        return;
    }
    if let Some(field) = entry.fields.last_mut() {
        field.value.push(' ');
        field.value.push_str(line.trim());
    } else {
        problems.push(Problem::new(file, line_no, "text before the first field of the entry"));
    }
}

/// The counts phrase `N entries: A open, B partial, C blocked, D done.` in a
/// line: the total, the counts, and the byte range of the phrase.
pub(crate) fn counts_phrase(line: &str) -> Option<(usize, Counts, Range<usize>)> {
    let key = " entries: ";
    let at = line.find(key)?;
    let digits = line[..at].bytes().rev().take_while(u8::is_ascii_digit).count();
    let start = at - digits;
    let total: usize = line[start..at].parse().ok()?;
    let rest = &line[at + key.len()..];
    let end_rel = rest.find('.')?;
    let parts = &rest[..end_rel];
    if parts.split(", ").count() != STATUSES.len() {
        return None;
    }
    let mut counts = Counts::default();
    for (part, want) in parts.split(", ").zip(STATUSES) {
        let (number, word) = part.split_once(' ')?;
        if word != want {
            return None;
        }
        counts.set(want, number.parse().ok()?);
    }
    let end = at + key.len() + end_rel + 1;
    Some((total, counts, start..end))
}

/// The first line that carries the counts phrase: its number, the total and
/// the counts.
pub(crate) fn find_counts(text: &str) -> Option<(usize, usize, Counts)> {
    lines_with_fences(text)
        .into_iter()
        .filter(|(_, _, fenced)| !fenced)
        .find_map(|(no, line, _)| counts_phrase(line).map(|(total, counts, _)| (no, total, counts)))
}

/// A row of the table of counts: `| P1 | a | b | c | d | total |`, or the
/// `| **All** | ... |` row. Returns the label, the counts and the total.
pub(crate) fn counts_row(line: &str) -> Option<(String, Counts, usize)> {
    let c = cells(line);
    if c.len() != 6 {
        return None;
    }
    let n: Vec<usize> = c[1..].iter().map(|v| v.parse().ok()).collect::<Option<_>>()?;
    let label = c[0].trim_matches('*').to_string();
    Some((label, Counts { open: n[0], partial: n[1], blocked: n[2], done: n[3] }, n[4]))
}

/// The body of the `## Heading` section of a document, up to the next `## `,
/// with each line number. A line in a fenced code block is not part of it.
pub(crate) fn doc_section<'a>(text: &'a str, heading: &str) -> Option<(usize, Vec<(usize, &'a str)>)> {
    let mut out: Option<(usize, Vec<(usize, &'a str)>)> = None;
    for (no, line, fenced) in lines_with_fences(text) {
        if !fenced && let Some(title) = line.strip_prefix("## ") {
            if out.is_some() {
                break;
            }
            if title.trim() == heading {
                out = Some((no, Vec::new()));
            }
            continue;
        }
        if let Some((_, body)) = out.as_mut()
            && !fenced
        {
            body.push((no, line));
        }
    }
    out
}

/// The code spans of a line (single backticks).
pub(crate) fn code_spans(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        out.push(&after[..close]);
        rest = &after[close + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_counts_phrase_is_read_where_it_stands() {
        let line = "**12 entries: 5 open, 1 partial, 2 blocked, 4 done.**";
        let (total, c, range) = counts_phrase(line).unwrap();
        assert_eq!(total, 12);
        assert_eq!((c.open, c.partial, c.blocked, c.done), (5, 1, 2, 4));
        assert_eq!(&line[range], "12 entries: 5 open, 1 partial, 2 blocked, 4 done.");
        let line = "`TODO/INDEX.md` holds 7 entries: 1 open, 2 partial, 3 blocked, 1 done.";
        assert_eq!(counts_phrase(line).unwrap().0, 7);
        assert!(counts_phrase("7 entries: 1 open, 2 partial, 3 done.").is_none());
        assert!(counts_phrase("7 entries: 1 open, 2 partial, 3 blocked, 1 closed.").is_none());
    }

    #[test]
    fn a_phrase_after_a_wide_character_is_read() {
        // A character of more than one byte before the number must not stop the reader.
        let wide = char::from_u32(0xE9).unwrap_or('?');
        let line = format!("{wide}3 entries: 1 open, 0 partial, 1 blocked, 1 done.");
        assert_eq!(counts_phrase(&line).map(|(total, ..)| total), Some(3));
    }

    #[test]
    fn a_heading_in_a_fence_is_not_an_entry() {
        let text = "# T-001: One\n\n**Status:** open\n\n## Prove\n\n```sh\n# T-002: not an entry\n## Done\n```\n";
        let (e, p) = entries("a.md", text);
        assert!(p.is_empty(), "{p:?}");
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].sections.len(), 1);
        assert!(e[0].sections[0].body.contains("# T-002"));
    }

    #[test]
    fn an_area_file_can_have_one_title() {
        let text = "# Repository and tooling\n\nThe area.\n\n# T-001: One\n\n**Status:** open\n";
        let (e, p) = entries("a.md", text);
        assert!(p.is_empty(), "{p:?}");
        assert_eq!(e.len(), 1);
        // A second title before the first entry.
        let (_, p) = entries("a.md", &format!("# Notes\n\n{text}"));
        assert_eq!(p.iter().map(|x| x.line).collect::<Vec<_>>(), [3], "{p:?}");
        // A title after an entry.
        let (_, p) = entries("a.md", &format!("{text}\n# Notes\n"));
        assert_eq!(p.iter().map(|x| x.line).collect::<Vec<_>>(), [9], "{p:?}");
    }

    #[test]
    fn a_field_can_wrap() {
        let text = "# T-001: One\n\n**Source:** the operator,\nin two lines.\n**Status:** open\n";
        let (e, _) = entries("a.md", text);
        assert_eq!(e[0].field("Source"), Some("the operator, in two lines."));
        assert_eq!(e[0].field("Status"), Some("open"));
    }

    #[test]
    fn a_row_has_seven_cells() {
        let index = "| [T-001](repo.md) | P1 | S | M0 | chore | done | One |\n| [T-002](repo.md) | P1 | S |\n";
        let (r, p) = rows("TODO/INDEX.md", index);
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].id.as_str(), r[0].file.as_str(), r[0].milestone.as_str()), ("T-001", "repo.md", "M0"));
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].line, 2);
    }

    #[test]
    fn a_pipe_in_a_cell_is_written_with_a_backslash() {
        let line = "| [T-067](image.md) | P1 | S | M2 | feature | open | `image pull\\|list` |";
        let c = cells(line);
        assert_eq!(c.len(), 7, "{c:?}");
        assert_eq!(c[6], "`image pull|list`");
        assert_eq!(table_line(&c), line);
        let (r, p) = rows("TODO/INDEX.md", line);
        assert!(p.is_empty(), "{p:?}");
        assert_eq!(r[0].title, "`image pull|list`");
        // A cell that ends with an escaped pipe keeps it.
        assert_eq!(cells("| a | b\\|"), ["a", "b|"]);
    }

    #[test]
    fn a_section_skips_its_fenced_lines() {
        let text = "## Work order\n\n1. T-001.\n```\nT-999\n```\n## Next\n";
        let (head, body) = doc_section(text, "Work order").unwrap();
        assert_eq!(head, 1);
        assert!(body.iter().all(|(_, l)| !l.contains("T-999")), "{body:?}");
        assert!(body.iter().any(|(_, l)| l.contains("T-001")), "{body:?}");
    }
}
