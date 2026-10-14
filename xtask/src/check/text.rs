//! Check `text`: is every text file printable ASCII, with LF line ends and a
//! final newline?
//!
//! The defect this check exists to catch: a character that review does not
//! show. A control byte makes grep call the file binary, and `git diff` then
//! prints no diff at all. A CR, a non-ASCII character or a trailing space
//! changes the bytes, but a reader does not see the change. A file without a
//! final newline makes the last line of a diff noisy.
//!
//! This check merges two TEMPLATE scripts: `check-markers.sh` (the character
//! set) and `check-control-bytes.sh` (the control bytes). TEMPLATE allows five
//! marker glyphs. This repository allows none: no marker glyph, no emoji, no
//! byte-order mark. The only exception is a trailing space in Markdown, where
//! two spaces make a line break.
//!
//! The scope is the visible files (tracked, plus untracked that git does not
//! ignore) that `repo::is_text_path` calls text.

use crate::repo::{self, Repo};
use crate::report::{CheckReport, Finding, Status};

pub(crate) const DEFECT: &str =
    "a byte other than tab, LF and printable ASCII, a missing final newline, or a trailing space";

const NAME: &str = "text";

/// This repository has more text files than this. Fewer means that the file
/// list is wrong, not that the tree is clean.
const FLOOR: usize = 30;

/// The findings of one file after this count are counted, not listed.
const MAX_PER_FILE: usize = 20;

pub(crate) fn check(repo: &Repo) -> CheckReport {
    let files = match repo.visible_files() {
        Ok(files) => files,
        Err(e) => return CheckReport::not_run(NAME, format!("cannot list the files: {e}")),
    };
    let mut scanned = 0;
    let mut findings = Vec::new();
    let mut unreadable = Vec::new();
    for rel in files.iter().filter(|f| repo::is_text_path(f)) {
        match repo.read_bytes(rel) {
            Ok(bytes) => {
                scanned += 1;
                findings.extend(scan(rel, &bytes));
            }
            Err(e) => unreadable.push(e),
        }
    }
    finish(NAME, scanned, FLOOR, findings, &unreadable)
}

/// The report of a check that reads many files. A file that the check could
/// not read goes into the notes, and it changes a pass into "not run": a file
/// that nobody read is not a clean file.
pub(super) fn finish(
    name: &'static str,
    scanned: usize,
    floor: usize,
    findings: Vec<Finding>,
    unreadable: &[String],
) -> CheckReport {
    let mut report = CheckReport::from_findings(name, scanned, floor, findings);
    for e in unreadable {
        report.notes.push(format!("could not read {e}"));
    }
    if !unreadable.is_empty() && report.status == Status::Pass {
        report.status = Status::NotRun;
    }
    report
}

/// The findings for the bytes of one file. `rel` decides the trailing-space rule.
fn scan(rel: &str, bytes: &[u8]) -> Vec<Finding> {
    let mut lines: Vec<&[u8]> = bytes.split(|&b| b == b'\n').collect();
    let mut found = Vec::new();
    if bytes.last() == Some(&b'\n') {
        // The final LF ends the last line. The empty piece after it is not a line.
        lines.pop();
    } else if !bytes.is_empty() {
        found.push(Finding::new(rel, lines.len(), "the file does not end with a newline (LF)"));
    }
    let markdown = repo::is_markdown(rel);
    for (index, line) in lines.iter().enumerate() {
        let number = index + 1;
        if let Some(message) = first_bad(line) {
            found.push(Finding::new(rel, number, message));
        }
        if !markdown && let Some(column) = trailing_space(line) {
            found.push(Finding::new(rel, number, format!("column {column}: a trailing space or tab")));
        }
    }
    if found.len() > MAX_PER_FILE {
        let hidden = found.len() - MAX_PER_FILE;
        found.truncate(MAX_PER_FILE);
        found.push(Finding::new(rel, 0, format!("{hidden} more findings in this file are not listed")));
    }
    found
}

/// One unit of a line: a character, or a byte that is not part of valid UTF-8.
#[derive(Debug, Clone, Copy)]
enum Unit {
    Char(char),
    Byte(u8),
}

