//! The pins table of docs/references.md, parsed from text.
//!
//! The defect this module exists to catch: a pin that a person reads one way
//! and a program reads another way. The table is the one record of the
//! commit that each citation names. So the parser refuses a row that it cannot
//! read, and it does not guess.
//!
//! The format: a section `## Pins` with a table whose header names the
//! columns `Reference`, `Commit`, `Date` and `Why it is pinned`. The first cell
//! of a row is `` `owner/repo` `` with an optional label after it. The commit
//! cell is `` `<40 hex>` ``, or a text such as `release v0.9.0`, which `refs`
//! lists and skips.

use std::collections::HashMap;

/// The heading of the section that holds the table.
pub(crate) const HEADING: &str = "## Pins";

/// One row of the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    /// The line of the row in the file, from 1.
    pub(crate) line: usize,
    /// `owner/repo`, as written.
    pub(crate) repo: String,
    /// The text of the first cell after the code span, for example `(old)`.
    pub(crate) label: String,
    /// The commit cell as written, without backticks.
    pub(crate) commit_cell: String,
    /// The commit in lowercase when the cell is 40 hex digits. `None` is a
    /// row that `refs` lists and skips.
    pub(crate) commit: Option<String>,
    pub(crate) date: String,
    pub(crate) reason: String,
    /// The directory under .refs/: `owner__repo`. A second pin of the same
    /// repository at another commit gets `owner__repo@<12 hex>`.
    pub(crate) dir: String,
}

/// The rows that parse, and a problem for each row that does not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Table {
    pub(crate) rows: Vec<Row>,
    /// The line, from 1, and what is wrong with it.
    pub(crate) problems: Vec<(usize, String)>,
}

/// Where each column is in a row.
struct Columns {
    width: usize,
    reference: usize,
    commit: usize,
    date: Option<usize>,
    reason: Option<usize>,
}

/// Parse the pins table. An error means there is no table to read: no
/// section, no table in it, or a header without the two required columns.
pub(crate) fn parse(text: &str) -> Result<Table, String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = section_start(&lines).ok_or_else(|| format!("there is no `{HEADING}` section"))?;
    let mut rest = lines.iter().enumerate().skip(start);
    // The header is the first table line after the heading, before the next heading.
    let (header_at, header) = loop {
        let Some((i, l)) = rest.next() else {
            return Err(format!("the `{HEADING}` section has no table"));
        };
        if l.starts_with('#') {
            return Err(format!("the `{HEADING}` section has no table"));
        }
        if l.trim_start().starts_with('|') {
            break (i, cells(l));
        }
    };
    let columns = columns(&header)
        .ok_or_else(|| format!("line {}: the table header has no `Reference` and `Commit` columns", header_at + 1))?;
    let separator = rest.next().map(|(_, l)| cells(l)).unwrap_or_default();
    if separator.is_empty() || !separator.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':'))) {
        return Err(format!("line {}: the table header has no separator row after it", header_at + 1));
    }
    let mut table = Table::default();
    for (i, l) in rest.take_while(|(_, l)| l.trim_start().starts_with('|')) {
        match row(i + 1, &cells(l), &columns) {
            Ok(r) => table.rows.push(r),
            Err(e) => table.problems.push((i + 1, e)),
        }
    }
    assign_dirs(&mut table);
    Ok(table)
}

/// The index of the first line after the heading. A heading inside a fenced
/// code block is an example, not the section.
fn section_start(lines: &[&str]) -> Option<usize> {
    let mut fenced = false;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        } else if !fenced && l.trim_end() == HEADING {
            return Some(i + 1);
        }
    }
    None
}

fn columns(header: &[String]) -> Option<Columns> {
    let find = |f: &dyn Fn(&str) -> bool| header.iter().position(|h| f(&h.to_ascii_lowercase()));
    Some(Columns {
        width: header.len(),
        reference: find(&|h| h == "reference")?,
        commit: find(&|h| h == "commit")?,
        date: find(&|h| h == "date"),
        reason: find(&|h| h.starts_with("why")),
    })
}

fn row(line: usize, cells: &[String], cols: &Columns) -> Result<Row, String> {
    if cells.len() != cols.width {
        return Err(format!("the row has {} cells and the header has {}", cells.len(), cols.width));
    }
    let cell = |i: usize| cells.get(i).map_or("", String::as_str);
    let (repo, label) = split_code(cell(cols.reference));
    if !valid_repo(&repo) {
        return Err(format!("`{repo}` is not `owner/repo`"));
    }
    let (commit_code, commit_rest) = split_code(cell(cols.commit));
    let commit_cell = if commit_rest.is_empty() { commit_code } else { cell(cols.commit).replace('`', "") };
    let commit = is_commit(&commit_cell).then(|| commit_cell.to_ascii_lowercase());
    let text_of = |i: Option<usize>| i.map(|i| cell(i).replace('`', "")).unwrap_or_default();
    Ok(Row {
        line,
        dir: String::new(),
        repo,
        label,
        commit_cell,
        commit,
        date: text_of(cols.date),
        reason: text_of(cols.reason),
    })
}

/// The directory of each pin. The first pin of a repository gets
/// `owner__repo`. A later pin at another commit gets a suffix, so that two
/// pins of one repository never share one working tree.
fn assign_dirs(table: &mut Table) {
    let mut first: HashMap<String, String> = HashMap::new();
    let mut duplicates = Vec::new();
    for (index, r) in table.rows.iter_mut().enumerate() {
        let Some(sha) = &r.commit else { continue };
        let base = r.repo.replacen('/', "__", 1);
        let key = r.repo.to_ascii_lowercase();
        match first.get(&key) {
            None => {
                first.insert(key, sha.clone());
                r.dir = base;
            }
            Some(seen) if seen == sha => duplicates.push((index, r.line, r.repo.clone())),
            Some(_) => r.dir = format!("{base}@{}", &sha[..12]),
        }
    }
    for (index, line, repo) in duplicates.into_iter().rev() {
        table.rows.remove(index);
        table.problems.push((line, format!("`{repo}` is pinned twice at the same commit")));
    }
    table.problems.sort();
}

