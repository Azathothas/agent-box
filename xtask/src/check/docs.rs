//! `cargo xtask check docs`: the links of the Markdown files resolve.
//!
//! The defect: a document that was true when somebody wrote it. A rename
//! breaks a link, a new heading text breaks an anchor, and a page that nothing
//! links to is not read, so nobody corrects it. The other checks do not see
//! these defects.
//!
//! The rules, over each visible Markdown file (tracked, or untracked and not
//! ignored):
//!
//! 1. A relative link `[text](target)`, an image, and a link definition
//!    `[label]: target` go to a file or a directory of the repository. The
//!    check does not follow http, https and mailto links.
//! 2. An anchor `#name` is a heading slug of the target Markdown file, as
//!    GitHub makes it. A link that is only `#name` goes to its own file.
//! 3. Each Markdown file under `docs/` and `TODO/` has a link from another
//!    Markdown file. A link to a directory marks no file in it.
//! 4. A link in fenced code or in inline code is not a link.
//! 5. A path of this repository in inline code exists (see `paths`).
//!
//! A target exists when it is a visible file, or a directory that holds one,
//! with the same letter case. The check does not ask the disk: a file that
//! git ignores is not on GitHub, and Windows ignores letter case, so the disk
//! accepts links that GitHub breaks.

pub(super) mod markdown;
mod paths;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use crate::repo::{Repo, is_markdown};
use crate::report::{CheckReport, Finding};
use markdown::Kind;

const NAME: &str = "docs";
/// Fewer Markdown files than this means that the scope is wrong.
const FLOOR: usize = 10;
/// The pages that a reader opens first. They need no link from another file.
const ROOTS: [&str; 3] = ["README.md", "AGENTS.md", "SECURITY.md"];
/// Each Markdown file under these directories needs a link from another file.
const LINKED_DIRS: [&str; 2] = ["docs/", "TODO/"];
/// The links that go out of the repository. The check does not follow them.
const EXTERNAL: [&str; 3] = ["http://", "https://", "mailto:"];

pub(crate) const DEFECT: &str =
    "a document that was true when somebody wrote it: a broken link or anchor, a stale path, a page without a link";

pub(crate) fn check(repo: &Repo) -> CheckReport {
    match repo.visible_files() {
        Ok(files) => run(repo, files),
        Err(e) => CheckReport::not_run(NAME, format!("cannot list the files of the repository: {e}")),
    }
}

/// Read the Markdown files among `files` (paths relative to the root of
/// `repo`) and apply the rules.
fn run(repo: &Repo, files: Vec<String>) -> CheckReport {
    let mut docs = BTreeMap::new();
    for rel in files.iter().filter(|f| is_markdown(f)) {
        match repo.read_bytes(rel) {
            // Bytes that are not UTF-8 are a finding of the `text` check. This check reads past them.
            Ok(bytes) => {
                docs.insert(rel.clone(), String::from_utf8_lossy(&bytes).into_owned());
            }
            Err(e) => return CheckReport::not_run(NAME, format!("cannot read a Markdown file: {e}")),
        }
    }
    let files: BTreeSet<String> = files.into_iter().collect();
    let lines = |rel: &str| {
        docs.get(rel).map_or_else(
            || repo.read_bytes(rel).ok().map(|bytes| String::from_utf8_lossy(&bytes).lines().count()),
            |text| Some(text.lines().count()),
        )
    };
    let outcome = check_docs(&Scope { docs: &docs, files: &files, lines: &lines });
    let note = format!(
        "checked {} relative links, {} anchors and {} repository paths",
        outcome.links, outcome.anchors, outcome.paths
    );
    CheckReport::from_findings(NAME, docs.len(), FLOOR, outcome.findings).note(note)
}

/// What the rules read: each Markdown file with its text, each visible file,
/// and the number of lines of a file (`None` when the file cannot be read).
struct Scope<'a> {
    docs: &'a BTreeMap<String, String>,
    files: &'a BTreeSet<String>,
    lines: &'a dyn Fn(&str) -> Option<usize>,
}

