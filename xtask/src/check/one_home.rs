//! `cargo xtask check one-home`: each fact has one home.
//!
//! The defect: one fact with two homes. A sentence that somebody copies into
//! a second document drifts: a later change corrects one copy, and the reader
//! trusts the other. This check finds each sentence of 12 words or more that
//! is in two or more Markdown files, and it reports each location of it.
//!
//! The check compares sentences after it normalizes them: lowercase, no
//! emphasis marks, the text of a link without its target, the content of
//! inline code without the backticks, one space between words, and no
//! punctuation at the ends. Fenced code, block quotes, headings and table rows
//! are not sentences. A fact in different words is not a finding: a review
//! finds that, not this check.

use std::collections::{BTreeMap, BTreeSet};

use super::docs::markdown::{self, Kind};
use crate::repo::{Repo, is_markdown};
use crate::report::{CheckReport, Finding};

const NAME: &str = "one-home";
/// A constant, not a flag: a limit that a command line can change gets raised
/// instead of met. Two documents do not share 12 words in sequence by chance.
const MIN_WORDS: usize = 12;
/// Fewer Markdown files than this means that the scope is wrong.
const FLOOR: usize = 10;
/// The number of characters of a sentence that a finding shows.
const EXCERPT: usize = 72;

pub(crate) const DEFECT: &str =
    "one fact with two homes: a sentence of 12 words or more in two Markdown files, where the copies drift apart";

pub(crate) fn check(repo: &Repo) -> CheckReport {
    let files = match repo.visible_files() {
        Ok(files) => files,
        Err(e) => return CheckReport::not_run(NAME, format!("cannot list the files of the repository: {e}")),
    };
    let mut docs = BTreeMap::new();
    for rel in files.into_iter().filter(|f| is_markdown(f)) {
        match repo.read_bytes(&rel) {
            // Bytes that are not UTF-8 are a finding of the `text` check. This check reads past them.
            Ok(bytes) => {
                docs.insert(rel, String::from_utf8_lossy(&bytes).into_owned());
            }
            Err(e) => return CheckReport::not_run(NAME, format!("cannot read a Markdown file: {e}")),
        }
    }
    report(&docs)
}

/// The report over a set of documents (path and text).
fn report(docs: &BTreeMap<String, String>) -> CheckReport {
    let (findings, compared) = duplicates(docs);
    let note = format!("compared {compared} sentences of {MIN_WORDS} words or more");
    CheckReport::from_findings(NAME, docs.len(), FLOOR, findings).note(note)
}

/// One finding for each location of each sentence that is in two or more
/// files, and the number of sentences that the check compared.
fn duplicates(docs: &BTreeMap<String, String>) -> (Vec<Finding>, usize) {
    let mut homes: BTreeMap<String, Vec<(&str, usize)>> = BTreeMap::new();
    let mut compared = 0;
    for (path, text) in docs {
        for (line, sentence) in sentences(text) {
            compared += 1;
            homes.entry(sentence).or_default().push((path.as_str(), line));
        }
    }
    let mut findings = Vec::new();
    for (sentence, places) in &homes {
        let files: BTreeSet<&str> = places.iter().map(|&(path, _)| path).collect();
        if files.len() < 2 {
            continue;
        }
        let excerpt = excerpt(sentence);
        for (index, &(path, line)) in places.iter().enumerate() {
            let others: Vec<String> = places
                .iter()
                .enumerate()
                .filter(|&(other, _)| other != index)
                .map(|(_, (other_path, other_line))| format!("{other_path}:{other_line}"))
                .collect();
            let message = format!("the sentence \"{excerpt}\" is also at {}", others.join(", "));
            findings.push(Finding::new(path, line, message));
        }
    }
    findings.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    (findings, compared)
}

/// The normalized sentences of a document that have `MIN_WORDS` words or
/// more, each with the number of the line where it starts.
fn sentences(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    // Only paragraphs and list items hold sentences: code, quotes, headings and tables do not.
    for block in markdown::blocks(text).into_iter().filter(|block| block.kind == Kind::Text) {
        let plain = markdown::plain_text(&block.text);
        for (start, raw) in split_sentences(&plain) {
            let sentence = normalize(raw);
            if sentence.split(' ').count() >= MIN_WORDS {
                out.push((block.line + plain[..start].matches('\n').count(), sentence));
            }
        }
    }
    out
}

