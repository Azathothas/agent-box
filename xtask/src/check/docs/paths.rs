//! Rule 5 of the docs check: a path of this repository in inline code exists.
//!
//! The defect: a document names a file in inline code, for example
//! `xtask/src/report.rs:40`, and a later change moves the file or the line.
//! The link rules do not read inline code, so the name goes stale without a
//! finding.
//!
//! A code span names a path of this repository when it has a `/` and its first
//! part is a top-level directory of the repository, or when it starts with
//! `./` or `../` (a path relative to the document). A span with a space or a
//! pattern character (`<`, `*`, `{`, `$`, `%`) is a command or a pattern, and
//! a span with `@` cites another project. A `:N` or `:N-M` after the path is
//! a line or a range of lines, and each line must exist.

use std::collections::BTreeSet;

use super::markdown::{self, Block};
use super::{Outcome, Scope, is_dir, resolve};
use crate::report::Finding;

/// The characters that make a code span a command, a pattern or a citation
/// of another project, not a path of this repository.
const NOT_A_PATH: [char; 13] = [' ', '\t', '<', '>', '*', '{', '}', '$', '%', '?', '@', '\\', '|'];

/// Check the paths in the inline code spans of one block of the file `from`.
/// `tops` holds the top-level directories of the repository.
pub(super) fn check_block(from: &str, block: &Block, tops: &BTreeSet<&str>, scope: &Scope<'_>, out: &mut Outcome) {
    for (span, ticks) in markdown::code_spans(&block.text) {
        let content = block.text[span.start + ticks..span.end - ticks].trim();
        let Some((path, lines)) = repo_path(from, content, tops) else {
            continue;
        };
        out.paths += 1;
        let at = block.line + block.text[..span.start].matches('\n').count();
        let problem = if scope.files.contains(&path) {
            lines.and_then(|lines| (scope.lines)(&path).and_then(|count| line_problem(content, lines, count)))
        } else if is_dir(scope.files, &path) {
            None
        } else if path == content {
            // The most frequent cause is a file of another project without its citation.
            Some(format!(
                "`{content}` is not in this repository; cite a file of another project as `owner/repo@commit:path`"
            ))
        } else {
            Some(format!("`{content}` goes to `{path}`, which is not a file or a directory of this repository"))
        };
        if let Some(problem) = problem {
            out.findings.push(Finding::new(from, at, problem));
        }
    }
}

/// The path relative to the root and the line part, when the code span
/// `span` in the file `from` names a path of this repository.
fn repo_path<'s>(from: &str, span: &'s str, tops: &BTreeSet<&str>) -> Option<(String, Option<&'s str>)> {
    if span.contains(NOT_A_PATH) {
        return None;
    }
    let (path, lines) = span.split_once(':').map_or((span, None), |(path, lines)| (path, Some(lines)));
    let path = path.split_once('#').map_or(path, |(path, _)| path);
    if path.starts_with("./") || path.starts_with("../") {
        // A relative path that goes above the root is not a path of this repository.
        return resolve(from, path).map(|resolved| (resolved, lines));
    }
    let (top, _) = path.split_once('/')?;
    tops.contains(top).then(|| (path.trim_end_matches('/').to_string(), lines))
}

/// The problem with the line part `lines` (`12`, `12-20` or `12:5`) of the
/// span `span`, for a file of `count` lines. A line part that is not a number
/// is not checked.
fn line_problem(span: &str, lines: &str, count: usize) -> Option<String> {
    // A column after the line (`12:5`) is not checked.
    let range = lines.split_once(':').map_or(lines, |(range, _)| range);
    for part in range.split('-') {
        let Ok(number) = part.parse::<usize>() else {
            return None;
        };
        if number == 0 || number > count {
            return Some(format!("`{span}` names line {number}, and the file has {count} lines"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tops() -> BTreeSet<&'static str> {
        ["xtask", "docs"].into_iter().collect()
    }

    #[test]
    fn a_span_is_a_path_only_when_it_looks_like_one() {
        let tops = tops();
        let found = |span| repo_path("docs/a.md", span, &tops);
        assert_eq!(found("xtask/src/main.rs:12"), Some(("xtask/src/main.rs".to_string(), Some("12"))));
        assert_eq!(found("docs/"), Some(("docs".to_string(), None)));
        assert_eq!(found("../xtask/src/main.rs"), Some(("xtask/src/main.rs".to_string(), None)));
        assert_eq!(found("./b.md#part"), Some(("docs/b.md".to_string(), None)));
        assert_eq!(found("../../above.md"), None);
        assert_eq!(found("cargo xtask check docs"), None);
        assert_eq!(found("crates/<each>/Cargo.toml"), None);
        assert_eq!(found("xtask/src/*.rs"), None);
        assert_eq!(found("owner/repo@main:xtask/a.rs:1"), None);
        assert_eq!(found("elsewhere/file.rs"), None);
        assert_eq!(found("main.rs"), None);
    }

    #[test]
    fn a_line_must_exist() {
        assert_eq!(line_problem("a:3", "3", 3), None);
        assert_eq!(line_problem("a:3-4", "3-4", 4), None);
        assert_eq!(line_problem("a:2:7", "2:7", 2), None);
        assert!(line_problem("a:9", "9", 3).is_some_and(|m| m.contains("line 9")));
        assert!(line_problem("a:2-9", "2-9", 3).is_some());
        assert!(line_problem("a:0", "0", 3).is_some());
        assert_eq!(line_problem("a::run", ":run", 3), None);
    }
}
