//! Tests of the docs check. Most tests give the documents as text in memory.
//! One test writes a small tree to a temporary directory and reads it back.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::{Scope, check_docs, link_targets, percent_decode, resolve, run};
use crate::repo::{Repo, is_markdown};
use crate::report::{Finding, Status};

/// Apply the rules to a tree of (path, text) pairs. Each pair is a visible file.
fn findings(tree: &[(&str, &str)]) -> Vec<Finding> {
    let docs: BTreeMap<String, String> = tree
        .iter()
        .filter(|(path, _)| is_markdown(path))
        .map(|(path, text)| ((*path).to_string(), (*text).to_string()))
        .collect();
    let files: BTreeSet<String> = tree.iter().map(|(path, _)| (*path).to_string()).collect();
    let lines = |rel: &str| tree.iter().find(|(path, _)| *path == rel).map(|(_, text)| text.lines().count());
    check_docs(&Scope { docs: &docs, files: &files, lines: &lines }).findings
}

fn locations(found: &[Finding]) -> Vec<String> {
    found.iter().map(Finding::location).collect()
}

#[test]
fn a_broken_link_is_a_finding_at_its_line() {
    let found = findings(&[("README.md", "# Title\n\nSee [a](a.md) and [b](b.md).\n"), ("a.md", "# A\n")]);
    assert_eq!(locations(&found), ["README.md:3"]);
    assert!(found[0].message.contains("`b.md`"), "{}", found[0].message);
}

#[test]
fn links_resolve_from_the_directory_of_the_linking_file() {
    let tree = [
        ("README.md", "[a](docs/a.md) [b](/docs/b.md) [crates](crates) [up](../x.md)\n"),
        ("docs/a.md", "[b](b.md) [root](../README.md) [dir](../crates/one/) [self](./a.md) [here](.)\n"),
        ("docs/b.md", "[a](./a.md?plain=1) [far](../../x.md)\n"),
        ("crates/one/Cargo.toml", ""),
    ];
    let found = findings(&tree);
    assert_eq!(locations(&found), ["README.md:1", "docs/b.md:1"]);
    assert!(found.iter().all(|f| f.message.contains("above the root")));
}

#[test]
fn a_link_to_a_directory_marks_no_file_in_it() {
    let found = findings(&[("README.md", "[docs](docs/) [todo](TODO)\n"), ("docs/a.md", "# A\n"), ("TODO/b.md", "")]);
    assert_eq!(locations(&found), ["TODO/b.md", "docs/a.md"]);
    assert!(found[0].message.contains("no other Markdown file links to this page"));
}

#[test]
fn roots_need_no_link_and_a_link_to_itself_does_not_count() {
    let tree = [
        ("README.md", ""),
        ("docs/README.md", "[me](README.md)\n"),
        ("docs/b.md", "# B\n\n[me](b.md) [top](#b) [c](../TODO/c.md)\n"),
        ("TODO/c.md", ""),
    ];
    assert_eq!(locations(&findings(&tree)), ["docs/b.md"]);
}

#[test]
fn an_anchor_must_be_a_heading_slug_of_the_target() {
    let tree = [
        (
            "README.md",
            "[ok](a.md#the-box_core-crate) [two](a.md#setup-1) [bad](a.md#nothing) [case](a.md#Setup)\n\
             [own](#intro) [rs](x.rs#L3) [space](a.md#the%2Dbox_core-crate)\n\n## Intro\n",
        ),
        ("a.md", "# The `box_core` crate\n\n## Setup\n\ntext\n\n## Setup\n"),
        ("x.rs", "fn main() {}\n"),
    ];
    let found = findings(&tree);
    assert_eq!(locations(&found), ["README.md:1", "README.md:1"]);
    assert!(found[0].message.contains("`#nothing`"));
    assert!(found[1].message.contains("the slug is `#setup`"), "{}", found[1].message);
}

#[test]
fn link_definitions_are_links_and_footnotes_are_not() {
    let tree = [
        ("README.md", "Text [a][x] and [b][y].\n\n[x]: missing.md\n[y]: <a.md> \"Title\"\n[^1]: a note, not a link\n"),
        ("a.md", ""),
    ];
    let found = findings(&tree);
    assert_eq!(locations(&found), ["README.md:3"]);
    assert!(found[0].message.contains("`missing.md`"));
}

#[test]
fn links_in_code_are_not_links() {
    let text = "```md\n[a](gone1.md)\n```\n\nInline `[b](gone2.md)` and ``[c](gone3.md)``.\n\
                A span `that\nruns [d](gone4.md) over` two lines.\n~~~\n[e](gone5.md)\n~~~\n[f](gone6.md)\n";
    let found = findings(&[("README.md", text)]);
    assert_eq!(locations(&found), ["README.md:11"]);
    assert!(found[0].message.contains("gone6.md"));
}

