//! A small Markdown reader for the `docs` and `one-home` checks.
//!
//! The defect it prevents: a check that reads a link or a sentence in a code
//! block as prose, or that makes a heading anchor that is different from the
//! anchor that GitHub makes. The reader finds fenced code blocks, inline code
//! spans, headings, block quotes, tables and text. It also turns inline
//! Markdown into the text that a reader sees. It is not a full Markdown
//! parser: it reads an indented code block and an HTML block as text.

mod inline;

use std::collections::{BTreeSet, HashMap};

pub(crate) use inline::{blank_code_spans, code_spans, plain_text, scheme};

/// The kind of a block. Each check reads some kinds and skips others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A fenced code block, with its fence lines.
    Code,
    /// An ATX heading (`## Title`), or a setext heading (text over `===` or `---`).
    Heading,
    /// A paragraph, or a list item without its marker.
    Text,
    /// Lines that start with `>`.
    Quote,
    /// The rows of a table.
    Table,
}

/// A block of lines of a document.
#[derive(Debug, Clone)]
pub(crate) struct Block {
    pub(crate) kind: Kind,
    /// The number of the first line, from 1.
    pub(crate) line: usize,
    /// One line for each line of the file, joined with `\n`. A heading holds
    /// its text only, without the `#` marks and without the underline.
    pub(crate) text: String,
    /// True for a list item. A `---` line after a list item is a thematic
    /// break, not a heading underline.
    list: bool,
}

impl Block {
    fn new(kind: Kind, line: usize, text: &str) -> Self {
        Self { kind, line, text: text.to_string(), list: false }
    }
}

/// Split a document into blocks.
pub(crate) fn blocks(text: &str) -> Vec<Block> {
    let mut reader = Reader::default();
    for (index, line) in text.lines().enumerate() {
        reader.line(index + 1, line);
    }
    reader.flush();
    reader.out
}

/// The state of [`blocks`]: the finished blocks, the open block, and the
/// mark and length of the open code fence.
#[derive(Debug, Default)]
struct Reader {
    out: Vec<Block>,
    open: Option<Block>,
    fence: Option<(char, usize)>,
}

impl Reader {
    fn flush(&mut self) {
        self.out.extend(self.open.take());
    }

    /// Add a line to the open block when it is of `kind`, or start a new block.
    fn add(&mut self, kind: Kind, number: usize, line: &str) {
        if let Some(block) = self.open.as_mut()
            && block.kind == kind
        {
            block.text.push('\n');
            block.text.push_str(line);
            return;
        }
        self.flush();
        self.open = Some(Block::new(kind, number, line));
    }

    /// Read one line. Each test returns when the line belongs to it.
    fn line(&mut self, number: usize, line: &str) {
        if let Some(fence) = self.fence {
            self.add(Kind::Code, number, line);
            if closes_fence(line, fence) {
                self.fence = None;
                self.flush();
            }
            return;
        }
        if let Some(fence) = opens_fence(line) {
            self.flush();
            self.add(Kind::Code, number, line);
            self.fence = Some(fence);
            return;
        }
        if line.trim().is_empty() {
            self.flush();
            return;
        }
        if let Some(title) = atx_heading(line) {
            self.flush();
            self.out.push(Block::new(Kind::Heading, number, title));
            return;
        }
        if is_setext_underline(line)
            && let Some(block) = self.open.as_mut()
            && block.kind == Kind::Text
            && !block.list
        {
            block.kind = Kind::Heading;
            self.flush();
            return;
        }
        if is_thematic_break(line) {
            self.flush();
            return;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with('>') {
            self.add(Kind::Quote, number, line);
            return;
        }
        if is_table_delimiter(line) {
            // The line before a delimiter row is the header row of the table.
            if let Some(block) = self.open.as_mut()
                && block.kind == Kind::Text
                && !block.list
                && !block.text.contains('\n')
            {
                block.kind = Kind::Table;
            }
            self.add(Kind::Table, number, line);
            return;
        }
        let in_table = self.open.as_ref().is_some_and(|block| block.kind == Kind::Table);
        if trimmed.starts_with('|') || (in_table && line.contains('|')) {
            self.add(Kind::Table, number, line);
            return;
        }
        if let Some(item) = list_item(line) {
            self.flush();
            self.open = Some(Block { kind: Kind::Text, line: number, text: item.to_string(), list: true });
            return;
        }
        self.add(Kind::Text, number, line);
    }
}

/// The mark and the length of the fence when the line opens a fenced code block.
fn opens_fence(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start();
    let mark = trimmed.chars().next().filter(|&c| matches!(c, '`' | '~'))?;
    let len = trimmed.chars().take_while(|&c| c == mark).count();
    // The info string of a backtick fence has no backtick, so "```a```" at
    // the start of a line is inline code. The marks are ASCII: `len` is a byte count.
    let inline_code = mark == '`' && trimmed[len..].contains('`');
    (len >= 3 && !inline_code).then_some((mark, len))
}

/// True when the line closes the fence that `opens_fence` returned.
fn closes_fence(line: &str, (mark, len): (char, usize)) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= len && trimmed.chars().all(|c| c == mark)
}

