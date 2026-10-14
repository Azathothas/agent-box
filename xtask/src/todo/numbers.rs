//! Line numbers in the text of the record. A citation `FILE:N` moves with FILE
//! (`remap`), and the reader reads it (`check`). A bare `:N` that follows a
//! citation of FILE in its paragraph does the same. A number in plain text after
//! a citation ("line 12", "lines 3-4") does neither. So it can come to name a
//! line that holds other code. "line 12 at `COMMIT`" is a reading of a past tree,
//! which needs no move. Each number of a paragraph that reads at a named commit
//! is a reading too.

use std::fs;
use std::ops::Range;
use std::path::Path;

use super::model::Problem;
use super::parse::lines_with_fences;
use super::refs::citation;

/// The bare `:N` of each paragraph, read in the file that it follows, and
/// each line number in plain text after a citation.
pub(crate) fn check(root: &Path, rel: &str, text: &str, p: &mut Vec<Problem>) {
    for para in paragraphs(text) {
        let (joined, starts) = join(&para);
        let line_at = |offset: usize| starts.iter().rev().find(|(start, _)| *start <= offset).map_or(0, |(_, no)| *no);
        let spans = spans_of(&para, &starts);
        bare(root, rel, &spans, &line_at, p);
        for (range, words) in plain(&joined, &spans) {
            let cited = spans.iter().any(|span| span.range.end <= range.start && span.cites.is_some());
            let read = spans.iter().any(|span| span.range.end <= range.start && span.commit_read);
            if cited && !read && !at_commit(&joined[range.end..]) {
                p.push(Problem::new(
                    rel,
                    line_at(range.start),
                    format!(
                        "\"{words}\" follows a citation as plain text, which remap does not move and check \
                         does not read: write `:N`, or \"{words} at `COMMIT`\" for a reading of a past tree"
                    ),
                ));
            }
        }
    }
}

/// A code span of a paragraph. It has its place in the joined text, its text,
/// the file that it cites, and whether it is a commit after "at".
#[derive(Debug)]
struct Span {
    range: Range<usize>,
    inner: String,
    cites: Option<String>,
    commit_read: bool,
}

