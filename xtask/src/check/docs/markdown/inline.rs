//! Inline Markdown for the `docs` and `one-home` checks: code spans, and the
//! text that a reader sees.
//!
//! The defect it prevents: a check that reads a link in inline code as a
//! link, or that compares the Markdown marks of two sentences instead of
//! their words.

use std::ops::Range;

/// The inline code spans of a text: the byte range of each span with its
/// backticks, and the number of backticks on each side. A run of backticks
/// without a closing run of the same length is text, as Markdown reads it.
pub(crate) fn code_spans(text: &str) -> Vec<(Range<usize>, usize)> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        match bytes[pos] {
            // A backslash outside a code span escapes the next character.
            b'\\' => pos += 2,
            b'`' => {
                let start = pos;
                pos = run_end(bytes, pos);
                let ticks = pos - start;
                if let Some(end) = closing_run(bytes, pos, ticks) {
                    spans.push((start..end, ticks));
                    pos = end;
                }
            }
            _ => pos += 1,
        }
    }
    spans
}

/// The end of the run of backticks that starts at `pos`.
fn run_end(bytes: &[u8], pos: usize) -> usize {
    pos + bytes[pos..].iter().take_while(|&&b| b == b'`').count()
}

/// The end of the next run of exactly `ticks` backticks at or after `pos`.
fn closing_run(bytes: &[u8], mut pos: usize, ticks: usize) -> Option<usize> {
    while pos < bytes.len() {
        if bytes[pos] == b'`' {
            let end = run_end(bytes, pos);
            if end - pos == ticks {
                return Some(end);
            }
            pos = end;
        } else {
            pos += 1;
        }
    }
    None
}

/// The text with each inline code span replaced by spaces. The line breaks
/// stay, so the line numbers stay correct.
pub(crate) fn blank_code_spans(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (span, _) in code_spans(text) {
        out.push_str(&text[last..span.start]);
        out.extend(text[span.start..span.end].chars().map(|c| if c == '\n' { '\n' } else { ' ' }));
        last = span.end;
    }
    out.push_str(&text[last..]);
    out
}

/// The scheme of a URL (`https` in `https://example.org`), or `None`. A scheme
/// has two characters or more, so a Windows drive letter is not a scheme.
pub(crate) fn scheme(text: &str) -> Option<&str> {
    let (scheme, _) = text.split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next()?;
    let valid = scheme.len() >= 2
        && first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then_some(scheme)
}

/// A character of inline text. `literal` is true for the content of a code
/// span and for a character after a backslash. Such a character is never
/// Markdown syntax.
#[derive(Debug, Clone, Copy)]
struct Ch {
    value: char,
    literal: bool,
}

/// Inline Markdown as the text that a reader sees: the content of a code span
/// without its backticks, the text of a link without its target, and no
/// emphasis marks and no HTML tags. The line breaks stay.
pub(crate) fn plain_text(text: &str) -> String {
    let marked = mark_literals(text);
    let marked = drop_html(&marked);
    let marked = drop_link_syntax(&marked);
    drop_emphasis(&marked)
}

/// The characters of `text` with the code span backticks and the escape
/// backslashes removed, and the `literal` flag set.
fn mark_literals(text: &str) -> Vec<Ch> {
    let mut out = Vec::with_capacity(text.len());
    let mut last = 0;
    for (span, ticks) in code_spans(text) {
        mark_escapes(&text[last..span.start], &mut out);
        let content = &text[span.start + ticks..span.end - ticks];
        // Markdown removes one space at each side when both are there.
        let content = content
            .strip_prefix(' ')
            .and_then(|inner| inner.strip_suffix(' '))
            .filter(|inner| !inner.trim().is_empty())
            .unwrap_or(content);
        out.extend(content.chars().map(|value| Ch { value, literal: true }));
        last = span.end;
    }
    mark_escapes(&text[last..], &mut out);
    out
}

fn mark_escapes(text: &str, out: &mut Vec<Ch>) {
    let mut chars = text.chars().peekable();
    while let Some(value) = chars.next() {
        if value == '\\'
            && let Some(&next) = chars.peek()
            && next.is_ascii_punctuation()
        {
            chars.next();
            out.push(Ch { value: next, literal: true });
        } else {
            out.push(Ch { value, literal: false });
        }
    }
}

/// Remove the HTML tags, and keep the address of an autolink (`<https://...>`).
fn drop_html(chars: &[Ch]) -> Vec<Ch> {
    let mut out = Vec::with_capacity(chars.len());
    let mut pos = 0;
    while let Some(&ch) = chars.get(pos) {
        if ch.value == '<'
            && !ch.literal
            && let Some((kept, used)) = html_at(&chars[pos + 1..])
        {
            out.extend(kept);
            pos += used;
        } else {
            out.push(ch);
            pos += 1;
        }
    }
    out
}