#[test]
fn web_links_are_not_followed_and_other_schemes_are_findings() {
    let text = "[w](https://example.org/x) [m](mailto:team@example.org) [u](HTTPS://EXAMPLE.ORG) [f](file:///x/y)\n";
    let found = findings(&[("README.md", text)]);
    assert_eq!(locations(&found), ["README.md:1"]);
    assert!(found[0].message.contains("`file:`"));
}

#[test]
fn letter_case_must_match_and_the_finding_names_the_file() {
    let found = findings(&[("README.md", "[g](Guide.md)\n"), ("guide.md", "")]);
    assert_eq!(locations(&found), ["README.md:1"]);
    assert!(found[0].message.contains("`guide.md` has a different letter case"), "{}", found[0].message);
}

#[test]
fn escapes_angle_brackets_titles_and_parentheses() {
    let tree = [
        ("README.md", "[a](my%20notes.md) [b](<my notes.md>) [c](notes(1).md \"Title\") [d]() [e](\n"),
        ("my notes.md", ""),
        ("notes(1).md", ""),
    ];
    let found = findings(&tree);
    assert_eq!(locations(&found), ["README.md:1"]);
    assert!(found[0].message.contains("empty target"));
}

#[test]
fn paths_in_inline_code_must_exist() {
    let text = "See `xtask/src/main.rs:3`, `xtask/src/main.rs:9`, `xtask/src/gone.rs`, `xtask/` and `xtask/src/*.rs`.\n\
                \n> Quoted: `xtask/src/quoted.rs`.\n\n```\n`xtask/src/fenced.rs`\n```\n\
                Other: `elsewhere/file.rs` and `../above.md`.\n";
    let found = findings(&[("README.md", text), ("xtask/src/main.rs", "a\nb\nc\nd\n")]);
    assert_eq!(locations(&found), ["README.md:1", "README.md:1"]);
    assert!(found[0].message.contains("line 9"));
    assert!(found[1].message.contains("`xtask/src/gone.rs`"));
}

#[test]
fn helpers_read_targets_paths_and_escapes() {
    assert_eq!(link_targets("[a](b.md) ![i](c.png \"t\") [d](<e f.md>)"), ["b.md", "c.png", "e f.md"]);
    assert_eq!(link_targets("[label]: target.md \"Title\""), ["target.md"]);
    assert_eq!(resolve("docs/a.md", "../b.md"), Some("b.md".to_string()));
    assert_eq!(resolve("docs/a.md", "/c/d.md"), Some("c/d.md".to_string()));
    assert_eq!(resolve("a.md", "../x.md"), None);
    assert_eq!(percent_decode("a%20b%2"), "a b%2");
}

/// A temporary directory that the test removes at the end.
struct TempTree(PathBuf);

impl TempTree {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("xtask-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create the temporary directory");
        Self(dir)
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.0.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create a directory");
        }
        std::fs::write(path, text).expect("write a file");
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_tree_on_disk_resolves_each_link_from_its_own_file() {
    let tree = TempTree::new("docs-check");
    let mut files: Vec<String> = Vec::new();
    let mut add = |rel: &str, text: &str| {
        tree.write(rel, text);
        files.push(rel.to_string());
    };
    let pages: Vec<String> = (1..=7).map(|n| format!("docs/page{n}.md")).collect();
    let index: String = pages.iter().flat_map(|page| ["- [", page, "](", page, ")\n"]).collect();
    add("README.md", &format!("# Project\n\n{index}- [Rules](TODO/RULES.md#the-rules)\n"));
    for page in &pages {
        add(page, "# Page\n\nBack to the [index](../README.md#project). See `xtask/src/main.rs:2`.\n");
    }
    add("TODO/RULES.md", "# The rules\n\nThe [first page](../docs/page1.md#page) and [the code](../xtask/).\n");
    add("SECURITY.md", "# Security\n");
    add("xtask/src/main.rs", "fn main() {}\n// two lines\n");
    let repo = Repo { root: tree.0.clone() };

    let good = run(&repo, files.clone());
    assert_eq!(good.status, Status::Pass, "{:?}", good.findings);
    assert_eq!(good.scanned, 10);

    // Below the floor the check read too little to say anything.
    let few = run(&repo, files[..3].to_vec());
    assert_eq!(few.status, Status::NotRun);

    tree.write("docs/stray.md", "[gone](../missing.md)\n");
    files.push("docs/stray.md".to_string());
    let bad = run(&repo, files);
    assert_eq!(bad.status, Status::Fail);
    assert_eq!(locations(&bad.findings), ["docs/stray.md", "docs/stray.md:1"]);
}
