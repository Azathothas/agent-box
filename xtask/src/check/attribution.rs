//! Check `attribution`: does a commit of this history credit a tool?
//!
//! The defect this check exists to catch: a commit message that names the
//! tool that wrote it. Several agent tools tell the model to add a co-author
//! trailer. So the credit comes from a default setting, not from a decision,
//! and it comes on each commit of a session. The rule: the operator is the
//! only identity in the history.
//!
//! The check reads `git log`, so it finds a credit after the commit exists.
//! The hook `.githooks/commit-msg` refuses the message before the commit
//! exists. So the check also examines the hook: the file must exist, and its
//! index mode must be 100755. git does not run a hook without the executable
//! bit, and it gives no message about it.
//!
//! A finding names a commit by its short hash and names the rule. It never
//! prints the message.
//!
//! NOTE: This file does not hold the names, the trailer key or the emoji as
//! text. It joins each one from two halves or writes it as an escape, and the
//! tests build each planted message at run time. Other checks read the text
//! of this file, and it must not trip them.

use crate::proc;
use crate::repo::Repo;
use crate::report::{CheckReport, Finding, Status};

pub(crate) const DEFECT: &str = "a commit message that credits a tool, or a commit-msg hook that git does not run";

const NAME: &str = "attribution";

/// Zero commits read is not a pass: the check agreed with nothing.
const FLOOR: usize = 1;

/// The hook that refuses a credit before the commit exists, and the index mode that git runs.
const HOOK: &str = ".githooks/commit-msg";
const EXECUTABLE: &str = "100755";

/// The format of one record: the full hash, a NUL, the raw message, and the
/// byte 0x01. A message holds newlines, so a newline cannot end a record.
const LOG_FORMAT: &str = "--format=%H%x00%B%x01";
const RECORD_END: char = '\u{1}';

/// The number of hex digits of the short hash in a finding.
const SHORT: usize = 12;

/// The tool and vendor names, matched as whole words in any case.
const NAMES: [&str; 10] = [
    concat!("cla", "ude"),
    concat!("anthro", "pic"),
    concat!("copi", "lot"),
    concat!("chat", "gpt"),
    concat!("open", "ai"),
    concat!("gem", "ini"),
    concat!("cur", "sor"),
    concat!("dev", "in"),
    concat!("cod", "ex"),
    concat!("ai", "der"),
];

/// The key of a co-author trailer, in lowercase.
const TRAILER: &str = concat!("co-auth", "ored-by");

/// The robot face emoji, U+1F916.
const ROBOT: char = '\u{1F916}';

/// The lines that a tool adds to a message. Each phrase starts a word and ends a word.
const GENERATED: [&str; 2] = [concat!("gener", "ated with"), concat!("gener", "ated by")];

/// The parts of the created-with phrase and its forms: a verb, an article, a maker.
const CREATED: [&str; 2] = ["created with", "created by"];
const ARTICLES: [&str; 2] = ["a", "an"];
const MAKERS: [&str; 3] = ["ai", "assistant", "agent"];

pub(crate) fn check(repo: &Repo) -> CheckReport {
    let log = match git(repo, &["log", "--no-show-signature", LOG_FORMAT]) {
        Ok(out) => out,
        Err(e) => return CheckReport::not_run(NAME, format!("cannot read the history: {e}")),
    };
    let (commits, broken) = parse_log(&log);
    let mut findings = Vec::new();
    if broken > 0 {
        let message = format!("{broken} records did not split into a hash and a message, so they were not read");
        findings.push(Finding::new("git log", 0, message));
    }
    for commit in &commits {
        let short = commit.hash.get(..SHORT).unwrap_or(commit.hash);
        for (line, rule) in scan_message(commit.body) {
            findings.push(Finding::new(format!("commit {short}"), line, rule));
        }
    }
    let hook_error = match hook_findings(repo) {
        Ok(found) => {
            findings.extend(found);
            None
        }
        Err(e) => Some(e),
    };
    let mut report = CheckReport::from_findings(NAME, commits.len(), FLOOR, findings);
    if let Some(e) = hook_error {
        report = report.note(format!("could not read the index entry of {HOOK}: {e}"));
        if report.status == Status::Pass {
            report.status = Status::NotRun;
        }
    }
    for note in notes(repo) {
        report = report.note(note);
    }
    report
}

/// Run git in the repository root and return its output, or why it failed.
fn git(repo: &Repo, args: &[&str]) -> Result<String, String> {
    let out = proc::run_in(&repo.root, "git", args, proc::QUICK)?;
    if out.timed_out {
        return Err(format!("git {} did not stop within the time limit", args.join(" ")));
    }
    if !out.success() {
        return Err(format!("git {} failed: {}", args.join(" "), out.stderr.trim()));
    }
    Ok(out.stdout)
}