/// The units of a line, in order. An editor counts columns in these units.
fn units(line: &[u8]) -> impl Iterator<Item = Unit> + '_ {
    line.utf8_chunks()
        .flat_map(|chunk| chunk.valid().chars().map(Unit::Char).chain(chunk.invalid().iter().map(|&b| Unit::Byte(b))))
}

/// Tab and the printable ASCII characters. LF is the line end and never
/// comes into a line.
fn allowed(c: char) -> bool {
    c == '\t' || (' '..='~').contains(&c)
}

/// The first unit of `line` that is not allowed, as a message with its
/// column. The message also counts the other bad units of the line.
fn first_bad(line: &[u8]) -> Option<String> {
    let mut bad = units(line).enumerate().filter(|(_, unit)| !matches!(unit, Unit::Char(c) if allowed(*c)));
    let (index, unit) = bad.next()?;
    let more = bad.count();
    let what = match unit {
        Unit::Char(c) => describe(c),
        Unit::Byte(b) => format!("byte 0x{b:02X} is not valid UTF-8"),
    };
    let column = index + 1;
    Some(if more == 0 {
        format!("column {column}: {what}")
    } else {
        format!("column {column}: {what} ({more} more on this line)")
    })
}

/// The column of the first space or tab of the whitespace at the end of
/// `line`, when the line has such whitespace.
fn trailing_space(line: &[u8]) -> Option<usize> {
    let spaces = line.iter().rev().take_while(|&&b| b == b' ' || b == b'\t').count();
    (spaces > 0).then(|| units(&line[..line.len() - spaces]).count() + 1)
}

