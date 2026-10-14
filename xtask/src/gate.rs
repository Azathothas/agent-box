//! `cargo xtask gate`: run every step of the repository gate in one command.
//!
//! The defect this command exists to catch: a gate that is a list. A person who
//! runs a list by hand runs a different subset each time. A step that did not
//! run then looks the same as a step that passed. The gate runs the same steps
//! in the same order, and it reads each exit code from the process that gave
//! it. It never counts a step that did not run as a pass.
//!
//! Exit codes: 0 every step passed; 1 a step failed, or with `--strict` a step
//! did not run; 2 no step failed but a step did not run, or no step ran; 64
//! usage error.

use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::check;
use crate::proc;
use crate::repo::Repo;
use crate::report::{self, CheckReport, Finding, Status};

const USAGE: &str = "usage: cargo xtask gate [--strict] [--list] [--json] [STEP...]

  The steps, in the order the gate runs them: fmt, clippy, test, checks, deny.

  STEP...   run only the named steps, in the order above
  --strict  a step that did not run is a failure (the CI mode)
  --list    print the step names, one per line, and stop (a JSON array with --json)
  --json    print one JSON object at the end; the output of cargo goes to stderr

Exit codes: 0 pass, 1 fail, 2 a step did not run, 64 usage error.";

/// The steps, in the order the gate runs them.
const STEPS: [&str; 5] = ["fmt", "clippy", "test", "checks", "deny"];

/// The number of parallel compiler jobs when `CARGO_BUILD_JOBS` is not set.
/// A workspace build with one job for each core can use all the memory of a
/// small host. Set the variable to use a different number.
const DEFAULT_JOBS: &str = "4";

/// The time limit of a version probe of a cargo subcommand. The first call in
/// the repository can make rustup install the pinned toolchain, which takes
/// some minutes.
const PROBE_LIMIT: Duration = Duration::from_mins(10);

/// What the command line asks for.
#[derive(Debug)]
struct Options {
    strict: bool,
    list: bool,
    json: bool,
    /// The steps to run, in the order of [`STEPS`], each one time.
    steps: Vec<&'static str>,
}

/// The result of one step.
#[derive(Debug)]
struct StepResult {
    name: &'static str,
    status: Status,
    elapsed: Duration,
    /// One line: the command, or why the step did not run, or how it failed.
    detail: String,
    /// The reports of the step `checks`. The other steps have none.
    reports: Vec<CheckReport>,
}

/// `cargo xtask gate [--strict] [--list] [--json] [STEP...]`
pub(crate) fn run(args: &[String]) -> u8 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return report::EXIT_PASS;
    }
    let opts = match parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("xtask gate: {e}\n\n{USAGE}");
            return report::EXIT_USAGE;
        }
    };
    if opts.list {
        print_list(opts.json);
        return report::EXIT_PASS;
    }
    let repo = match Repo::find() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("xtask gate: {e}");
            return report::EXIT_NOT_RUN;
        }
    };
    let mut results = Vec::new();
    for name in &opts.steps {
        say(opts.json, &format!("\n== gate step: {name} =="));
        results.push(run_step(&repo, name, opts.json));
    }
    let statuses: Vec<Status> = results.iter().map(|r| r.status).collect();
    let code = verdict(&statuses, opts.strict);
    if opts.json {
        println!("{:#}", to_json(&results, &opts, code));
    } else {
        print_summary(&results, &opts, code);
    }
    code
}

/// Read the command line. An unknown option or step is a usage error.
fn parse(args: &[String]) -> Result<Options, String> {
    let mut opts = Options { strict: false, list: false, json: false, steps: Vec::new() };
    let mut named: Vec<&str> = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--strict" => opts.strict = true,
            "--list" => opts.list = true,
            "--json" => opts.json = true,
            flag if flag.starts_with('-') => return Err(format!("unknown option `{flag}`")),
            name if STEPS.contains(&name) => named.push(name),
            name => return Err(format!("no step named `{name}`; the steps are: {}", STEPS.join(", "))),
        }
    }
    opts.steps =
        if named.is_empty() { STEPS.to_vec() } else { STEPS.iter().copied().filter(|s| named.contains(s)).collect() };
    Ok(opts)
}