/// For the characters after a `<`: the characters to keep and the number of
/// characters to skip (with the `<` and the `>`), when an autolink or an HTML
/// tag starts there.
fn html_at(rest: &[Ch]) -> Option<(Vec<Ch>, usize)> {
    let end = rest.iter().position(|ch| matches!(ch.value, '<' | '>'))?;
    if rest[end].value != '>' {
        return None;
    }
    let inner = &rest[..end];
    let text: String = inner.iter().map(|ch| ch.value).collect();
    let name = text.strip_prefix('/').unwrap_or(&text);
    if scheme(&text).is_some() && !text.contains(char::is_whitespace) {
        Some((inner.iter().map(|ch| Ch { value: ch.value, literal: true }).collect(), end + 2))
    } else if name.starts_with(|c: char| c.is_ascii_alphabetic()) || text.starts_with("!--") {
        Some((inner.iter().filter(|ch| ch.value == '\n').copied().collect(), end + 2))
    } else {
        None
    }
}

/// Remove the syntax of links and images, and keep the link text:
/// `[text](target)`, `[text][label]` and `![alt](source)` become `text` and `alt`.
fn drop_link_syntax(chars: &[Ch]) -> Vec<Ch> {
    let mut out = Vec::with_capacity(chars.len());
    let mut pos = 0;
    while let Some(&ch) = chars.get(pos) {
        pos += 1;
        let next = chars.get(pos).filter(|next| !next.literal).map(|next| next.value);
        let syntax = ch.value == '[' || (ch.value == '!' && next == Some('['));
        if !ch.literal && ch.value == ']' {
            let close = match next {
                Some('(') => Some(')'),
                Some('[') => Some(']'),
                _ => None,
            };
            if let Some(close) = close
                && let Some(len) = group_len(&chars[pos..], close)
            {
                // A target that spans lines keeps its line breaks.
                out.extend(chars[pos..pos + len].iter().filter(|ch| ch.value == '\n').copied());
                pos += len;
            }
        } else if ch.literal || !syntax {
            out.push(ch);
        }
    }
    out
}

/// The length of the group that opens at the start of `chars` and ends at
/// the matching `close`, with both marks.
fn group_len(chars: &[Ch], close: char) -> Option<usize> {
    let open = chars.first()?.value;
    let mut depth = 0usize;
    for (index, ch) in chars.iter().enumerate() {
        if ch.value == open {
            depth += 1;
        } else if ch.value == close {
            depth -= 1;
            if depth == 0 {
                return Some(index + 1);
            }
        }
    }
    None
}

/// Remove the emphasis marks: each `*`, each run of two or more `~`, and the
/// `_` runs that open or close emphasis. An `_` inside a word (`snake_case`)
/// is not a mark, and an `_` without a partner stays.
fn drop_emphasis(chars: &[Ch]) -> String {
    let mut keep = vec![true; chars.len()];
    let mut openers: Vec<Range<usize>> = Vec::new();
    let mut pos = 0;
    while let Some(&ch) = chars.get(pos) {
        if ch.literal || !matches!(ch.value, '*' | '_' | '~') {
            pos += 1;
            continue;
        }
        let end = pos + chars[pos..].iter().take_while(|next| next.value == ch.value && !next.literal).count();
        if ch.value == '_' {
            let before = pos.checked_sub(1).map(|index| chars[index].value);
            let after = chars.get(end).map(|next| next.value);
            let opens = after.is_some_and(|c| !c.is_whitespace()) && !before.is_some_and(char::is_alphanumeric);
            let closes = before.is_some_and(|c| !c.is_whitespace()) && !after.is_some_and(char::is_alphanumeric);
            if closes && let Some(open) = openers.pop() {
                keep[open].fill(false);
                keep[pos..end].fill(false);
            } else if opens {
                openers.push(pos..end);
            }
        } else if ch.value == '*' || end - pos >= 2 {
            keep[pos..end].fill(false);
        }
        pos = end;
    }
    chars.iter().zip(keep).filter_map(|(ch, kept)| kept.then_some(ch.value)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_spans_pair_runs_of_the_same_length() {
        let text = "a `b` c ``d ` e`` f `g";
        let spans: Vec<&str> = code_spans(text).into_iter().map(|(span, _)| &text[span]).collect();
        assert_eq!(spans, ["`b`", "``d ` e``"]);
        assert_eq!(blank_code_spans("x `y\nz` w"), "x   \n   w");
        // An escaped backtick does not open a span.
        assert_eq!(code_spans("\\`a` b").len(), 0);
    }

    #[test]
    fn plain_text_keeps_what_a_reader_sees() {
        assert_eq!(
            plain_text("**Bold** and _soft_ in [a link](x.md) and ![alt](i.png)"),
            "Bold and soft in a link and alt"
        );
        assert_eq!(plain_text("`a_b` and snake_case and [ref][label]"), "a_b and snake_case and ref");
        assert_eq!(plain_text("<https://example.org/a_b> <br> \\*kept\\*"), "https://example.org/a_b  *kept*");
        assert_eq!(plain_text("a < b and c > d, ~~old~~ new"), "a < b and c > d, old new");
    }
}