/// What is wrong with a character that is not allowed, and what to write.
fn describe(c: char) -> String {
    let code = u32::from(c);
    if code < 0x20 || code == 0x7F {
        let name = match code {
            0x00 => " (NUL)",
            0x07 => " (BEL)",
            0x08 => " (BS)",
            0x0B => " (VT)",
            0x0C => " (FF)",
            0x0D => " (CR)",
            0x1B => " (ESC)",
            0x7F => " (DEL)",
            _ => "",
        };
        let fix = if code == 0x0D { "use LF line ends only" } else { "write the escape, not the byte" };
        return format!("control byte 0x{code:02X}{name}; {fix}");
    }
    let hint = match code {
        0x2012..=0x2015 | 0x2212 => " (a dash); write - or \"to\"",
        0x2018 | 0x2019 | 0x201C | 0x201D => " (a curly quote); write ' or \"",
        0x00A0 | 0x2007 | 0x202F => " (a non-breaking space); write a space",
        0x2190..=0x21FF => " (an arrow); write -> or <-",
        0x00D7 => " (a multiplication sign); write x",
        0x2026 => " (an ellipsis); write ...",
        0x2264 => "; write <=",
        0x2265 => "; write >=",
        0xFEFF => " (a byte-order mark); remove it",
        _ => "",
    };
    format!("U+{code:04X} is not ASCII{hint}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The line and the message of each finding.
    fn found(rel: &str, bytes: &[u8]) -> Vec<(usize, String)> {
        scan(rel, bytes).into_iter().map(|f| (f.line, f.message)).collect()
    }

    fn none() -> Vec<(usize, String)> {
        Vec::new()
    }

    fn one(line: usize, message: &str) -> Vec<(usize, String)> {
        vec![(line, message.to_string())]
    }

    /// A character from its code point. The tests build every bad character
    /// at run time, so this source file stays clean.
    fn chr(code: u32) -> char {
        char::from_u32(code).expect("a valid code point")
    }

    #[test]
    fn clean_ascii_passes() {
        assert_eq!(found("a.rs", b"fn main() {}\n\tlet x = 1;\n~\n"), none());
    }

    #[test]
    fn an_empty_file_passes() {
        assert_eq!(found("a.rs", b""), none());
    }

    #[test]
    fn a_missing_final_newline_is_found_on_the_last_line() {
        let f = found("a.toml", b"a = 1\nb = 2");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].0, 2);
        assert!(f[0].1.contains("newline"));
    }

    #[test]
    fn a_cr_is_found_with_its_column() {
        let bytes = [b'a', b'b', 0x0D, b'\n'];
        let f = found("a.md", &bytes);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].0, 1);
        assert!(f[0].1.starts_with("column 3: control byte 0x0D (CR)"), "{}", f[0].1);
    }

    #[test]
    fn each_control_byte_and_del_is_found() {
        let mut planted: Vec<u8> = (0x00..0x20).filter(|&b| b != b'\t' && b != b'\n').collect();
        planted.push(0x7F);
        for b in planted {
            let bytes = [b'x', b, b'\n'];
            let f = found("a.sh", &bytes);
            assert_eq!(f.len(), 1, "byte {b:#04x}");
            assert!(f[0].1.starts_with(&format!("column 2: control byte 0x{b:02X}")), "{}", f[0].1);
        }
    }

    #[test]
    fn non_ascii_is_found_by_code_point_and_character_column() {
        let dash = chr(0x2014);
        let text = format!("ab{dash}c{dash}\n");
        let f = found("a.md", text.as_bytes());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].1, "column 3: U+2014 is not ASCII (a dash); write - or \"to\" (1 more on this line)");
    }

    #[test]
    fn an_emoji_and_a_byte_order_mark_are_found() {
        let text = format!("{}x\n{}\n", chr(0xFEFF), chr(0x1F600));
        let f = found("a.md", text.as_bytes());
        assert_eq!(f.len(), 2);
        assert!(f[0].1.starts_with("column 1: U+FEFF"), "{}", f[0].1);
        assert_eq!(f[1].1, "column 1: U+1F600 is not ASCII");
    }

    #[test]
    fn a_byte_that_is_not_utf8_is_found() {
        let bytes = [b'a', 0xFF, b'b', b'\n'];
        assert_eq!(found("a.json", &bytes), one(1, "column 2: byte 0xFF is not valid UTF-8"));
    }

    #[test]
    fn a_trailing_space_fails_outside_markdown_only() {
        assert_eq!(found("a.rs", b"let x;  \nok\n"), one(1, "column 7: a trailing space or tab"));
        assert_eq!(found("a.rs", b"x\t\n"), one(1, "column 2: a trailing space or tab"));
        assert_eq!(found("a.md", b"line break  \nnext\n"), none());
    }

    #[test]
    fn the_findings_of_one_file_have_a_ceiling() {
        let text = "x \n".repeat(MAX_PER_FILE + 5);
        let f = found("a.rs", text.as_bytes());
        assert_eq!(f.len(), MAX_PER_FILE + 1);
        assert_eq!(f[MAX_PER_FILE..], one(0, "5 more findings in this file are not listed"));
    }

    #[test]
    fn an_unread_file_is_not_a_pass() {
        let report = finish(NAME, 40, FLOOR, Vec::new(), &["a.rs: denied".to_string()]);
        assert_eq!(report.status, Status::NotRun);
        let report = finish(NAME, 40, FLOOR, vec![Finding::new("b.rs", 1, "m")], &["a.rs: denied".to_string()]);
        assert_eq!(report.status, Status::Fail);
    }

    #[test]
    fn the_sources_of_the_text_and_prose_checks_pass() {
        let sources: [(&str, &[u8]); 5] = [
            ("text.rs", include_bytes!("text.rs")),
            ("prose.rs", include_bytes!("prose.rs")),
            ("prose/inline.rs", include_bytes!("prose/inline.rs")),
            ("prose/rust.rs", include_bytes!("prose/rust.rs")),
            ("prose/tests.rs", include_bytes!("prose/tests.rs")),
        ];
        for (rel, bytes) in sources {
            assert_eq!(found(rel, bytes), none(), "{rel}");
        }
    }

    #[test]
    fn the_check_reads_this_repository_above_the_floor() {
        let repo = Repo::find().expect("the repository root");
        // The copy of the tree that `cargo xtask linux` makes has no .git, so
        // there is no file list to read. The test runs natively and in CI.
        if !repo.root.join(".git").exists() {
            eprintln!("skipped: {} has no .git", repo.root.display());
            return;
        }
        let report = check(&repo);
        assert!(report.scanned >= FLOOR, "{:?}", report.notes);
    }
}