/// The exit code of the gate from the status of each step. A failure wins. A
/// step that did not run is not a pass, and zero steps that passed is not a
/// pass.
fn verdict(statuses: &[Status], strict: bool) -> u8 {
    let failed = statuses.contains(&Status::Fail);
    let incomplete = statuses.contains(&Status::NotRun) || !statuses.contains(&Status::Pass);
    if failed || (strict && incomplete) {
        report::EXIT_FAIL
    } else if incomplete {
        report::EXIT_NOT_RUN
    } else {
        report::EXIT_PASS
    }
}

fn print_list(as_json: bool) {
    if as_json {
        println!("{}", json!(STEPS));
    } else {
        for s in STEPS {
            println!("{s}");
        }
    }
}

/// Progress lines go to stdout for a person. With `--json` they go to stderr,
/// so stdout holds only the JSON object.
fn say(as_json: bool, line: &str) {
    if as_json {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

fn run_step(repo: &Repo, name: &'static str, as_json: bool) -> StepResult {
    let start = Instant::now();
    let mut reports = Vec::new();
    let (status, detail) = match name {
        "fmt" => tool_step(repo, "fmt", "rustfmt", &["fmt", "--all", "--", "--check"], as_json),
        "clippy" => tool_step(
            repo,
            "clippy",
            "clippy",
            &["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"],
            as_json,
        ),
        "test" => cargo(repo, &["test", "--workspace", "--locked"], as_json),
        "checks" => {
            reports = run_checks(repo, as_json);
            checks_outcome(&reports)
        }
        "deny" => deny(repo, as_json),
        other => (Status::NotRun, format!("the gate has no step named `{other}`")),
    };
    StepResult { name, status, elapsed: start.elapsed(), detail, reports }
}

/// A cargo step that needs a rustup component. When the component does not
/// answer, the step did not run: that is a fact about the host, not the code.
fn tool_step(repo: &Repo, sub: &str, component: &str, args: &[&str], as_json: bool) -> (Status, String) {
    if let Err(reason) = probe_cargo_tool(repo, sub, component) {
        return (Status::NotRun, reason);
    }
    cargo(repo, args, as_json)
}

/// Run `cargo SUB --version` in the repository root, so the pinned toolchain
/// answers and not the default one.
fn probe_cargo_tool(repo: &Repo, sub: &str, component: &str) -> Result<(), String> {
    let out = proc::run_in(&repo.root, "cargo", &[sub, "--version"], PROBE_LIMIT)
        .map_err(|e| format!("cargo did not start: {e}"))?;
    if out.success() {
        Ok(())
    } else if out.timed_out {
        Err(format!("`cargo {sub} --version` did not answer in {} s", PROBE_LIMIT.as_secs()))
    } else {
        Err(format!(
            "`cargo {sub} --version` failed ({}); `rustup component add {component}` installs it",
            first_line(&out.stderr)
        ))
    }
}

/// Run one cargo command with the terminal attached, so the developer sees
/// the compiler output when it comes.
fn cargo(repo: &Repo, args: &[&str], as_json: bool) -> (Status, String) {
    let shown = format!("cargo {}", args.join(" "));
    say(as_json, &format!("gate: {shown}"));
    let mut cmd = Command::new("cargo");
    cmd.args(args).current_dir(&repo.root);
    if std::env::var_os("CARGO_BUILD_JOBS").is_none() {
        cmd.env("CARGO_BUILD_JOBS", DEFAULT_JOBS);
    }
    if as_json {
        cmd.stdout(std::io::stderr());
    }
    match proc::run_attached(&mut cmd) {
        Some(0) => (Status::Pass, shown),
        Some(code) => (Status::Fail, format!("{shown}: exit code {code}")),
        None => (Status::NotRun, format!("{shown}: cargo did not start, or a signal stopped it")),
    }
}

/// Run every registered check. A check that panics gets a failed report, so
/// the other checks still run and the summary still prints.
fn run_checks(repo: &Repo, as_json: bool) -> Vec<CheckReport> {
    check::CHECKS
        .iter()
        .map(|c| {
            let r = std::panic::catch_unwind(|| (c.run)(repo)).unwrap_or_else(|_| panicked(c.name));
            if !as_json {
                r.print_text();
            }
            r
        })
        .collect()
}

/// The report of a check that panicked. The check has a defect of its own, so
/// this is a failure. The panic message is on stderr.
fn panicked(name: &'static str) -> CheckReport {
    let finding = Finding::new("xtask/src/check", 0, format!("the check `{name}` stopped with a panic"));
    CheckReport::from_findings(name, 0, 0, vec![finding])
}

fn checks_outcome(reports: &[CheckReport]) -> (Status, String) {
    let count = |s: Status| reports.iter().filter(|r| r.status == s).count();
    let detail = format!(
        "{} checks: {} pass, {} fail, {} not run",
        reports.len(),
        count(Status::Pass),
        count(Status::Fail),
        count(Status::NotRun)
    );
    (report::worst(reports), detail)
}

/// cargo-deny is not a rustup component. The step runs only when the tool
/// answers a version probe.
fn deny(repo: &Repo, as_json: bool) -> (Status, String) {
    if proc::probe_version("cargo-deny", &["--version"]).is_none() {
        return (Status::NotRun, "cargo-deny is not installed; cargo xtask setup installs it".to_string());
    }
    cargo(repo, &["deny", "--locked", "check"], as_json)
}

fn print_summary(results: &[StepResult], opts: &Options, code: u8) {
    println!();
    println!("gate summary{}", if opts.strict { " (--strict)" } else { "" });
    println!("  step     status       time  detail");
    for r in results {
        println!("  {:8} {:8} {:>8}  {}", r.name, r.status.word(), format_duration(r.elapsed), r.detail);
        // Under the step `checks`, one line for each check that did not pass.
        for c in r.reports.iter().filter(|c| c.status != Status::Pass) {
            let why = if c.status == Status::Fail {
                format!("{} findings", c.findings.len())
            } else {
                c.notes.first().cloned().unwrap_or_default()
            };
            println!("           {:8}           check {}: {why}", c.status.word(), c.name);
        }
    }
    let count = |s: Status| results.iter().filter(|r| r.status == s).count();
    println!(
        "gate: {}. {} pass, {} fail, {} not run.",
        code_status(code).word(),
        count(Status::Pass),
        count(Status::Fail),
        count(Status::NotRun)
    );
    if count(Status::NotRun) > 0 {
        if opts.strict {
            println!("NOTE: With --strict, a step that did not run is a failure.");
        } else if count(Status::Fail) == 0 {
            println!("NOTE: A step that did not run is not a pass, so the gate exits 2. --strict makes it a failure.");
        } else {
            println!("NOTE: A step that did not run is not a pass.");
        }
    }
    let skipped: Vec<&str> = STEPS.iter().copied().filter(|s| !opts.steps.contains(s)).collect();
    if !skipped.is_empty() {
        println!("NOTE: Only the named steps ran. These steps of the gate did not run: {}.", skipped.join(", "));
    }
}

fn to_json(results: &[StepResult], opts: &Options, code: u8) -> Value {
    let skipped: Vec<&str> = STEPS.iter().copied().filter(|s| !opts.steps.contains(s)).collect();
    json!({
        "status": status_key(code_status(code)),
        "strict": opts.strict,
        "steps": results.iter().map(step_json).collect::<Vec<_>>(),
        "not_selected": skipped,
    })
}

fn step_json(r: &StepResult) -> Value {
    let mut v = json!({
        "name": r.name,
        "status": status_key(r.status),
        "millis": u64::try_from(r.elapsed.as_millis()).unwrap_or(u64::MAX),
        "detail": r.detail,
    });
    if r.name == "checks" {
        v["checks"] = Value::Array(r.reports.iter().map(CheckReport::to_json).collect());
    }
    v
}

/// The status that an exit code of the gate stands for.
fn code_status(code: u8) -> Status {
    match code {
        report::EXIT_PASS => Status::Pass,
        report::EXIT_FAIL => Status::Fail,
        _ => Status::NotRun,
    }
}

/// The JSON word of a status, the same words as in `report.rs`.
fn status_key(s: Status) -> &'static str {
    match s {
        Status::Pass => "pass",
        Status::Fail => "fail",
        Status::NotRun => "not_run",
    }
}

/// A duration for a person: `0.4s`, `59.9s`, `2m 05s`.
fn format_duration(d: Duration) -> String {
    let tenths = d.as_millis() / 100;
    if tenths < 600 {
        format!("{}.{}s", tenths / 10, tenths % 10)
    } else {
        let secs = d.as_secs();
        format!("{}m {:02}s", secs / 60, secs % 60)
    }
}

fn first_line(text: &str) -> &str {
    text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("no message")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn every_step_passed_is_a_pass() {
        assert_eq!(verdict(&[Status::Pass, Status::Pass], false), report::EXIT_PASS);
        assert_eq!(verdict(&[Status::Pass, Status::Pass], true), report::EXIT_PASS);
    }

    #[test]
    fn a_step_that_did_not_run_is_not_a_pass() {
        let s = [Status::Pass, Status::NotRun];
        assert_eq!(verdict(&s, false), report::EXIT_NOT_RUN);
        assert_eq!(verdict(&s, true), report::EXIT_FAIL);
    }

    #[test]
    fn a_failure_wins_over_not_run() {
        let s = [Status::NotRun, Status::Fail, Status::Pass];
        assert_eq!(verdict(&s, false), report::EXIT_FAIL);
    }

    #[test]
    fn zero_steps_is_not_a_pass() {
        assert_eq!(verdict(&[], false), report::EXIT_NOT_RUN);
        assert_eq!(verdict(&[], true), report::EXIT_FAIL);
        assert_eq!(verdict(&[Status::NotRun], false), report::EXIT_NOT_RUN);
    }

    #[test]
    fn no_step_named_runs_every_step_in_order() {
        let o = parse(&args(&["--strict"])).unwrap();
        assert!(o.strict && !o.list && !o.json);
        assert_eq!(o.steps, STEPS.to_vec());
    }

    #[test]
    fn named_steps_run_in_gate_order_one_time() {
        let o = parse(&args(&["deny", "fmt", "deny", "--json"])).unwrap();
        assert!(o.json);
        assert_eq!(o.steps, vec!["fmt", "deny"]);
    }

    #[test]
    fn an_unknown_step_or_option_is_a_usage_error() {
        assert!(parse(&args(&["lint"])).is_err());
        assert!(parse(&args(&["--fast"])).is_err());
    }

    #[test]
    fn a_panicked_check_is_a_failure() {
        assert_eq!(panicked("text").status, Status::Fail);
    }

    #[test]
    fn the_checks_step_takes_the_worst_report() {
        let pass = CheckReport::from_findings("a", 3, 1, Vec::new());
        let empty = CheckReport::from_findings("b", 0, 1, Vec::new());
        let (status, detail) = checks_outcome(&[pass, empty]);
        assert_eq!(status, Status::NotRun);
        assert_eq!(detail, "2 checks: 1 pass, 0 fail, 1 not run");
        assert_eq!(checks_outcome(&[]).0, Status::NotRun);
    }

    #[test]
    fn durations_read_as_seconds_then_minutes() {
        assert_eq!(format_duration(Duration::from_millis(400)), "0.4s");
        assert_eq!(format_duration(Duration::from_millis(59_950)), "59.9s");
        assert_eq!(format_duration(Duration::from_secs(125)), "2m 05s");
    }

    #[test]
    fn the_json_names_the_steps_that_were_not_selected() {
        let o = parse(&args(&["fmt"])).unwrap();
        let r = StepResult {
            name: "fmt",
            status: Status::Pass,
            elapsed: Duration::from_millis(5),
            detail: String::new(),
            reports: Vec::new(),
        };
        let v = to_json(&[r], &o, report::EXIT_PASS);
        assert_eq!(v["status"], "pass");
        assert_eq!(v["not_selected"], json!(["clippy", "test", "checks", "deny"]));
        assert_eq!(v["steps"][0]["millis"], 5);
    }
}