/// One commit of the history.
#[derive(Debug, PartialEq, Eq)]
struct Commit<'a> {
    hash: &'a str,
    body: &'a str,
}

/// The commits in the output of `git log` with [`LOG_FORMAT`], and the number
/// of records that did not split into a hash and a message.
fn parse_log(out: &str) -> (Vec<Commit<'_>>, usize) {
    let mut commits = Vec::new();
    let mut broken = 0;
    for record in out.split(RECORD_END) {
        // git puts a newline after each record, so the next record starts with it.
        let record = record.trim_start_matches(['\n', '\r']);
        if record.is_empty() {
            continue;
        }
        match record.split_once('\0') {
            Some((hash, body)) if is_object_id(hash) => commits.push(Commit { hash, body }),
            _ => broken += 1,
        }
    }
    (commits, broken)
}

/// True for a full object id: 40 hex digits (SHA-1) or 64 (SHA-256).
fn is_object_id(hash: &str) -> bool {
    matches!(hash.len(), 40 | 64) && hash.bytes().all(|c| c.is_ascii_hexdigit())
}

/// Each rule that a message breaks, with the line (from 1) where it breaks it.
fn scan_message(body: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (index, line) in body.lines().enumerate() {
        let number = index + 1;
        let lower = line.to_lowercase();
        if is_trailer(&lower) {
            out.push((number, "a co-author trailer".to_string()));
        }
        if is_generated(&lower) {
            out.push((number, "a generated-with line".to_string()));
        }
        if line.contains(ROBOT) {
            out.push((number, "the robot face emoji (U+1F916)".to_string()));
        }
        for name in NAMES.iter().filter(|n| has_phrase(&lower, n, true)) {
            out.push((number, format!("a tool or vendor name: {name}")));
        }
    }
    out
}

/// True for a co-author trailer: optional space, the key, optional space, and a colon.
fn is_trailer(lower: &str) -> bool {
    lower.trim_start().strip_prefix(TRAILER).is_some_and(|rest| rest.trim_start().starts_with(':'))
}

/// True for a line that says that a tool made the commit.
fn is_generated(lower: &str) -> bool {
    let words = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    if GENERATED.iter().any(|phrase| has_phrase(&words, phrase, true)) {
        return true;
    }
    for verb in CREATED {
        for article in ARTICLES {
            for maker in MAKERS {
                // "ai" must end a word, or "aim" matches. "agent" can go on, as in "agents".
                if has_phrase(&words, &format!("{verb} {article} {maker}"), maker == "ai") {
                    return true;
                }
            }
        }
    }
    false
}

/// True when `phrase` is in `text` and starts a word. With `whole`, it must also end a word.
fn has_phrase(text: &str, phrase: &str, whole: bool) -> bool {
    let bytes = text.as_bytes();
    let joined = |c: &u8| c.is_ascii_alphanumeric() || *c == b'_';
    text.match_indices(phrase).any(|(at, _)| {
        let starts = !at.checked_sub(1).and_then(|i| bytes.get(i)).is_some_and(joined);
        let ends = !whole || !bytes.get(at + phrase.len()).is_some_and(joined);
        starts && ends
    })
}

/// The problems of the hook: the file is missing, or git does not run it.
fn hook_findings(repo: &Repo) -> Result<Vec<Finding>, String> {
    let mut out = Vec::new();
    if !repo.path(HOOK).is_file() {
        out.push(Finding::new(
            HOOK,
            0,
            "the commit-msg hook is missing, so nothing refuses a credit before the commit",
        ));
    }
    let entry = git(repo, &["ls-files", "-s", "--", HOOK])?;
    match index_mode(&entry) {
        None => out.push(Finding::new(HOOK, 0, "the hook is not in the index, so a clone does not get it")),
        Some(EXECUTABLE) => {}
        Some(mode) => out.push(Finding::new(
            HOOK,
            0,
            format!(
                "the index mode is {mode}, not {EXECUTABLE}, so git does not run the hook; \
                 fix it with `git update-index --chmod=+x {HOOK}`"
            ),
        )),
    }
    Ok(out)
}

/// The mode in the first line of `git ls-files -s`, or `None` when git has no entry.
fn index_mode(entry: &str) -> Option<&str> {
    entry.lines().next()?.split_whitespace().next()
}

/// Facts that are not findings: a shallow clone, and a checkout that does not run the hook.
fn notes(repo: &Repo) -> Vec<String> {
    let mut out = Vec::new();
    if git(repo, &["rev-parse", "--is-shallow-repository"]).is_ok_and(|s| s.trim() == "true") {
        out.push("this clone is shallow: the check read the fetched commits only, not the whole history".to_string());
    }
    // A clone does not copy the hook setting, so this is a note and not a finding.
    if !git(repo, &["config", "--get", "core.hooksPath"]).is_ok_and(|p| is_hooks_dir(p.trim())) {
        out.push(
            "this checkout does not run the hook: core.hooksPath is not .githooks (`cargo xtask setup` sets it)".into(),
        );
    }
    out
}

