//! The result of one check, its exit code and its two output forms (text for a
//! person, JSON with `--json`).
//!
//! The defect this module guards against: a check that read nothing and
//! printed a pass. Each check gives the number of items it scanned and its
//! floor; below the floor the status is `NotRun`, never `Pass`.

use serde_json::{Value, json};

pub(crate) const EXIT_PASS: u8 = 0;
pub(crate) const EXIT_FAIL: u8 = 1;
pub(crate) const EXIT_NOT_RUN: u8 = 2;
pub(crate) const EXIT_USAGE: u8 = 64;

/// The three outcomes. "Could not run" and "failed" are different facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    Pass,
    Fail,
    NotRun,
}

impl Status {
    pub(crate) fn exit_code(self) -> u8 {
        match self {
            Status::Pass => EXIT_PASS,
            Status::Fail => EXIT_FAIL,
            Status::NotRun => EXIT_NOT_RUN,
        }
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Fail => "FAIL",
            Status::NotRun => "NOT RUN",
        }
    }
}

/// One problem: where it is and what is wrong. `line` 0 means the whole file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) message: String,
}

impl Finding {
    pub(crate) fn new(path: impl Into<String>, line: usize, message: impl Into<String>) -> Self {
        Self { path: path.into(), line, message: message.into() }
    }

    pub(crate) fn location(&self) -> String {
        if self.line == 0 { self.path.clone() } else { format!("{}:{}", self.path, self.line) }
    }
}

/// The result of one check.
#[derive(Debug, Clone)]
pub(crate) struct CheckReport {
    pub(crate) name: &'static str,
    pub(crate) status: Status,
    /// How many items (files, commits, crates, entries) the check read.
    pub(crate) scanned: usize,
    pub(crate) findings: Vec<Finding>,
    /// Facts that are not findings: what was skipped and why, what was read.
    pub(crate) notes: Vec<String>,
}

impl CheckReport {
    /// A report whose status follows from the findings and the floor.
    /// Below the floor the check read too little to say anything: `NotRun`.
    pub(crate) fn from_findings(name: &'static str, scanned: usize, floor: usize, findings: Vec<Finding>) -> Self {
        let mut notes = Vec::new();
        let status = if scanned < floor {
            notes.push(format!("read {scanned} items, below the floor of {floor}: the scope is empty or wrong"));
            Status::NotRun
        } else if findings.is_empty() {
            Status::Pass
        } else {
            Status::Fail
        };
        Self { name, status, scanned, findings, notes }
    }

    /// A report for a check that could not run at all, with the reason.
    pub(crate) fn not_run(name: &'static str, reason: impl Into<String>) -> Self {
        Self { name, status: Status::NotRun, scanned: 0, findings: Vec::new(), notes: vec![reason.into()] }
    }

    pub(crate) fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub(crate) fn print_text(&self) {
        println!("{:7} {} ({} read, {} findings)", self.status.word(), self.name, self.scanned, self.findings.len());
        for f in &self.findings {
            println!("  {}: {}", f.location(), f.message);
        }
        for n in &self.notes {
            println!("  note: {n}");
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "status": match self.status { Status::Pass => "pass", Status::Fail => "fail", Status::NotRun => "not_run" },
            "scanned": self.scanned,
            "findings": self.findings.iter().map(|f| json!({"path": f.path, "line": f.line, "message": f.message})).collect::<Vec<_>>(),
            "notes": self.notes,
        })
    }
}

/// The worst status of several reports: a failure wins over not-run, which
/// wins over a pass. Zero reports is not a pass.
pub(crate) fn worst(reports: &[CheckReport]) -> Status {
    if reports.is_empty() {
        return Status::NotRun;
    }
    if reports.iter().any(|r| r.status == Status::Fail) {
        Status::Fail
    } else if reports.iter().any(|r| r.status == Status::NotRun) {
        Status::NotRun
    } else {
        Status::Pass
    }
}

/// Print several reports in the chosen form and return the exit code.
pub(crate) fn emit(reports: &[CheckReport], as_json: bool) -> u8 {
    let status = worst(reports);
    if as_json {
        let doc = json!({
            "status": match status { Status::Pass => "pass", Status::Fail => "fail", Status::NotRun => "not_run" },
            "checks": reports.iter().map(CheckReport::to_json).collect::<Vec<_>>(),
        });
        println!("{doc:#}");
    } else {
        for r in reports {
            r.print_text();
        }
    }
    status.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_the_floor_is_not_a_pass() {
        let r = CheckReport::from_findings("t", 0, 1, Vec::new());
        assert_eq!(r.status, Status::NotRun);
    }

    #[test]
    fn zero_reports_is_not_a_pass() {
        assert_eq!(worst(&[]), Status::NotRun);
    }

    #[test]
    fn a_failure_wins_over_not_run() {
        let a = CheckReport::not_run("a", "x");
        let b = CheckReport::from_findings("b", 5, 1, vec![Finding::new("f", 1, "m")]);
        assert_eq!(worst(&[a, b]), Status::Fail);
    }
}