/// What the rules found, and how many items they checked.
#[derive(Debug, Default)]
struct Outcome {
    findings: Vec<Finding>,
    links: usize,
    anchors: usize,
    paths: usize,
}

/// Apply the rules to the documents of `scope`.
fn check_docs(scope: &Scope<'_>) -> Outcome {
    let anchors = scope.docs.iter().map(|(path, text)| (path.as_str(), markdown::anchors(text))).collect();
    let tops: BTreeSet<&str> = scope.files.iter().filter_map(|f| f.split_once('/').map(|(top, _)| top)).collect();
    let mut walk = Walk { files: scope.files, anchors, linked: BTreeSet::new(), out: Outcome::default() };
    for (from, text) in scope.docs {
        for block in markdown::blocks(text) {
            if block.kind == Kind::Code {
                continue;
            }
            let prose = markdown::blank_code_spans(&block.text);
            for (offset, line) in prose.split('\n').enumerate() {
                for target in link_targets(line) {
                    walk.link(from, block.line + offset, target);
                }
            }
            // A block quote quotes another source: its paths are not paths of this repository.
            if block.kind != Kind::Quote {
                paths::check_block(from, &block, &tops, scope, &mut walk.out);
            }
        }
    }
    let Walk { linked, mut out, .. } = walk;
    for path in scope.docs.keys() {
        let name = path.rsplit('/').next().unwrap_or(path);
        if LINKED_DIRS.iter().any(|dir| path.starts_with(dir)) && !ROOTS.contains(&name) && !linked.contains(path) {
            let message =
                "no other Markdown file links to this page: a page without a link is not read, so nobody corrects it";
            out.findings.push(Finding::new(path.as_str(), 0, message));
        }
    }
    out.findings.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    out
}

/// The state of the walk over the links: the anchors of each document, the
/// files that have a link from another file, and the outcome.
struct Walk<'a> {
    files: &'a BTreeSet<String>,
    anchors: BTreeMap<&'a str, BTreeSet<String>>,
    linked: BTreeSet<String>,
    out: Outcome,
}

impl Walk<'_> {
    fn finding(&mut self, from: &str, line: usize, message: String) {
        self.out.findings.push(Finding::new(from, line, message));
    }

    /// Check one link target that `from` has on line `line`.
    fn link(&mut self, from: &str, line: usize, target: &str) {
        let lower = target.to_ascii_lowercase();
        if EXTERNAL.iter().any(|prefix| lower.starts_with(*prefix)) {
            return;
        }
        if let Some(scheme) = markdown::scheme(target) {
            self.finding(
                from,
                line,
                format!("the link `{target}` has the scheme `{scheme}:`, not http, https or mailto"),
            );
            return;
        }
        let (path, anchor) = target.split_once('#').unwrap_or((target, ""));
        let path = path.split_once('?').map_or(path, |(path, _)| path);
        if path.is_empty() && anchor.is_empty() {
            self.finding(from, line, "a link with an empty target".to_string());
            return;
        }
        self.out.links += 1;
        let resolved = if path.is_empty() { Some(from.to_string()) } else { resolve(from, &percent_decode(path)) };
        let Some(resolved) = resolved else {
            self.finding(from, line, format!("the link `{target}` goes above the root of the repository"));
            return;
        };
        let is_file = self.files.contains(&resolved);
        if !is_file && !is_dir(self.files, &resolved) {
            let mut message = format!("the link `{target}` goes to `{resolved}`, which does not exist");
            if let Some(near) = self.files.iter().find(|f| f.eq_ignore_ascii_case(&resolved)) {
                // GitHub compares letter case. Windows does not, so the link works there and nowhere else.
                message = format!("{message}; `{near}` has a different letter case");
            }
            self.finding(from, line, message);
            return;
        }
        if is_file && resolved != from {
            self.linked.insert(resolved.clone());
        }
        // Only a Markdown file has heading anchors, and `anchors` holds each Markdown file.
        if anchor.is_empty() || !is_file {
            return;
        }
        let Some(slugs) = self.anchors.get(resolved.as_str()) else {
            return;
        };
        self.out.anchors += 1;
        let name = percent_decode(anchor);
        if slugs.contains(&name) {
            return;
        }
        let lower = name.to_lowercase();
        let hint = if slugs.contains(&lower) { format!("; the slug is `#{lower}`") } else { String::new() };
        self.finding(from, line, format!("the anchor `#{name}` is not a heading of `{resolved}`{hint}"));
    }
}

