//! Check `placeholders`: did a template placeholder survive into a real file?
//!
//! The defect this check exists to catch: a document that reads as finished
//! and is not. A placeholder in a router, a record or a license is a sentence
//! that looks true and says nothing, and the next session acts on it. Nothing
//! fails, and the file has the correct shape, so the defect stays quiet.
//!
//! The rules:
//!
//! - a double-brace placeholder: two open braces, a name that starts with an
//!   uppercase letter, and two close braces, on one line;
//! - a template word (YEAR, AUTHOR or EMAIL) on the copyright line of a license;
//! - a stand-in marker, for example the replace marker or the change-me marker;
//! - a template guidance comment, which speaks to the person who filled in the
//!   file and not to the person who reads it now.
//!
//! The scope is the text files of the tracked files plus the untracked files
//! that git does not ignore.
//!
//! NOTE: This file does not hold these shapes as text. It joins each marker
//! from two halves, and the tests build each planted value at run time, so the
//! check can read its own source.

use std::ops::Range;

use super::secrets::Scope;
use crate::repo::Repo;
use crate::report::{CheckReport, Finding};

pub(crate) const DEFECT: &str = "a template placeholder that survived into a real file";

const NAME: &str = "placeholders";

/// Fewer text files than this means that the scope is wrong, not that the tree is clean.
const FLOOR: usize = 20;

/// The two halves of a double-brace placeholder.
const OPEN: &str = "{{";
const CLOSE: &str = "}}";

/// The longest text between the braces that the rule accepts. A longer span is code.
const INNER_MAX: usize = 80;

/// Stand-in markers. Each one means "somebody meant to change this". Two
/// halves make each marker, so this file does not hold it.
const MARKERS: [&str; 6] = [
    concat!("<REPL", "ACE"),
    concat!("CHANGE", "ME"),
    concat!("YOUR", "_NAME"),
    concat!("YOUR", "_EMAIL"),
    concat!("YOUR", "_PROJECT"),
    concat!("TODO", ": fill"),
];

/// The template words of a copyright line.
const LICENSE_WORDS: [&str; 3] = ["YEAR", "AUTHOR", "EMAIL"];

/// The start of an HTML comment, and the word that makes it a guidance comment.
const COMMENT_OPEN: &str = "<!--";
const GUIDANCE: &str = "TEMPLATE";

pub(crate) fn check(repo: &Repo) -> CheckReport {
    let files = match repo.visible_files() {
        Ok(files) => files,
        Err(e) => return CheckReport::not_run(NAME, format!("cannot list the files of the repository: {e}")),
    };
    let mut findings = Vec::new();
    let mut scope = Scope::default();
    for rel in &files {
        if let Some(text) = scope.read(repo, rel) {
            findings.extend(scan_text(rel, &text));
        }
    }
    scope.close(CheckReport::from_findings(NAME, scope.text_files, FLOOR, findings))
}

/// The findings in the text of one file. Line numbers start at 1.
fn scan_text(rel: &str, text: &str) -> Vec<Finding> {
    let license = is_license(rel);
    let mut out = Vec::new();
    for (index, raw) in text.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let number = index + 1;
        for span in double_braces(line) {
            out.push(Finding::new(rel, number, format!("a double-brace placeholder survived: {}", &line[span])));
        }
        for marker in MARKERS.iter().filter(|m| line.contains(*m)) {
            out.push(Finding::new(rel, number, format!("a stand-in marker survived: {marker}")));
        }
        if is_guidance(line) {
            out.push(Finding::new(rel, number, "a template guidance comment survived"));
        }
        let words = if license { license_words(line) } else { Vec::new() };
        if !words.is_empty() {
            let message = format!("the copyright line keeps the template words {}", words.join(", "));
            out.push(Finding::new(rel, number, message));
        }
    }
    out
}

/// The byte span of each double-brace placeholder in a line.
fn double_braces(line: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    for (at, _) in line.match_indices(OPEN) {
        // A dollar sign before the braces makes a GitHub Actions expression.
        if line[..at].ends_with('$') {
            continue;
        }
        let rest = &line[at + OPEN.len()..];
        // A Go template starts with a dot or a lowercase word. A placeholder starts with an uppercase letter.
        if !rest.trim_start_matches([' ', '\t']).starts_with(|c: char| c.is_ascii_uppercase()) {
            continue;
        }
        let Some(end) = rest.find(CLOSE) else { continue };
        let inner = &rest[..end];
        if inner.len() <= INNER_MAX && !inner.contains(['{', '}']) {
            out.push(at..at + OPEN.len() + end + CLOSE.len());
        }
    }
    out
}

