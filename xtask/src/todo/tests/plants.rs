//! The reader against planted defects. Each test plants one disagreement
//! into a record that agrees with itself, and the reader must name it. The
//! first test is the control: the record as written has no problem.

use super::{AREA, Tree};
use crate::repo::Repo;
use crate::report::Status;

fn assert_found(t: &Tree, what: &str) {
    let p = t.problems();
    assert!(p.iter().any(|x| x.contains(what)), "no problem mentions {what:?}; the reader found: {p:#?}");
}

#[test]
fn the_control_record_agrees() {
    let t = Tree::new("control");
    assert_eq!(t.problems(), Vec::<String>::new());
}

#[test]
fn a_wrong_counts_line_is_found() {
    let t = Tree::new("counts-line");
    t.plant("TODO/INDEX.md", "**3 entries: 1 open", "**3 entries: 2 open");
    assert_found(&t, "TODO/INDEX.md:3: the counts line says 3 entries");
}

#[test]
fn a_wrong_priority_row_is_found() {
    let t = Tree::new("priority-row");
    t.plant("TODO/INDEX.md", "| P1 | 1 | 0 | 0 | 1 | 2 |", "| P1 | 0 | 0 | 0 | 1 | 1 |");
    assert_found(&t, "the P1 row of the counts table");
}

#[test]
fn a_wrong_all_row_is_found() {
    let t = Tree::new("all-row");
    t.plant("TODO/INDEX.md", "| **All** | 1 | 0 | 1 | 1 | 3 |", "| **All** | 7 | 0 | 1 | 1 | 9 |");
    assert_found(&t, "the All row of the counts table");
}

#[test]
fn a_wrong_progress_count_is_found() {
    let t = Tree::new("progress-count");
    t.plant("TODO/PROGRESS.md", "holds 3 entries: 1 open", "holds 3 entries: 0 open");
    assert_found(&t, "TODO/PROGRESS.md:3: the counts line");
}

#[test]
fn a_status_that_differs_from_the_row_is_found() {
    let t = Tree::new("status");
    t.plant("TODO/area.md", "**Status:** open", "**Status:** partial");
    assert_found(&t, "T-001: the entry says Status partial, the index says open");
}

#[test]
fn a_title_that_differs_from_the_row_is_found() {
    let t = Tree::new("title");
    t.plant("TODO/area.md", "# T-001: The first entry", "# T-001: The first entry, renamed");
    assert_found(&t, "T-001: the title differs");
}

#[test]
fn an_entry_with_no_row_is_found() {
    let t = Tree::new("no-row");
    let extra = AREA.split("# T-003").nth(1).unwrap().replace(": The third entry", ": A fourth");
    t.write("TODO/more.md", &format!("More.\n\n# T-004{extra}"));
    assert_found(&t, "T-004 has no row in the index");
}

#[test]
fn a_row_with_no_entry_is_found() {
    let t = Tree::new("no-entry");
    let third = "| [T-003](area.md) | P1 | S | M3 | defect | done | The third entry |";
    t.plant("TODO/INDEX.md", third, &format!("{third}\n| [T-005](area.md) | P1 | S | M3 | defect | done | Gone |"));
    assert_found(&t, "T-005 has a row and no entry");
}

#[test]
fn an_id_used_twice_is_found() {
    let t = Tree::new("twice");
    let third = AREA.split("# T-003").nth(1).unwrap();
    t.write("TODO/other.md", &format!("Other.\n\n# T-003{third}"));
    assert_found(&t, "T-003 is also an entry");
}

#[test]
fn rows_out_of_order_are_found() {
    let t = Tree::new("order");
    let r1 = "| [T-001](area.md) | P1 | S | M3 | defect | open | The first entry |";
    let r2 = "| [T-002](area.md) | P2 | M | M4 | feature | blocked | The second entry |";
    t.plant("TODO/INDEX.md", &format!("{r1}\n{r2}"), &format!("{r2}\n{r1}"));
    assert_found(&t, "T-001 is out of order");
}

