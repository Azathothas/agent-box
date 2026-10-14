//! Check `secrets`: does a file that git would publish hold a credential?
//!
//! The defect this check exists to catch: a credential, or a private key,
//! reaches a remote. A history rewrite does not undo that. The value was
//! readable, and a mirror or a cache can keep it. After that, the only fix is
//! to rotate the credential. This check stops the need for that fix.
//!
//! The scope is the tracked files plus the untracked files that git does not
//! ignore. A new file that nobody staged is where a new credential is most
//! likely to be, and the next `git add -A` takes it. Ignored files stay out,
//! because they are ignored on purpose.
//!
//! WARNING: The check finds the shapes that it knows. A pass is not a
//! clearance. It cannot find a password that looks like a word, so read the
//! diff too.
//!
//! A finding never shows the value. It shows the name of the rule and the
//! first four characters of the match.

mod rules;

use crate::repo::Repo;
use crate::report::{CheckReport, Finding, Status};

pub(crate) const DEFECT: &str = "a credential or a private key in a file that git would publish";

const NAME: &str = "secrets";

/// Fewer text files than this means that the scope is wrong, not that the tree is clean.
const FLOOR: usize = 20;

pub(crate) fn check(repo: &Repo) -> CheckReport {
    let files = match repo.visible_files() {
        Ok(files) => files,
        Err(e) => return CheckReport::not_run(NAME, format!("cannot list the files of the repository: {e}")),
    };
    let mut findings = Vec::new();
    let mut scope = Scope::default();
    for rel in &files {
        // A credential file is a finding by its name alone, also when it is binary.
        if let Some(why) = rules::credential_file(rel) {
            findings.push(Finding::new(rel.as_str(), 0, why));
        }
        if let Some(text) = scope.read(repo, rel) {
            findings.extend(rules::scan_text(rel, &text));
        }
    }
    let found = !findings.is_empty();
    let report = scope.close(CheckReport::from_findings(NAME, scope.text_files, FLOOR, findings));
    let report = report.note("the rules find known token shapes only; a pass is not a clearance");
    if found {
        report.note("for a real credential: rotate it first, then tell the operator, then remove it from the tree")
    } else {
        report
    }
}

/// What a check read of its scope: the text files, the binary files that it
/// did not read, and the files that it could not read.
#[derive(Debug, Default)]
pub(super) struct Scope {
    pub(super) text_files: usize,
    binary_files: usize,
    unreadable: Vec<String>,
}

impl Scope {
    /// The text of one file of the scope, or `None` for a binary file or a file
    /// that cannot be read. A binary file has a NUL byte, which is the rule of
    /// `grep -I`. A byte that is not UTF-8 becomes U+FFFD, so that one bad byte
    /// does not hide the rest of the file.
    pub(super) fn read(&mut self, repo: &Repo, rel: &str) -> Option<String> {
        let bytes = match repo.read_bytes(rel) {
            Ok(bytes) => bytes,
            Err(e) => {
                self.unreadable.push(e);
                return None;
            }
        };
        if bytes.contains(&0) {
            self.binary_files += 1;
            return None;
        }
        self.text_files += 1;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Add the facts about the scope to a report. A file that could not be
    /// read is a gap in the scope, so a report with no findings becomes
    /// `NotRun`: the check did not read all that it must read.
    pub(super) fn close(&self, mut report: CheckReport) -> CheckReport {
        if self.binary_files > 0 {
            report = report.note(format!("did not read {} binary files (a NUL byte)", self.binary_files));
        }
        for e in &self.unreadable {
            report = report.note(format!("could not read {e}"));
        }
        if !self.unreadable.is_empty() && report.status == Status::Pass {
            report.status = Status::NotRun;
            report = report.note("a file of the scope could not be read, so this is not a pass");
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreadable_file_turns_a_pass_into_not_run() {
        let scope = Scope { text_files: 30, binary_files: 0, unreadable: vec!["a.md: denied".to_string()] };
        let report = scope.close(CheckReport::from_findings(NAME, 30, FLOOR, Vec::new()));
        assert_eq!(report.status, Status::NotRun);
    }

    #[test]
    fn an_unreadable_file_does_not_hide_a_failure() {
        let scope = Scope { text_files: 30, binary_files: 0, unreadable: vec!["a.md: denied".to_string()] };
        let found = vec![Finding::new("b.md", 1, "a rule")];
        let report = scope.close(CheckReport::from_findings(NAME, 30, FLOOR, found));
        assert_eq!(report.status, Status::Fail);
    }

    #[test]
    fn an_empty_scope_is_not_a_pass() {
        let scope = Scope::default();
        let report = scope.close(CheckReport::from_findings(NAME, 0, FLOOR, Vec::new()));
        assert_eq!(report.status, Status::NotRun);
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