/// Split plain text into sentences, each with its byte offset. A sentence
/// ends at `.`, `!`, `?` or `:` before white space or the end of the text. A
/// closing mark after the end (`.)` or `."`) stays with the sentence.
fn split_sentences(text: &str) -> Vec<(usize, &str)> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if !matches!(c, '.' | '!' | '?' | ':') {
            continue;
        }
        while let Some(&(_, next)) = chars.peek()
            && matches!(next, '.' | '!' | '?' | ':' | ')' | ']' | '"' | '\'')
        {
            chars.next();
        }
        let end = chars.peek().map_or(text.len(), |&(index, _)| index);
        if end == text.len() || text[end..].starts_with(char::is_whitespace) {
            pieces.push((start, &text[start..end]));
            start = end;
        }
    }
    pieces.push((start, &text[start..]));
    // The white space before a sentence is not part of it, and does not move its line.
    pieces
        .into_iter()
        .map(|(offset, piece)| {
            let trimmed = piece.trim_start();
            (offset + piece.len() - trimmed.len(), trimmed)
        })
        .filter(|(_, piece)| !piece.trim().is_empty())
        .collect()
}

/// Lowercase, one space between words, and no punctuation at either end.
fn normalize(sentence: &str) -> String {
    let words: Vec<&str> = sentence.split_whitespace().collect();
    let joined = words.join(" ").to_lowercase();
    joined.trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace()).to_string()
}

/// The start of a sentence for a finding.
fn excerpt(sentence: &str) -> String {
    if sentence.chars().count() <= EXCERPT {
        return sentence.to_string();
    }
    let cut: String = sentence.chars().take(EXCERPT).collect();
    format!("{}...", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Status;

    /// A sentence of 17 words.
    const FACT: &str =
        "The daemon keeps one record for each instance and writes it before the helper starts the engine.";

    fn docs(entries: &[(&str, String)]) -> BTreeMap<String, String> {
        entries.iter().map(|(path, text)| ((*path).to_string(), text.clone())).collect()
    }

    fn locations(found: &[Finding]) -> Vec<String> {
        found.iter().map(Finding::location).collect()
    }

    #[test]
    fn a_sentence_in_two_files_is_reported_at_each_location() {
        let docs = docs(&[
            ("a.md", format!("# A\n\nIntro text. {FACT}\n")),
            ("b.md", format!("Some words\nand more words.\n{FACT}\n")),
            ("c.md", format!("- {FACT}\n")),
        ]);
        let (found, compared) = duplicates(&docs);
        assert_eq!(locations(&found), ["a.md:3", "b.md:3", "c.md:1"]);
        assert!(found[0].message.ends_with("is also at b.md:3, c.md:1"), "{}", found[0].message);
        assert!(found[0].message.contains("\"the daemon keeps one record"));
        assert_eq!(compared, 3);
    }

    #[test]
    fn normalization_ignores_case_marks_links_code_and_spacing() {
        let marked = "The **daemon** keeps one [record](docs/x.md) for each   instance and _writes_ it\n\
                      before the `helper` starts the engine!";
        let docs = docs(&[("a.md", marked.to_string()), ("b.md", FACT.to_string())]);
        assert_eq!(locations(&duplicates(&docs).0), ["a.md:1", "b.md:1"]);
    }

    #[test]
    fn short_sentences_and_repeats_in_one_file_are_not_findings() {
        let short = "The guest reads its own record when the daemon asks it.";
        let docs = docs(&[("a.md", format!("{short}\n\n{FACT}\n\n{FACT}\n")), ("b.md", format!("{short}\n"))]);
        assert_eq!(duplicates(&docs).0.len(), 0);
    }

    #[test]
    fn code_quotes_headings_and_tables_are_not_sentences() {
        let other = format!("```\n{FACT}\n```\n\n> {FACT}\n\n# {FACT}\n\n| {FACT} |\n| --- |\n\n{FACT}\n===\n");
        let docs = docs(&[("a.md", FACT.to_string()), ("b.md", other)]);
        assert_eq!(duplicates(&docs).0.len(), 0);
    }

    #[test]
    fn sentences_split_at_end_marks_and_keep_their_offset() {
        let found = split_sentences("One. Two (x.) three: four\nfive");
        assert_eq!(found, [(0, "One."), (5, "Two (x.)"), (14, "three:"), (21, "four\nfive")]);
        assert_eq!(split_sentences("Version 1.2 is out.  "), [(0, "Version 1.2 is out.")]);
        assert_eq!(normalize("(The  Daemon\nstarts.)"), "the daemon starts");
        // The cut falls after a space, which goes away before the "...".
        assert_eq!(excerpt(&"abc ".repeat(30)).len(), EXCERPT - 1 + 3);
    }

    #[test]
    fn below_the_floor_is_not_a_pass() {
        let few = docs(&[("a.md", FACT.to_string()), ("b.md", String::new())]);
        assert_eq!(report(&few).status, Status::NotRun);
        let entries: Vec<(String, String)> = (0..10).map(|n| (format!("{n}.md"), format!("Page {n}.\n"))).collect();
        let enough: BTreeMap<String, String> = entries.into_iter().collect();
        assert_eq!(report(&enough).status, Status::Pass);
    }
}