#[test]
fn an_effort_of_xl_is_refused() {
    let t = Tree::new("xl");
    t.plant("TODO/INDEX.md", "| P2 | M | M4 | feature | blocked |", "| P2 | XL | M4 | feature | blocked |");
    t.plant("TODO/area.md", "**Effort:** M", "**Effort:** XL");
    assert_found(&t, "the effort `XL` is not one of S, M, L");
}

#[test]
fn a_milestone_outside_the_list_is_refused() {
    let t = Tree::new("milestone");
    t.plant("TODO/INDEX.md", "| P2 | M | M4 | feature |", "| P2 | M | none | feature |");
    t.plant("TODO/area.md", "**Milestone:** M4", "**Milestone:** none");
    assert_found(&t, "the milestone `none` is not one of M0, M1");
}

#[test]
fn a_missing_section_is_found() {
    let t = Tree::new("no-premise");
    t.plant("TODO/area.md", "## Premise\n\nRead: `crates/x/src/lib.rs:3`.\n\n", "");
    assert_found(&t, "T-001: the section `## Premise` is missing");
}

#[test]
fn an_unknown_section_is_found() {
    let t = Tree::new("tasks");
    t.plant("TODO/area.md", "## Approach\n\nBound the wait.", "## Tasks\n\nBound the wait.");
    assert_found(&t, "`## Tasks` is not a section of an entry");
}

#[test]
fn a_prove_with_no_command_is_found() {
    let t = Tree::new("prove");
    t.plant("TODO/area.md", "Run `cargo test -p x -- second`.", "Run the tests.");
    assert_found(&t, "T-002: a Prove with no command");
}

#[test]
fn a_done_entry_needs_dated_evidence() {
    let t = Tree::new("done-date");
    t.plant("TODO/area.md", "2026-10-08, commit", "Today, commit");
    assert_found(&t, "T-003: `## Done` gives no date");
}

#[test]
fn a_blocked_entry_needs_its_blocker() {
    let t = Tree::new("blocker");
    t.plant("TODO/area.md", "## Blocker\n\nThe ruling of the operator on question Q1 in `TODO/PROGRESS.md`.\n", "");
    assert_found(&t, "T-002: blocked, with no `## Blocker`");
}

#[test]
fn a_blocker_names_a_question_that_was_asked() {
    let t = Tree::new("question");
    t.plant("TODO/area.md", "question Q1 in", "question Q9 in");
    assert_found(&t, "the blocker names Q9");
}

#[test]
fn the_work_order_names_no_done_entry() {
    let t = Tree::new("work-order");
    t.plant("TODO/PROGRESS.md", "1. T-001.", "1. T-003.");
    assert_found(&t, "the work order names T-003, which is done");
}

#[test]
fn a_reference_to_no_entry_is_found() {
    let t = Tree::new("dangling");
    t.plant("README.md", "(T-001 first)", "(T-099 first)");
    assert_found(&t, "README.md:3: T-099 is not an entry of the index");
}

#[test]
fn ids_and_paths_in_a_fence_are_not_read() {
    let t = Tree::new("fenced");
    t.write("docs/research/old.md", "# Old\n\n```\nold T-099 -> T-001, see `crates/x/src/nope.rs:9`\n```\n");
    assert_eq!(t.problems(), Vec::<String>::new());
}

#[test]
fn a_citation_in_docs_is_read() {
    let t = Tree::new("docs-citation");
    t.write("docs/research/notes.md", "# Notes\n\nRead `crates/x/src/nope.rs` and `crates/x/src/lib.rs:9`.\n");
    assert_found(&t, "docs/research/notes.md:3: `crates/x/src/nope.rs` does not exist");
    assert_found(&t, "docs/research/notes.md:3: `crates/x/src/lib.rs:9`: crates/x/src/lib.rs has 5 lines");
}

#[test]
fn a_citation_of_another_project_is_not_read() {
    let t = Tree::new("external");
    let text = "# Notes\n\nRead `QaidVoid/kage@f8df989:crates/nope.rs:99`, `lablup/bssh:crates/nope.rs`, line 3.\n";
    t.write("docs/notes.md", text);
    assert_eq!(t.problems(), Vec::<String>::new());
}