/// The text of an ATX heading (`## Title ##`), or `None`.
fn atx_heading(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let level = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    let rest = rest.trim();
    // A closing sequence of `#` marks goes away when a space comes before it.
    let open = rest.trim_end_matches('#');
    if open.is_empty() {
        Some(open)
    } else if open.ends_with([' ', '\t']) {
        Some(open.trim_end())
    } else {
        Some(rest)
    }
}

fn is_setext_underline(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && (trimmed.bytes().all(|b| b == b'=') || trimmed.bytes().all(|b| b == b'-'))
}

fn is_thematic_break(line: &str) -> bool {
    let marks: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    marks.len() >= 3 && ['-', '*', '_'].iter().any(|mark| marks.iter().all(|c| c == mark))
}

/// True for the delimiter row of a table: `| --- | :---: |`.
fn is_table_delimiter(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|')
        && trimmed.trim_matches('|').split('|').all(|cell| {
            let cell = cell.trim();
            let cell = cell.strip_prefix(':').unwrap_or(cell);
            let cell = cell.strip_suffix(':').unwrap_or(cell);
            !cell.is_empty() && cell.bytes().all(|b| b == b'-')
        })
}

/// The text of a list item without its marker (`-`, `*`, `+`, `1.`, `1)`)
/// and without its task box, or `None`.
fn list_item(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let after = if let Some(rest) = trimmed.strip_prefix(['-', '*', '+']) {
        rest
    } else {
        let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
        if !(1..=9).contains(&digits) {
            return None;
        }
        trimmed[digits..].strip_prefix(['.', ')'])?
    };
    if !after.is_empty() && !after.starts_with([' ', '\t']) {
        return None;
    }
    let rest = after.trim_start();
    Some(["[ ] ", "[x] ", "[X] "].into_iter().find_map(|task| rest.strip_prefix(task)).unwrap_or(rest))
}

