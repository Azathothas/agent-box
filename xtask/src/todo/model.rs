//! The words of the record: the values that the fields of an entry can take,
//! and the shapes that the reader and the writer share. `TODO/INDEX.md` gives
//! the meaning of each value. This file is the list that the checker accepts.

use std::fmt;

pub(crate) const STATUSES: [&str; 4] = ["open", "partial", "blocked", "done"];
pub(crate) const PRIORITIES: [&str; 4] = ["P0", "P1", "P2", "P3"];
/// No `XL`: an entry that big is two entries.
pub(crate) const EFFORTS: [&str; 3] = ["S", "M", "L"];
pub(crate) const MILESTONES: [&str; 14] =
    ["M0", "M1", "M2", "M3", "M4", "M5", "M6", "M7", "M8", "M9", "M10", "M11", "M12", "backlog"];
pub(crate) const CATEGORIES: [&str; 8] =
    ["defect", "feature", "measurement", "release", "chore", "research", "docs", "decision"];

/// The header fields of an entry. Each entry gives each field once.
pub(crate) const FIELDS: [&str; 6] = ["Source", "Category", "Milestone", "Priority", "Effort", "Status"];

/// The sections that an entry can have, in this order.
pub(crate) const SECTIONS: [&str; 9] =
    ["Problem", "Premise", "Approach", "Decision", "Prove", "Blocker", "Start condition", "Correction", "Done"];
pub(crate) const REQUIRED_SECTIONS: [&str; 4] = ["Problem", "Premise", "Approach", "Prove"];

/// The files of `TODO/` that hold no entries.
pub(crate) const RECORD_FILES: [&str; 4] = ["INDEX.md", "PROGRESS.md", "RULES.md", "upstream.md"];

/// One finding of the reader: the file, the line (0 for the whole file), and
/// what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) what: String,
}

impl Problem {
    pub(crate) fn new(file: &str, line: usize, what: impl Into<String>) -> Self {
        Self { file: file.to_string(), line, what: what.into() }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}: {}", self.file, self.what)
        } else {
            write!(f, "{}:{}: {}", self.file, self.line, self.what)
        }
    }
}

/// A row of the table of entries in `TODO/INDEX.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) id: String,
    pub(crate) file: String,
    pub(crate) priority: String,
    pub(crate) effort: String,
    pub(crate) milestone: String,
    pub(crate) category: String,
    pub(crate) status: String,
    pub(crate) title: String,
    pub(crate) line: usize,
}

/// A header field of an entry, with the line where it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) value: String,
    pub(crate) line: usize,
}

/// A `## Name` section of an entry, with its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Section {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) body: String,
}

/// One entry: `# T-NNN: Title`, its fields and its sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) fields: Vec<Field>,
    pub(crate) sections: Vec<Section>,
}

impl Entry {
    /// The value of a field, when the entry gives it.
    pub(crate) fn field(&self, name: &str) -> Option<&str> {
        self.fields.iter().find(|f| f.name == name).map(|f| f.value.as_str())
    }

    pub(crate) fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }
}

/// The entries in each status. The total is their sum, never a stored number.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) open: usize,
    pub(crate) partial: usize,
    pub(crate) blocked: usize,
    pub(crate) done: usize,
}

impl Counts {
    pub(crate) fn total(&self) -> usize {
        self.open + self.partial + self.blocked + self.done
    }

    /// Set the count of one status. An unknown status changes nothing.
    pub(crate) fn set(&mut self, status: &str, n: usize) {
        match status {
            "open" => self.open = n,
            "partial" => self.partial = n,
            "blocked" => self.blocked = n,
            "done" => self.done = n,
            _ => {}
        }
    }

    pub(crate) fn add(&mut self, status: &str) {
        match status {
            "open" => self.open += 1,
            "partial" => self.partial += 1,
            "blocked" => self.blocked += 1,
            "done" => self.done += 1,
            _ => {}
        }
    }

    /// The counts of the rows, for one priority or (with `None`) for all.
    pub(crate) fn of(rows: &[Row], priority: Option<&str>) -> Self {
        let mut counts = Self::default();
        for row in rows.iter().filter(|r| priority.is_none_or(|p| r.priority == p)) {
            counts.add(&row.status);
        }
        counts
    }

    /// The fixed phrase that the index and the progress record carry, so that
    /// a checker can read it: `N entries: A open, B partial, C blocked, D done.`
    pub(crate) fn phrase(&self) -> String {
        format!(
            "{} entries: {} open, {} partial, {} blocked, {} done.",
            self.total(),
            self.open,
            self.partial,
            self.blocked,
            self.done
        )
    }

    /// A row of the table of counts in the index.
    pub(crate) fn table_row(&self, label: &str) -> String {
        format!("| {label} | {} | {} | {} | {} | {} |", self.open, self.partial, self.blocked, self.done, self.total())
    }
}

/// True for an id of the form `T-NNN` (three digits or more).
pub(crate) fn is_id(s: &str) -> bool {
    s.strip_prefix("T-").is_some_and(|n| n.len() >= 3 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The number of an id, for the order of the rows.
pub(crate) fn id_number(id: &str) -> u32 {
    id.get(2..).and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// Each `T-NNN` in a text, with the line where it is.
pub(crate) fn ids_in(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut at = 0;
        while let Some(pos) = line[at..].find("T-") {
            let start = at + pos;
            let before_ok = start == 0 || !(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'-');
            let digits = bytes[start + 2..].iter().take_while(|b| b.is_ascii_digit()).count();
            let end = start + 2 + digits;
            let after_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
            if before_ok && digits >= 3 && after_ok {
                out.push((line[start..end].to_string(), idx + 1));
            }
            at = start + 2;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_found_as_words_only() {
        let found: Vec<String> =
            ids_in("T-001 and (T-012), not XT-003, not T-04, T-1234.").into_iter().map(|(id, _)| id).collect();
        assert_eq!(found, ["T-001", "T-012", "T-1234"]);
    }

    #[test]
    fn the_phrase_is_derived_from_the_counts() {
        let c = Counts { open: 3, partial: 1, blocked: 2, done: 4 };
        assert_eq!(c.phrase(), "10 entries: 3 open, 1 partial, 2 blocked, 4 done.");
        assert_eq!(c.table_row("P1"), "| P1 | 3 | 1 | 2 | 4 | 10 |");
    }

    #[test]
    fn an_id_has_three_digits_or_more() {
        assert!(is_id("T-001"));
        assert!(is_id("T-1234"));
        assert!(!is_id("T-01"));
        assert!(!is_id("T-00a"));
        assert!(!is_id("X-001"));
        assert_eq!(id_number("T-042"), 42);
    }

    #[test]
    fn the_milestones_are_m0_to_m12_and_the_backlog() {
        assert_eq!(MILESTONES.first(), Some(&"M0"));
        assert!(MILESTONES.contains(&"M12"));
        assert!(MILESTONES.contains(&"backlog"));
        assert!(!MILESTONES.contains(&"none"));
    }
}