/// True when a `core.hooksPath` value names the `.githooks` directory.
fn is_hooks_dir(value: &str) -> bool {
    let value = value.trim_end_matches(['/', '\\']);
    let value = value.strip_prefix("./").unwrap_or(value);
    value == ".githooks" || value.ends_with("/.githooks") || value.ends_with("\\.githooks")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No rule broken.
    const NONE: [&str; 0] = [];

    fn rules(body: &str) -> Vec<String> {
        scan_message(body).into_iter().map(|(_, rule)| rule).collect()
    }

    #[test]
    fn a_clean_message_breaks_no_rule() {
        assert_eq!(rules("Set up the repository, mine the references, and file the plan\n\nThe body.\n"), NONE);
    }

    #[test]
    fn finds_a_co_author_trailer_in_any_case() {
        let key = ["Co-Auth", "ored-By"].concat();
        let lines = [
            format!("{key}: A Person <a@b.c>"),
            format!("  {}  : x", key.to_lowercase()),
            format!("{}:x", key.to_uppercase()),
        ];
        for line in lines {
            let body = format!("Subject\n\n{line}\n");
            assert_eq!(scan_message(&body), [(3, "a co-author trailer".to_string())], "{line}");
        }
    }

    #[test]
    fn the_trailer_key_in_prose_is_not_a_trailer() {
        assert_eq!(rules(&format!("Explain why a {TRAILER} line is refused\n")), NONE);
    }

    #[test]
    fn finds_a_generated_with_line() {
        let generated = ["Gener", "ated"].concat();
        for line in [
            format!("{generated} with a tool"),
            format!("{}  by   the bot", generated.to_lowercase()),
            format!("Created by an {}", ["A", "I"].concat()),
            format!("This commit was {} an agent session", CREATED[0]),
        ] {
            assert_eq!(rules(&line), ["a generated-with line"], "{line}");
        }
        for line in [
            format!("Re{} with cargo update", generated.to_lowercase()),
            format!("{generated} bytes are not kept"),
            "Created with a plan in mind".to_string(),
            "Created by an aim to be brief".to_string(),
        ] {
            assert_eq!(rules(&line), NONE, "{line}");
        }
    }

    #[test]
    fn finds_the_robot_emoji() {
        assert_eq!(rules(&format!("{ROBOT} made this")), ["the robot face emoji (U+1F916)"]);
    }

    #[test]
    fn finds_a_tool_name_as_a_whole_word_in_any_case() {
        for name in NAMES {
            let line = format!("Ask {}-review for a pass", name.to_uppercase());
            assert_eq!(rules(&line), [format!("a tool or vendor name: {name}")], "{name}");
        }
        assert_eq!(rules(&format!("A note for {}tte", NAMES[0])), NONE);
    }

    #[test]
    fn reads_records_split_by_nul_and_0x01() {
        let (first, second) = ("a".repeat(40), "b".repeat(64));
        let out = format!("{first}\0Subject one\n\nBody.\n{RECORD_END}\n{second}\0Subject two\n{RECORD_END}\n");
        let (commits, broken) = parse_log(&out);
        assert_eq!(broken, 0);
        let expected =
            [Commit { hash: &first, body: "Subject one\n\nBody.\n" }, Commit { hash: &second, body: "Subject two\n" }];
        assert_eq!(commits, expected);
    }

    #[test]
    fn a_record_that_does_not_split_is_counted() {
        let out = format!("{}\0One\n{RECORD_END}\nno hash here{RECORD_END}\n", "c".repeat(40));
        let (commits, broken) = parse_log(&out);
        assert_eq!((commits.len(), broken), (1, 1));
    }

    #[test]
    fn reads_the_mode_of_an_index_entry() {
        assert_eq!(index_mode(&format!("100644 {} 0\t{HOOK}\n", "d".repeat(40))), Some("100644"));
        assert_eq!(index_mode(""), None);
    }

    #[test]
    fn knows_the_hooks_directory() {
        assert!(is_hooks_dir(".githooks"));
        assert!(is_hooks_dir("./.githooks/"));
        assert!(!is_hooks_dir(".git/hooks"));
    }

    /// The check on this repository. The history changes, so the test asserts
    /// the contract only: a pass always comes with one commit read or more.
    #[test]
    fn the_check_on_this_repository_keeps_the_floor_contract() {
        let Ok(repo) = Repo::find() else { return };
        let report = check(&repo);
        report.print_text();
        assert!(report.status != Status::Pass || report.scanned >= FLOOR);
    }
}