/// The GitHub anchor of a heading, before the `-1`, `-2` suffix of a repeated
/// heading: the plain text in lowercase, without the characters that are not
/// letters, digits, spaces, `-` or `_`, and with each space changed to `-`.
pub(crate) fn slug(heading: &str) -> String {
    plain_text(heading)
        .trim()
        .to_lowercase()
        .chars()
        .filter(|&c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// Each anchor of a document: the slug of each heading, with the `-1`, `-2`
/// suffix of a repeated slug as GitHub writes it, and the `id` or `name` of
/// each HTML `<a>` tag.
pub(crate) fn anchors(text: &str) -> BTreeSet<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out = BTreeSet::new();
    for block in blocks(text) {
        if block.kind == Kind::Code {
            continue;
        }
        html_ids(&blank_code_spans(&block.text), &mut out);
        if block.kind == Kind::Heading {
            let base = slug(&block.text);
            let mut name = base.clone();
            while seen.contains_key(&name) {
                let count = seen.entry(base.clone()).or_insert(0);
                *count += 1;
                name = format!("{base}-{count}");
            }
            seen.insert(name.clone(), 0);
            out.insert(name);
        }
    }
    out
}

/// Add the `id` and `name` values of the HTML `<a>` tags in `text`.
fn html_ids(text: &str, out: &mut BTreeSet<String>) {
    for (index, _) in text.match_indices("<a ") {
        let tag = &text[index..];
        let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
        for key in [" id=\"", " name=\""] {
            if let Some(start) = tag.find(key) {
                let value = &tag[start + key.len()..];
                if let Some(len) = value.find('"') {
                    out.insert(value[..len].to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(Kind, usize, String)> {
        blocks(text).into_iter().map(|block| (block.kind, block.line, block.text)).collect()
    }

    #[test]
    fn blocks_have_a_kind_and_a_first_line() {
        let text = "# Title\n\nSome text\nmore text\n\n```sh\nls\n```\n> quoted\n- item one\n  more\n2. item two\n";
        let found = kinds(text);
        assert_eq!(found[0], (Kind::Heading, 1, "Title".to_string()));
        assert_eq!(found[1], (Kind::Text, 3, "Some text\nmore text".to_string()));
        assert_eq!(found[2], (Kind::Code, 6, "```sh\nls\n```".to_string()));
        assert_eq!(found[3], (Kind::Quote, 9, "> quoted".to_string()));
        assert_eq!(found[4], (Kind::Text, 10, "item one\n  more".to_string()));
        assert_eq!(found[5], (Kind::Text, 12, "item two".to_string()));
        assert_eq!(found.len(), 6);
    }

    #[test]
    fn a_fence_closes_only_with_its_own_mark_and_length() {
        let found = kinds("````md\n```\n# not a heading\n```\n````\nafter\n~~~\n```\n~~~\n");
        assert_eq!(found[0].0, Kind::Code);
        assert_eq!(found[0].2.lines().count(), 5);
        assert_eq!(found[1], (Kind::Text, 6, "after".to_string()));
        assert_eq!(found[2], (Kind::Code, 7, "~~~\n```\n~~~".to_string()));
        // "```x```" at the start of a line is inline code, not a fence.
        assert_eq!(kinds("```x``` is code\n")[0].0, Kind::Text);
    }

    #[test]
    fn setext_headings_tables_and_breaks() {
        assert_eq!(kinds("Title\n=====\n")[0], (Kind::Heading, 1, "Title".to_string()));
        // A `---` line after a list item is a thematic break.
        assert_eq!(kinds("- item\n---\n"), vec![(Kind::Text, 1, "item".to_string())]);
        let table = kinds("a | b\n--- | ---\nc | d\n\n| e |\n");
        assert_eq!(table[0], (Kind::Table, 1, "a | b\n--- | ---\nc | d".to_string()));
        assert_eq!(table[1], (Kind::Table, 5, "| e |".to_string()));
    }

    #[test]
    fn slugs_follow_github() {
        assert_eq!(slug("The `box_core` crate"), "the-box_core-crate");
        assert_eq!(slug("9. Style: ASD-STE100 rules for every writer"), "9-style-asd-ste100-rules-for-every-writer");
        assert_eq!(slug("T-001: Cargo workspace, pinned toolchain"), "t-001-cargo-workspace-pinned-toolchain");
        assert_eq!(slug("Host <-> guest"), "host---guest");
        assert_eq!(slug("[Linked](a.md) *and* __strong__"), "linked-and-strong");
    }

    #[test]
    fn repeated_headings_get_a_number() {
        let found = anchors("# A\n## A\n## A\n");
        assert_eq!(found, ["a", "a-1", "a-2"].into_iter().map(String::from).collect::<BTreeSet<_>>());
        let found = anchors("# A 1\n# A\n# A\n<a id=\"custom\"></a>\n```\n# B\n```\n");
        assert_eq!(found, ["a", "a-1", "a-2", "custom"].into_iter().map(String::from).collect::<BTreeSet<_>>());
    }

    #[test]
    fn list_items_lose_their_marker_and_task_box() {
        assert_eq!(list_item("- [ ] open task"), Some("open task"));
        assert_eq!(list_item("12) twelve"), Some("twelve"));
        assert_eq!(list_item("**Bold:** text"), None);
        assert_eq!(list_item("-flag"), None);
        assert_eq!(scheme("https://example.org"), Some("https"));
        assert_eq!(scheme("C:/x"), None);
    }
}