/// The link targets of one line of prose: inline links `[text](target)`,
/// images, and a link definition `[label]: target`.
fn link_targets(line: &str) -> Vec<&str> {
    let mut out: Vec<&str> = definition(line).into_iter().collect();
    let mut rest = line;
    while let Some(index) = rest.find("](") {
        rest = &rest[index + 2..];
        if let Some((target, used)) = destination(rest) {
            out.push(target);
            rest = &rest[used..];
        }
    }
    out
}

/// The destination at the start of `text` (the part after `](`), and the
/// number of bytes that it uses. A destination in `<...>` can hold spaces,
/// and a bare destination can hold balanced parentheses. `None` at the end of
/// the line: the rest of such a link is on the next line, and the check does
/// not follow it.
fn destination(text: &str) -> Option<(&str, usize)> {
    let body = text.trim_start_matches([' ', '\t']);
    if body.is_empty() {
        return None;
    }
    let skipped = text.len() - body.len();
    if let Some(inner) = body.strip_prefix('<')
        && let Some(end) = inner.find('>')
    {
        return Some((&inner[..end], skipped + end + 2));
    }
    let mut depth = 0usize;
    let mut escaped = false;
    let mut end = body.len();
    for (index, c) in body.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '(' {
            depth += 1;
        } else if c == ')' && depth > 0 {
            depth -= 1;
        } else if c == ')' || c.is_whitespace() {
            end = index;
            break;
        }
    }
    Some((&body[..end], skipped + end))
}

/// The target of a link definition `[label]: target`, or `None`. A footnote
/// (`[^1]: text`) is not a link.
fn definition(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix('[')?;
    if rest.starts_with('^') {
        return None;
    }
    let (label, after) = rest.split_once("]:")?;
    if label.is_empty() || label.contains(['[', ']']) {
        return None;
    }
    let after = after.trim_start();
    if let Some(inner) = after.strip_prefix('<') {
        inner.find('>').map(|end| &inner[..end])
    } else {
        after.split_whitespace().next()
    }
}

/// The path of `target` relative to the root, for a link in the file `from`.
/// A target that starts with `/` starts at the root, as GitHub reads it.
/// `None` when the target goes above the root.
fn resolve(from: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    if !target.starts_with('/') {
        parts.extend(from.split('/'));
        // The last part is the name of the file that has the link.
        parts.pop();
    }
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(parts.join("/"))
}

/// True when `dir` is the root, or a directory that holds a visible file.
fn is_dir(files: &BTreeSet<String>, dir: &str) -> bool {
    if dir.is_empty() {
        return true;
    }
    let prefix = format!("{dir}/");
    files
        .range::<str, _>((Bound::Included(prefix.as_str()), Bound::Unbounded))
        .next()
        .is_some_and(|f| f.starts_with(&prefix))
}

/// Decode the `%XX` escapes of a link (`my%20file.md` is `my file.md`). A
/// result that is not UTF-8 gives the text back as it is.
fn percent_decode(text: &str) -> String {
    let hex = |byte: u8| char::from(byte).to_digit(16).and_then(|digit| u8::try_from(digit).ok());
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut pos = 0;
    while let Some(&byte) = bytes.get(pos) {
        if byte == b'%'
            && let Some(high) = bytes.get(pos + 1).and_then(|&b| hex(b))
            && let Some(low) = bytes.get(pos + 2).and_then(|&b| hex(b))
        {
            out.push(high * 16 + low);
            pos += 3;
        } else {
            out.push(byte);
            pos += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}