#[test]
fn a_cited_path_must_exist() {
    let t = Tree::new("path");
    t.plant("TODO/area.md", "Read: `crates/x/src/lib.rs:3`.", "Read: `crates/x/src/nope.rs`.");
    assert_found(&t, "`crates/x/src/nope.rs` does not exist");
}

#[test]
fn a_cited_line_must_exist() {
    let t = Tree::new("line");
    t.plant("TODO/area.md", "`crates/x/src/lib.rs:2-4`", "`crates/x/src/lib.rs:2-40`");
    assert_found(&t, "crates/x/src/lib.rs has 5 lines");
}

#[test]
fn a_quote_that_the_cited_line_does_not_hold_is_found() {
    let t = Tree::new("quote");
    t.plant("TODO/area.md", "says \"line 4\"", "says \"line 9\"");
    assert_found(&t, "`crates/x/src/lib.rs:4` says \"line 9\", but the cited lines do not hold that text");
}

#[test]
fn a_quote_over_two_lines_of_a_range_holds() {
    let t = Tree::new("quote-wrapped");
    t.plant(
        "TODO/area.md",
        "It fails; `crates/x/src/lib.rs:4` says \"line 4\".",
        "It fails; `crates/x/src/lib.rs:3-4` says \"line 3\nline 4\".",
    );
    assert_eq!(t.problems(), Vec::<String>::new());
    t.plant("TODO/area.md", "says \"line 3\nline 4\"", "says \"line 3\nline 5\"");
    assert_found(&t, "says \"line 3 line 5\", but the cited lines do not hold that text");
}

#[test]
fn a_cited_path_must_have_its_exact_case() {
    let t = Tree::new("case");
    t.plant("TODO/area.md", "Read: `crates/x/src/lib.rs:3`.", "Read: `crates/X/src/lib.rs:3`.");
    assert_found(&t, "`crates/X/src/lib.rs` does not exist");
}

#[test]
fn the_roadmap_keeps_no_open_items() {
    let t = Tree::new("roadmap-open");
    t.plant("docs/roadmap.md", "- T-002.", "- [ ] Something to do.");
    assert_found(&t, "docs/roadmap.md:10: an open item");
}

#[test]
fn the_roadmap_agrees_with_the_milestones() {
    let t = Tree::new("roadmap-milestone");
    t.plant("docs/roadmap.md", "- T-001: the first entry.", "- T-002: in the wrong milestone.");
    assert_found(&t, "T-002 is listed under M3, but its milestone is M4");
    // A done milestone can name the later entry that finished it.
    t.plant("docs/roadmap.md", "## M3: Beta", "## M3: Beta (done 2026-10-14)");
    assert_eq!(t.problems(), Vec::<String>::new());
}

#[test]
fn an_h1_that_is_not_an_entry_is_found() {
    let t = Tree::new("h1");
    // One H1 before the first entry is the title of the file.
    t.plant("TODO/area.md", "The entries of one area.", "# One area\n\nThe entries of one area.");
    assert_eq!(t.problems(), Vec::<String>::new());
    t.plant("TODO/area.md", "# T-002: The second entry", "# Notes\n\n# T-002: The second entry");
    assert_found(&t, "an H1 that is not an entry");
}

#[test]
fn an_unknown_field_is_found() {
    let t = Tree::new("field");
    t.plant("TODO/area.md", "**Effort:** S\n**Status:** open", "**Effort:** S\n**Owner:** someone\n**Status:** open");
    assert_found(&t, "`Owner` is not a field of an entry");
}

/// A reader that finds nothing to check must fail, not pass.
#[test]
fn an_index_with_no_rows_is_found() {
    let t = Tree::new("no-rows");
    let index = t.read("TODO/INDEX.md");
    let kept: Vec<&str> = index.lines().filter(|l| !l.starts_with("| [T-")).collect();
    t.write("TODO/INDEX.md", &(kept.join("\n") + "\n"));
    assert_found(&t, "TODO/INDEX.md: the index has no rows");
}