/// True for a template guidance comment: an HTML comment that starts with the guidance word.
fn is_guidance(line: &str) -> bool {
    line.match_indices(COMMENT_OPEN).any(|(at, _)| line[at + COMMENT_OPEN.len()..].trim_start().starts_with(GUIDANCE))
}

/// True for a license file: `LICENSE`, `LICENSE.md`, `LICENSE-MIT` or `COPYING`.
fn is_license(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name == "LICENSE" || name == "COPYING" || name.starts_with("LICENSE.") || name.starts_with("LICENSE-")
}

/// The template words on a copyright line, as whole uppercase words. Only the
/// copyright line counts: the warranty text of 0BSD and ISC names THE AUTHOR,
/// and that is correct license text, not a placeholder.
fn license_words(line: &str) -> Vec<&'static str> {
    if !line.to_ascii_lowercase().contains("copyright") {
        return Vec::new();
    }
    LICENSE_WORDS.into_iter().filter(|w| has_whole_word(line, w)).collect()
}

/// True when `word` is in `line` with no letter, digit or underscore next to it.
fn has_whole_word(line: &str, word: &str) -> bool {
    let bytes = line.as_bytes();
    let joined = |c: &u8| c.is_ascii_alphanumeric() || *c == b'_';
    line.match_indices(word).any(|(at, _)| {
        let before = at.checked_sub(1).and_then(|i| bytes.get(i));
        !before.is_some_and(joined) && !bytes.get(at + word.len()).is_some_and(joined)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Status;

    /// A double-brace placeholder, built at run time.
    fn braced(inner: &str) -> String {
        [OPEN, inner, CLOSE].concat()
    }

    /// No finding, no word, no span.
    const NONE: [&str; 0] = [];
    const NO_SPANS: [Range<usize>; 0] = [];

    fn messages(rel: &str, text: &str) -> Vec<String> {
        scan_text(rel, text).into_iter().map(|f| f.message).collect()
    }

    #[test]
    fn finds_a_double_brace_placeholder() {
        let line = format!("Name: {}", braced("PROJECT_NAME"));
        let spans = double_braces(&line);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0], 6..line.len());
        assert_eq!(double_braces(&braced(" PROJECT_NAME ")).len(), 1);
        assert_eq!(double_braces(&braced("One sentence: what this does")).len(), 1);
    }

    #[test]
    fn an_expression_or_a_go_template_is_not_a_placeholder() {
        for line in [
            format!("token: ${}", braced(" secrets.TOKEN ")),
            braced(" .Host.Arch "),
            braced("json .Config.Env"),
            braced("{}"),
        ] {
            assert_eq!(double_braces(&line), NO_SPANS, "{line}");
        }
    }

    #[test]
    fn finds_the_template_words_on_the_copyright_line_only() {
        let [year, author, email] = LICENSE_WORDS;
        let line = format!("Copyright (C) {year} by {author} {email}");
        assert_eq!(license_words(&line), LICENSE_WORDS);
        assert_eq!(license_words("Copyright (C) 2026 Azathothas"), NONE);
        assert_eq!(license_words(&format!("IN NO EVENT SHALL THE {author} BE LIABLE")), NONE);
        assert_eq!(license_words(&format!("Copyright (C) 2026 the {author}S")), NONE);
    }

    #[test]
    fn the_license_rule_reads_license_files_only() {
        let text = format!("Copyright (C) {} by a person\n", LICENSE_WORDS[0]);
        assert_eq!(messages("LICENSE", &text).len(), 1);
        assert_eq!(messages("docs/conventions.md", &text), NONE);
    }

    #[test]
    fn finds_a_stand_in_marker_and_a_guidance_comment() {
        for marker in MARKERS {
            assert_eq!(messages("README.md", &format!("value = {marker}\n")).len(), 1, "{marker}");
        }
        let comment = format!("{COMMENT_OPEN} {GUIDANCE}: fill in this section -->");
        assert_eq!(messages("README.md", &comment), ["a template guidance comment survived"]);
    }

    #[test]
    fn reports_the_line_of_each_finding() {
        let found = scan_text("a.md", &format!("one\r\ntwo {}\r\nthree\n", braced("X")));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 2);
    }

    /// The check on this repository. The tree changes, so the test asserts the
    /// contract only: a pass always comes with a scope at the floor or above.
    #[test]
    fn the_check_on_this_repository_keeps_the_floor_contract() {
        let Ok(repo) = Repo::find() else { return };
        let report = check(&repo);
        report.print_text();
        assert!(report.status != Status::Pass || report.scanned >= FLOOR);
    }
}