/// The cells of a table line, trimmed. `\|` is a pipe inside a cell.
pub(crate) fn cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = if t.ends_with("\\|") { t } else { t.strip_suffix('|').unwrap_or(t) };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// The text of the first code span of a cell, and the rest of the cell
/// outside it. A cell with no code span is all code and no rest.
fn split_code(cell: &str) -> (String, String) {
    let Some((before, after)) = cell.split_once('`') else {
        return (cell.trim().to_string(), String::new());
    };
    let Some((code, after)) = after.split_once('`') else {
        return (cell.trim().to_string(), String::new());
    };
    let rest = format!("{} {}", before.trim(), after.trim());
    (code.trim().to_string(), rest.trim().to_string())
}

/// `owner/repo` with the characters that GitHub allows in each part. The check
/// also keeps a pin from naming a path such as `..` under .refs/.
pub(crate) fn valid_repo(s: &str) -> bool {
    let Some((owner, name)) = s.split_once('/') else {
        return false;
    };
    let part = |p: &str| {
        !p.is_empty()
            && !p.starts_with(['.', '-'])
            && p.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    part(owner) && part(name)
}

/// A full commit id: 40 hex digits.
pub(crate) fn is_commit(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A commit id built at run time, so that no source file holds a long
    /// hex string that the secrets check reads.
    fn sha(seed: u32) -> String {
        (0..40).map(|i| char::from_digit((i * 7 + seed) % 16, 16).unwrap_or('0')).collect()
    }

    fn doc(rows: &[String]) -> String {
        let mut t = String::from(
            "# References\n\n```\n## Pins\n```\n\n## Pins\n\nThe table below.\n\n\
             | Reference | Commit | Date | Why it is pinned |\n| --- | --- | --- | --- |\n",
        );
        for r in rows {
            t.push_str(r);
            t.push('\n');
        }
        t.push_str("\n## Findings\n\n| Reference | Commit |\n| --- | --- |\n| `x/y` | `z` |\n");
        t
    }

    #[test]
    fn rows_parse_with_their_lines_and_directories() {
        let (a, b) = (sha(1), sha(2));
        let text = doc(&[
            format!("| `QaidVoid/kage` | `{a}` | 2026-10-12 | the CLI in the guest |"),
            format!("| `QaidVoid/kage` (old) | `{}` | 2026-09-15 | the older \\| plan |", b.to_ascii_uppercase()),
            "| `herdrdev/herdr` | release v0.9.0 (documentation index) | 2026-09-15 | docs |".to_string(),
        ]);
        let t = parse(&text).unwrap();
        assert!(t.problems.is_empty(), "{:?}", t.problems);
        assert_eq!(t.rows.len(), 3);
        let kage = &t.rows[0];
        assert_eq!(
            (kage.repo.as_str(), kage.commit.as_deref(), kage.dir.as_str()),
            ("QaidVoid/kage", Some(a.as_str()), "QaidVoid__kage")
        );
        assert_eq!(kage.line, 13);
        let old = &t.rows[1];
        assert_eq!(old.label, "(old)");
        assert_eq!(old.commit.as_deref(), Some(b.as_str()));
        assert_eq!(old.dir, format!("QaidVoid__kage@{}", &b[..12]));
        assert_eq!(old.reason, "the older | plan");
        let herdr = &t.rows[2];
        assert_eq!(herdr.commit, None);
        assert_eq!(herdr.commit_cell, "release v0.9.0 (documentation index)");
    }

    #[test]
    fn a_bad_row_is_a_problem_and_not_a_guess() {
        let a = sha(3);
        let text = doc(&[
            format!("| `../etc` | `{a}` | d | r |"),
            format!("| `a/b` | `{a}` | d |"),
            format!("| `c/d` | `{a}` | d | r |"),
            format!("| `c/d` | `{a}` | d | r |"),
        ]);
        let t = parse(&text).unwrap();
        assert_eq!(t.rows.len(), 1);
        let lines: Vec<usize> = t.problems.iter().map(|p| p.0).collect();
        assert_eq!(lines, [13, 14, 16]);
        assert!(t.problems[2].1.contains("twice"));
    }

    #[test]
    fn a_missing_section_or_header_is_an_error() {
        assert!(parse("# References\n\nno table\n").is_err());
        assert!(parse("## Pins\n\ntext\n\n## Next\n| a | b |\n").is_err());
        assert!(parse("## Pins\n\n| Name | Sha |\n| --- | --- |\n").is_err());
        assert!(parse("## Pins\n\n| Reference | Commit |\n| `a/b` | c |\n").is_err());
        let empty = parse("## Pins\n\n| Reference | Commit |\n| --- | :---: |\n").unwrap();
        assert!(empty.rows.is_empty() && empty.problems.is_empty());
    }

    #[test]
    fn cells_split_on_pipes_that_are_not_escaped() {
        assert_eq!(cells("| a | `b` | c \\| d |"), ["a", "`b`", "c | d"]);
        assert_eq!(cells("a|b"), ["a", "b"]);
        assert!(valid_repo("google/crosvm") && valid_repo("a.b/c_d-e"));
        assert!(!valid_repo("a/b/c") && !valid_repo("a") && !valid_repo("-a/b") && !valid_repo("a/.."));
        assert!(is_commit(&sha(5)) && !is_commit("release v0.9.0") && !is_commit(&sha(5)[..39]));
    }
}