#[test]
fn a_missing_roadmap_is_found() {
    let t = Tree::new("no-roadmap");
    std::fs::remove_file(t.path().join("docs/roadmap.md")).unwrap();
    assert_found(&t, "docs/roadmap.md: the roadmap is missing");
}

#[test]
fn a_blocker_on_an_open_entry_is_found() {
    let t = Tree::new("stale-blocker");
    t.plant("TODO/area.md", "**Status:** blocked", "**Status:** open");
    t.plant("TODO/INDEX.md", "| feature | blocked |", "| feature | open |");
    t.plant("TODO/INDEX.md", "**3 entries: 1 open, 0 partial, 1 blocked", "**3 entries: 2 open, 0 partial, 0 blocked");
    assert_found(&t, "a `## Blocker` on an entry that is open");
}

#[test]
fn a_range_that_ends_before_it_starts_is_found() {
    // A remap that moved only the start of a range can leave `155-154`.
    let t = Tree::new("range-backwards");
    t.plant("TODO/area.md", "`crates/x/src/lib.rs:2-4`", "`crates/x/src/lib.rs:4-2`");
    assert_found(&t, "`crates/x/src/lib.rs:4-2`: a range runs from line 1 or later");
}

#[test]
fn a_line_0_is_found() {
    let t = Tree::new("line-zero");
    t.plant("TODO/area.md", "Read: `crates/x/src/lib.rs:3`.", "Read: `crates/x/src/lib.rs:0`.");
    assert_found(&t, "`crates/x/src/lib.rs:0`: a range runs from line 1 or later");
}

#[test]
fn a_plain_line_number_after_a_citation_is_found() {
    // remap moves neither "line 5" nor the other numbers of its file.
    let t = Tree::new("plain-number");
    t.plant("TODO/area.md", "Read: `crates/x/src/lib.rs:3`.", "Read: `crates/x/src/lib.rs:3`, and line 5.");
    assert_found(&t, "\"line 5\" follows a citation as plain text");
}

#[test]
fn a_bare_number_names_a_line_of_its_file() {
    let t = Tree::new("bare-number");
    t.plant("TODO/area.md", "Read: `crates/x/src/lib.rs:3`.", "Read: `crates/x/src/lib.rs:3` and `:9`.");
    assert_found(&t, "`:9` after `crates/x/src/lib.rs`: crates/x/src/lib.rs has 5 lines");
}

#[test]
fn a_reading_at_a_commit_and_a_bare_number_agree() {
    let t = Tree::new("number-forms");
    t.plant(
        "TODO/area.md",
        "Read: `crates/x/src/lib.rs:3`.",
        "Read: `crates/x/src/lib.rs:3`, line 9 at `0123abc`, and `:4`.",
    );
    assert_eq!(t.problems(), Vec::<String>::new());
}

/// The check of the gate. Each problem is a finding with its file and line, and
/// the scope is the number of entries. No record, or no entry, is not a pass.
#[test]
fn the_gate_check_maps_problems_to_findings() {
    let t = Tree::new("gate");
    let repo = Repo { root: t.path().to_path_buf() };
    let report = super::super::check(&repo);
    assert_eq!((report.status, report.scanned), (Status::Pass, 3), "{report:#?}");

    t.plant("TODO/area.md", "**Status:** open", "**Status:** partial");
    let report = super::super::check(&repo);
    assert_eq!(report.status, Status::Fail, "{report:#?}");
    assert!(
        report.findings.iter().any(|f| f.path == "TODO/area.md" && f.line == 3 && f.message.contains("Status partial")),
        "{report:#?}"
    );

    t.write("TODO/area.md", "The entries of one area.\n");
    let report = super::super::check(&repo);
    assert_eq!((report.status, report.scanned), (Status::NotRun, 0), "{report:#?}");

    std::fs::remove_file(t.path().join("TODO/INDEX.md")).unwrap();
    let report = super::super::check(&repo);
    assert_eq!(report.status, Status::NotRun, "{report:#?}");
    assert!(report.notes.iter().any(|n| n.contains("TODO/INDEX.md is missing")), "{report:#?}");
}