/// The paragraphs of a text, as `remap` reads them: a blank line, a heading
/// or a fence ends one.
fn paragraphs(text: &str) -> Vec<Vec<(usize, &str)>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for (no, line, fenced) in lines_with_fences(text) {
        if fenced || line.trim().is_empty() || line.starts_with('#') {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        cur.push((no, line));
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A paragraph as one text, and the offset and the number of each of its
/// lines.
fn join(para: &[(usize, &str)]) -> (String, Vec<(usize, usize)>) {
    let mut joined = String::new();
    let mut starts = Vec::new();
    for &(no, line) in para {
        starts.push((joined.len(), no));
        joined.push_str(line);
        joined.push('\n');
    }
    (joined, starts)
}

/// The code spans of each line, as Markdown reads them. A run of backticks opens
/// a span, and the next run of the same length closes it. So
/// `` `FILE:12` and `:40` `` is one span, which cites nothing.
fn spans_of(para: &[(usize, &str)], starts: &[(usize, usize)]) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for (&(_, line), &(offset, _)) in para.iter().zip(starts) {
        let bytes = line.as_bytes();
        let mut pos = 0;
        while pos < bytes.len() {
            if bytes[pos] != b'`' {
                pos += 1;
                continue;
            }
            let open = pos;
            while pos < bytes.len() && bytes[pos] == b'`' {
                pos += 1;
            }
            let run = pos - open;
            let Some((close_start, close_end)) = closing_run(bytes, pos, run) else {
                // A run that nothing closes is text.
                continue;
            };
            let inner = if run == 1 { &line[pos..close_start] } else { line[pos..close_start].trim_matches(' ') };
            let before = line[..open].trim_end();
            let after_at = before.ends_with(" at") || before == "at" || before.ends_with("(at");
            out.push(Span {
                range: offset + open..offset + close_end,
                inner: inner.to_string(),
                cites: citation(inner).map(|(path, _)| path.to_string()),
                commit_read: after_at && is_commit(inner),
            });
            pos = close_end;
        }
    }
    out
}

/// The first run of exactly `run` backticks at or after `from`: its start and
/// its end.
fn closing_run(bytes: &[u8], from: usize, run: usize) -> Option<(usize, usize)> {
    let mut scan = from;
    while scan < bytes.len() {
        if bytes[scan] != b'`' {
            scan += 1;
            continue;
        }
        let start = scan;
        while scan < bytes.len() && bytes[scan] == b'`' {
            scan += 1;
        }
        if scan - start == run {
            return Some((start, scan));
        }
    }
    None
}

/// Each bare `:N` names a line of the file that the last citation before it
/// in its paragraph names. A bare `:N` that follows no citation names no
/// file, as for `remap`, and is not read.
fn bare(root: &Path, rel: &str, spans: &[Span], line_at: &dyn Fn(usize) -> usize, p: &mut Vec<Problem>) {
    let mut context: Option<&str> = None;
    for span in spans {
        if let Some(path) = &span.cites {
            context = Some(path.as_str());
            continue;
        }
        let (Some(path), Some((first, last))) = (context, span.inner.strip_prefix(':').and_then(numbers)) else {
            continue;
        };
        let at = line_at(span.range.start);
        if first == 0 || first > last {
            p.push(Problem::new(
                rel,
                at,
                format!("`{}`: a range runs from line 1 or later to a line at or after its start", span.inner),
            ));
            continue;
        }
        let Ok(cited) = fs::read_to_string(root.join(path)) else { continue };
        let count = cited.lines().count();
        if last > count {
            p.push(Problem::new(rel, at, format!("`{}` after `{path}`: {path} has {count} lines", span.inner)));
        }
    }
}

/// `N` or `N-M`, digits only: the first line and the last.
fn numbers(s: &str) -> Option<(usize, usize)> {
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    match s.split_once('-') {
        Some((first, last)) if digits(first) && digits(last) => Some((first.parse().ok()?, last.parse().ok()?)),
        None if digits(s) => {
            let n = s.parse().ok()?;
            Some((n, n))
        }
        _ => None,
    }
}

/// A commit as the record writes one: 7 to 40 lower-case hex digits.
fn is_commit(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The text after a number begins "at `COMMIT`", on the same line or the next.
fn at_commit(rest: &str) -> bool {
    let Some(after) = rest.trim_start().strip_prefix("at") else { return false };
    let Some(after) = after.trim_start().strip_prefix('`') else { return false };
    after.split_once('`').is_some_and(|(inner, _)| is_commit(inner))
}

/// Each "line N" or "lines N-M" in the plain text of a paragraph, also "lines 3
/// and 5" and "lines 3, 5 or 7". Plain text is outside code spans and outside
/// double quotes. A quote holds text, and names no line.
fn plain(joined: &str, spans: &[Span]) -> Vec<(Range<usize>, String)> {
    let bytes = joined.as_bytes();
    let mut text = vec![true; bytes.len()];
    for span in spans {
        text[span.range.clone()].fill(false);
    }
    let mut quoted = false;
    for (idx, flag) in text.iter_mut().enumerate() {
        if *flag && bytes[idx] == b'"' {
            quoted = !quoted;
            *flag = false;
        } else if quoted {
            *flag = false;
        }
    }
    let mut out = Vec::new();
    let mut idx = 0;
    while idx < bytes.len() {
        // A byte inside a character of more than one byte starts no word.
        if !joined.is_char_boundary(idx) {
            idx += 1;
            continue;
        }
        let word_start = idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric();
        let rest = &joined[idx..];
        let head = ["line ", "lines ", "Line ", "Lines "].iter().find(|w| rest.starts_with(**w));
        let (true, true, Some(head)) = (text[idx], word_start, head) else {
            idx += 1;
            continue;
        };
        let Some(stop) = number_list(joined, idx + head.len(), &text) else {
            idx += 1;
            continue;
        };
        out.push((idx..stop, joined[idx..stop].to_string()));
        idx = stop;
    }
    out
}

/// The end of a list of numbers that starts at `at`: "12", "3-4", "3 and 5",
/// "3, 5 or 7-9". None when no number starts there.
fn number_list(s: &str, at: usize, text: &[bool]) -> Option<usize> {
    let bytes = s.as_bytes();
    let number = |from: usize| -> Option<usize> {
        let mut k = from;
        while k < bytes.len() && bytes[k].is_ascii_digit() && text[k] {
            k += 1;
        }
        if k == from {
            return None;
        }
        if k + 1 < bytes.len() && bytes[k] == b'-' && bytes[k + 1].is_ascii_digit() {
            k += 1;
            while k < bytes.len() && bytes[k].is_ascii_digit() {
                k += 1;
            }
        }
        Some(k)
    };
    let mut end = number(at)?;
    while let Some(sep) = [", and ", ", or ", " and ", " or ", ", "].iter().find(|sep| s[end..].starts_with(**sep)) {
        let Some(next) = number(end + sep.len()) else { break };
        end = next;
    }
    // A number that runs into a word or a decimal point is not a line.
    let next = bytes.get(end).copied().unwrap_or(b' ');
    let decimal = next == b'.' && bytes.get(end + 1).is_some_and(u8::is_ascii_digit);
    (!next.is_ascii_alphanumeric() && !decimal).then_some(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A root with no files: no cited file can be read there.
    fn no_root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("xtask-todo-numbers-{}-no-such-root", std::process::id()))
    }

    fn found(text: &str) -> Vec<String> {
        let mut p = Vec::new();
        check(&no_root(), "TODO/t.md", text, &mut p);
        p.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_plain_number_after_a_citation_is_found() {
        let p = found("Read `xtask/src/main.rs:3`, and lines 5 and 7-8.\n");
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("\"lines 5 and 7-8\" follows a citation"), "{p:?}");
        // A character of more than one byte before it (an em dash, built here
        // so that this file stays ASCII).
        let dash = char::from_u32(0x2014).unwrap_or('?');
        let p = found(&format!("Read `xtask/src/main.rs:3` {dash} and line 5.\n"));
        assert_eq!(p.len(), 1, "{p:?}");
    }

    #[test]
    fn readings_quotes_and_other_numbers_pass() {
        for text in [
            "Read `xtask/src/main.rs:3`, line 5 at `0123abc`.\n",
            "Read `xtask/src/main.rs:3`, line 5\nat `0123abc`.\n",
            "Read, at `0123abc`: `xtask/src/main.rs:3`, and line 5.\n",
            "Read `xtask/src/main.rs:3`; it says \"line 5\".\n",
            "Line 5 of the output, before any citation: `xtask/src/main.rs:3`.\n",
            "Read `xtask/src/main.rs:3`.\n\nA new paragraph: line 5 of a log.\n",
            "Read `xtask/src/main.rs:3`, in 5 lines, with a pipeline 2.5 long.\n",
            "Read `QaidVoid/kage@f8df989:src/main.rs:3`, line 5 of another project.\n",
        ] {
            assert_eq!(found(text), Vec::<String>::new(), "{text:?}");
        }
    }

    #[test]
    fn a_double_backtick_span_is_one_span() {
        let mut p = Vec::new();
        let para = [(1, "(the record writes `` `FILE:12` and `:40` ``)")];
        let (_, starts) = join(&para);
        let spans = spans_of(&para, &starts);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].inner, "`FILE:12` and `:40`");
        bare(&no_root(), "TODO/t.md", &spans, &|_| 1, &mut p);
        assert!(p.is_empty(), "{p:?}");
    }
}
