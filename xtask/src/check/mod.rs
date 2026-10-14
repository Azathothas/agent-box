//! The repository checks. Each check is one function that reads the repository
//! and returns a [`CheckReport`]. `cargo xtask check NAME...` runs some of
//! them, and the gate runs all of them.
//!
//! The contract of every check (from the operator's TEMPLATE): a header comment
//! names the defect it exists to catch; exit 0 pass, 1 fail, 2 could not run;
//! `--json` gives a structured answer; it does not depend on the working
//! directory; it is read-only; and it has a floor, so an empty scope is not a pass.

mod attribution;
mod docs;
mod one_home;
mod placeholders;
mod prose;
mod secrets;
mod text;

use crate::repo::Repo;
use crate::report::{self, CheckReport};

/// One registered check.
pub(crate) struct Check {
    pub(crate) name: &'static str,
    /// The defect the check exists to catch, in one line.
    pub(crate) defect: &'static str,
    pub(crate) run: fn(&Repo) -> CheckReport,
}

/// Every check, in the order the gate runs them.
pub(crate) const CHECKS: &[Check] = &[
    Check { name: "text", defect: text::DEFECT, run: text::check },
    Check { name: "prose", defect: prose::DEFECT, run: prose::check },
    Check { name: "docs", defect: docs::DEFECT, run: docs::check },
    Check { name: "one-home", defect: one_home::DEFECT, run: one_home::check },
    Check { name: "placeholders", defect: placeholders::DEFECT, run: placeholders::check },
    Check { name: "secrets", defect: secrets::DEFECT, run: secrets::check },
    Check { name: "attribution", defect: attribution::DEFECT, run: attribution::check },
    Check { name: "layering", defect: crate::layering::DEFECT, run: crate::layering::check },
    Check { name: "todo", defect: crate::todo::DEFECT, run: crate::todo::check },
];

/// Find a check by name.
pub(crate) fn find(name: &str) -> Option<&'static Check> {
    CHECKS.iter().find(|c| c.name == name)
}

/// `cargo xtask check [--json] [--list] NAME...|all`
pub(crate) fn run(args: &[String]) -> u8 {
    let as_json = args.iter().any(|a| a == "--json");
    if args.iter().any(|a| a == "--list") {
        for c in CHECKS {
            println!("{:13} {}", c.name, c.defect);
        }
        return report::EXIT_PASS;
    }
    let names: Vec<&str> = args.iter().map(String::as_str).filter(|a| !a.starts_with('-')).collect();
    if names.is_empty() {
        eprintln!("xtask check: name one check or more, or `all`; `cargo xtask check --list` names them");
        return report::EXIT_USAGE;
    }
    let selected: Vec<&Check> = if names == ["all"] {
        CHECKS.iter().collect()
    } else {
        let mut v = Vec::new();
        for n in names {
            let Some(c) = find(n) else {
                eprintln!("xtask check: no check named `{n}`; `cargo xtask check --list` names them");
                return report::EXIT_USAGE;
            };
            v.push(c);
        }
        v
    };
    let repo = match Repo::find() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("xtask check: {e}");
            return report::EXIT_NOT_RUN;
        }
    };
    let reports: Vec<CheckReport> = selected.iter().map(|c| (c.run)(&repo)).collect();
    report::emit(&reports, as_json)
}
