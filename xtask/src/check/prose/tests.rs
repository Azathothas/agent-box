//! Tests of the prose check, on Markdown and on Rust comments.

use super::*;

/// `n` words without a sentence end.
fn words(n: usize) -> String {
    vec!["word"; n].join(" ")
}

/// A sentence of `n` words that ends with a full stop.
fn sentence(n: usize) -> String {
    format!("{}.", words(n))
}

/// The line of each finding in a Markdown text.
fn lines(text: &str) -> Vec<usize> {
    markdown("a.md", text).iter().map(|f| f.line).collect()
}

fn none() -> Vec<usize> {
    Vec::new()
}

#[test]
fn a_sentence_of_26_words_fails() {
    let found = markdown("a.md", &format!("{}\n", sentence(26)));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 1);
    let start = "26 words in one sentence, the limit is 25: \"word word word word word word word word ...\"";
    assert_eq!(found[0].message, start);
}

#[test]
fn a_sentence_of_25_words_passes() {
    assert_eq!(lines(&format!("{}\n", sentence(25))), none());
}

#[test]
fn a_full_stop_ends_a_sentence_and_a_version_number_does_not() {
    assert_eq!(lines(&format!("{} {}\n", sentence(13), sentence(13))), none());
    assert_eq!(lines(&format!("{} {}\n", words(13), sentence(13))), [1]);
    // 26 words: the dots in 1.99.0 and in a.rs do not end the sentence.
    assert_eq!(lines(&format!("Rust 1.99.0 reads a.rs {}\n", sentence(22))), [1]);
}

#[test]
fn a_code_span_counts_as_one_word() {
    let span = "`cargo xtask check prose --json`";
    assert_eq!(lines(&format!("{} {span}.\n", words(24))), none());
    assert_eq!(lines(&format!("{} {span}.\n", words(25))), [1]);
}

#[test]
fn a_link_counts_its_text_only() {
    let link = "[two words](https://example.com/a/b/c/d/e \"a long title here\")";
    assert_eq!(lines(&format!("{} {link}.\n", words(23))), none());
    assert_eq!(lines(&format!("{} {link}.\n", words(24))), [1]);
}

#[test]
fn a_bare_url_counts_as_one_word() {
    let url = "https://example.com/a/b?c=d";
    assert_eq!(lines(&format!("{} {url}.\n", words(24))), none());
    assert_eq!(lines(&format!("{} {url}.\n", words(25))), [1]);
}

#[test]
fn paragraph_lines_join_until_a_blank_line() {
    assert_eq!(lines(&format!("{}\n{}\n", words(15), words(15))), [1]);
    assert_eq!(lines(&format!("{}\n\n{}\n", words(15), words(15))), none());
}

#[test]
fn a_list_item_ends_at_the_next_item_or_a_blank_line() {
    let half = words(15);
    let list = format!("- {half}\n* {half}\n+ {half}\n1. {half}\n2) {half}\n- [x] {half}\n");
    assert_eq!(lines(&list), none());
    assert_eq!(lines(&format!("- {half}\n  {half}\n")), [1]);
    assert_eq!(lines(&format!("- {half}\n\n  {half}\n")), none());
}

#[test]
fn a_bold_label_starts_a_new_unit() {
    assert_eq!(lines(&format!("**Source:** {}\n**Category:** {}\n", words(15), words(15))), none());
}

#[test]
fn a_fenced_block_is_skipped() {
    let body = format!("{}\nsimply\n", sentence(30));
    for fence in ["```", "~~~", "````"] {
        let text = format!("Before.\n\n{fence}sh\n{body}{fence}\n\nAfter.\n");
        assert_eq!(lines(&text), none(), "{fence}");
    }
    // A shorter fence does not close a longer one.
    assert_eq!(lines(&format!("````\n```\n{body}````\n")), none());
}

#[test]
fn an_indented_code_block_is_skipped_outside_a_list_only() {
    let long = sentence(30);
    assert_eq!(lines(&format!("Text.\n\n    {long}\n")), none());
    assert_eq!(lines(&format!("- Item.\n\n    {long}\n")), [3]);
}

#[test]
fn quotes_headings_comments_and_link_definitions_are_skipped() {
    let long = sentence(30);
    let text = format!(
        "> {long}\n\n# {long}\n\n{long}\n===\n\n<!-- {long} -->\n\n<!--\n{long}\n-->\n\n\
         [label]: https://example.com/ \"simply\"\n\n---\n"
    );
    assert_eq!(lines(&text), none());
}

#[test]
fn text_around_an_html_comment_is_read() {
    assert_eq!(lines(&format!("{} <!-- x --> {}\n", words(13), sentence(13))), [1]);
}

#[test]
fn a_comment_opener_in_a_code_span_is_text() {
    assert_eq!(lines(&format!("Write `<!--` here. {}\n", sentence(26))), [1]);
}

#[test]
fn each_table_cell_is_checked_on_its_own() {
    let long = sentence(26);
    let half = words(15);
    let table = format!("| A | B |\n| --- | :-: |\n| {long} | short |\n| {half} | {half} |\n");
    assert_eq!(lines(&table), [3]);
    // A table without the outer pipes.
    let table = format!("A | B\n--- | ---\n{half} | {half}\n{long} | x\n\nAfter.\n");
    assert_eq!(lines(&table), [4]);
}

#[test]
fn an_escaped_pipe_does_not_split_a_cell() {
    let half = words(15);
    assert_eq!(lines(&format!("| A |\n| --- |\n| {half} \\| {half} |\n")), [3]);
}

#[test]
fn each_banned_word_is_found_with_its_line() {
    for (term, _) in inline::BANNED {
        let found = markdown("a.md", &format!("First line.\nThe {term} case.\n"));
        assert_eq!(found.len(), 1, "{term}");
        assert_eq!(found[0].line, 2, "{term}");
        assert!(found[0].message.starts_with(&format!("\"{term}\" is not allowed: ")), "{}", found[0].message);
    }
}

#[test]
fn a_banned_word_in_a_cell_or_a_list_item_is_found() {
    assert_eq!(lines("| a | very |\n| - | - |\n"), [1]);
    assert_eq!(lines("Intro.\n\n- just one\n"), [3]);
}

#[test]
fn a_banned_word_in_a_rust_comment_is_found() {
    let found = rust_comments("a.rs", "fn f() {}\n// This is very small.\n");
    assert_eq!(found.iter().map(|f| f.line).collect::<Vec<_>>(), [2]);
}

#[test]
fn a_rust_string_a_code_span_and_a_doc_example_are_not_read() {
    let source = "let s = \"very\";\n/// Call `very` here.\n///\n/// ```\n/// let very = 1;\n/// ```\n/// After.\n";
    assert_eq!(rust_comments("a.rs", source), Vec::<Finding>::new());
}

#[test]
fn a_long_rust_comment_is_not_a_finding() {
    assert_eq!(rust_comments("a.rs", &format!("// {}\n", sentence(40))), Vec::<Finding>::new());
}

#[test]
fn the_comments_of_the_text_and_prose_checks_hold_no_banned_word() {
    let sources = [
        ("text.rs", include_str!("../text.rs")),
        ("prose.rs", include_str!("../prose.rs")),
        ("prose/inline.rs", include_str!("inline.rs")),
        ("prose/rust.rs", include_str!("rust.rs")),
        ("prose/tests.rs", include_str!("tests.rs")),
    ];
    for (rel, source) in sources {
        assert_eq!(rust_comments(rel, source), Vec::<Finding>::new(), "{rel}");
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
    assert!(!report.notes.iter().any(|n| n.contains("below the floor")), "{:?}", report.notes);
}
