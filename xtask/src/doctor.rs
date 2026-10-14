//! `cargo xtask doctor [--json] [--kvm]`: what this host can do for development.
//!
//! The defect this module exists to catch: a session that assumes its host. A
//! tool on PATH can be a stub, a gh token can be dead, and the active Rust can
//! differ from the pinned one. So every probe runs the tool and reads its
//! answer. Presence on PATH is not capability (TEMPLATE scripts/doctor/doctor.sh).
//!
//! A probe is not a gate. A missing tool is data, so the exit code is 0 when
//! the doctor ran. It is 2 only when no probe could run a program at all. The
//! doctor changes nothing. With `--kvm` on Windows it starts one container,
//! which wsl-toolkit removes when the container stops.

mod host;

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use crate::proc;
use crate::repo::Repo;
use crate::report;

/// A version query answers at once. The limit covers a cold start.
const VERSION_LIMIT: Duration = Duration::from_secs(20);

/// `gh api user` crosses the network.
const NETWORK_LIMIT: Duration = Duration::from_secs(30);

/// The longest evidence line that the doctor prints.
const EVIDENCE_MAX: usize = 200;

const USAGE: &str = "usage: cargo xtask doctor [--json] [--kvm]

Reports what this host can do for development. Each probe runs a tool and
prints ok, missing or ???? (could not tell) with its evidence. It changes
nothing. Exit 0 when it ran, 2 when no probe could run a program.

  --json  write a structured answer
  --kvm   on Windows, also open /dev/kvm in a container of the Linux runner
          (this starts a container; the first run pulls the image)";

/// The answer of one probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    Ok,
    Missing,
    /// The probe ran and could not tell. Printed as `????`.
    Unknown,
}

impl Verdict {
    fn word(self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Missing => "missing",
            Verdict::Unknown => "????",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Missing => "missing",
            Verdict::Unknown => "unknown",
        }
    }
}

/// One probe: its name, its verdict and the line that proves the verdict.
#[derive(Debug, Clone)]
pub(crate) struct Probe {
    pub(crate) name: &'static str,
    pub(crate) verdict: Verdict,
    pub(crate) evidence: String,
}

impl Probe {
    pub(crate) fn new(name: &'static str, verdict: Verdict, evidence: impl Into<String>) -> Self {
        Self { name, verdict, evidence: evidence.into() }
    }
}

pub(crate) fn run(args: &[String]) -> u8 {
    let mut as_json = false;
    let mut kvm = false;
    for a in args {
        match a.as_str() {
            "--json" => as_json = true,
            "--kvm" => kvm = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return report::EXIT_PASS;
            }
            other => {
                eprintln!("xtask doctor: unknown argument `{other}`\n\n{USAGE}");
                return report::EXIT_USAGE;
            }
        }
    }
    let mut notes = Vec::new();
    let repo = match Repo::find() {
        Ok(r) => Some(r),
        Err(e) => {
            notes.push(format!("the repository root is not found ({e}); the toolchain probe cannot read its pin"));
            None
        }
    };
    if kvm && !cfg!(windows) {
        notes.push("--kvm reaches the Linux runner on Windows only; this host opens /dev/kvm itself".to_string());
    }
    let probes = collect(repo.as_ref(), kvm);
    // The os probe reads the build target and runs nothing, so it does not
    // count. With no other answer, the doctor could not run at all.
    let ran = probes.iter().any(|p| p.name != "os" && p.verdict != Verdict::Unknown);
    if !ran {
        notes.push("no probe could run a program: the doctor could not run".to_string());
    }
    if as_json {
        print_json(&probes, &notes, ran);
    } else {
        print_text(&probes, &notes);
    }
    if ran { report::EXIT_PASS } else { report::EXIT_NOT_RUN }
}

fn collect(repo: Option<&Repo>, kvm: bool) -> Vec<Probe> {
    // rustup picks the toolchain from the directory, so the Rust tools answer
    // from the repository root. Other tools answer from the temp directory.
    let tmp = std::env::temp_dir();
    let rust_dir = repo.map_or_else(|| tmp.clone(), |r| r.root.clone());
    let mut v = vec![os_probe(&tmp)];
    let rustc = version_probe("rustc", &rust_dir, "rustc", &["--version"]);
    let cargo = version_probe("cargo", &rust_dir, "cargo", &["--version"]);
    let toolchain = toolchain_probe(repo, &rustc);
    v.extend([rustc, cargo, toolchain]);
    v.push(version_probe("git", &tmp, "git", &["--version"]));
    let gh = version_probe("gh", &tmp, "gh", &["--version"]);
    let gh_runs = gh.verdict == Verdict::Ok;
    v.push(gh);
    v.push(gh_auth_probe(&tmp, gh_runs));
    v.push(version_probe("cargo-deny", &tmp, "cargo-deny", &["--version"]));
    if cfg!(windows) {
        v.extend(host::windows(&tmp, kvm));
    } else if cfg!(target_os = "linux") {
        v.extend(host::linux(&tmp));
    }
    v
}

/// The OS and the architecture of this build, and the kernel line of the host.
fn os_probe(tmp: &Path) -> Probe {
    let (program, args): (&str, &[&str]) =
        if cfg!(windows) { ("cmd", &["/d", "/c", "ver"]) } else { ("uname", &["-sr"]) };
    let kernel = match proc::run_in(tmp, program, args, VERSION_LIMIT) {
        Ok(out) if out.success() => first_line(&out.stdout).unwrap_or_else(|| "no kernel line".to_string()),
        Ok(_) | Err(_) => format!("`{program}` gave no kernel line"),
    };
    Probe::new("os", Verdict::Ok, format!("{} {}; {kernel}", std::env::consts::OS, std::env::consts::ARCH))
}

/// Run `program args` and read its first line. A program that does not start
/// is missing. A program that starts and exits non-zero to a version query is
/// a stub or a broken install, which is missing too.
fn version_probe(name: &'static str, dir: &Path, program: &str, args: &[&str]) -> Probe {
    let shown = format!("{program} {}", args.join(" "));
    match proc::run_in(dir, program, args, VERSION_LIMIT) {
        Err(e) => Probe::new(name, Verdict::Missing, format!("does not start: {e}")),
        Ok(out) if out.timed_out => {
            Probe::new(name, Verdict::Unknown, format!("`{shown}` gave no answer in {}s", VERSION_LIMIT.as_secs()))
        }
        Ok(out) if !out.success() => {
            let why = first_line(&out.stderr).or_else(|| first_line(&out.stdout)).unwrap_or_default();
            let code = out.code.map_or_else(|| "no code".to_string(), |c| c.to_string());
            Probe::new(name, Verdict::Missing, format!("`{shown}` exited {code} (a stub or a broken install): {why}"))
        }
        Ok(out) => match first_line(&out.stdout).or_else(|| first_line(&out.stderr)) {
            Some(line) => Probe::new(name, Verdict::Ok, line),
            None => Probe::new(name, Verdict::Missing, format!("`{shown}` printed nothing: probably a stub")),
        },
    }
}

/// The pinned channel of rust-toolchain.toml against the rustc that answers
/// in the repository root.
fn toolchain_probe(repo: Option<&Repo>, rustc: &Probe) -> Probe {
    const NAME: &str = "toolchain";
    let Some(repo) = repo else {
        return Probe::new(NAME, Verdict::Unknown, "the repository root is not found, so the pin is not read");
    };
    let text = match repo.read_text("rust-toolchain.toml") {
        Ok(t) => t,
        Err(e) => return Probe::new(NAME, Verdict::Unknown, format!("the pin is not read: {e}")),
    };
    let Some(channel) = toolchain_channel(&text) else {
        return Probe::new(NAME, Verdict::Unknown, "rust-toolchain.toml names no channel");
    };
    if rustc.verdict != Verdict::Ok {
        return Probe::new(NAME, Verdict::Unknown, format!("pinned {channel}; rustc gave no version"));
    }
    let active = &rustc.evidence;
    match channel_matches(&channel, active) {
        Some(true) => Probe::new(NAME, Verdict::Ok, format!("pinned {channel} is active: {active}")),
        Some(false) => Probe::new(
            NAME,
            Verdict::Missing,
            format!("pinned {channel}, but the active rustc is `{active}`: run `rustup toolchain install {channel}`"),
        ),
        None => Probe::new(
            NAME,
            Verdict::Unknown,
            format!("the pin `{channel}` is not a version; compare it with `{active}`"),
        ),
    }
}

/// The `channel` of the `[toolchain]` table, or the single line of the old
/// format that holds the channel name and nothing else.
pub(crate) fn toolchain_channel(text: &str) -> Option<String> {
    let mut table = String::new();
    let mut plain = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            table = line.to_string();
            continue;
        }
        plain.push(line);
        if table != "[toolchain]" {
            continue;
        }
        let Some(value) = line.strip_prefix("channel").and_then(|r| r.trim_start().strip_prefix('=')) else {
            continue;
        };
        let value = value.trim().trim_matches(|c: char| c == '"' || c == '\'');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    match plain.as_slice() {
        [only] if table.is_empty() && !only.contains('=') => Some((*only).to_string()),
        _ => None,
    }
}

/// Whether a numeric channel (`1.99.0` or `1.99`) names the version in a
/// `rustc --version` line. `None` when the channel is not a number, for
/// example `stable`: then the doctor cannot tell.
pub(crate) fn channel_matches(channel: &str, rustc_line: &str) -> Option<bool> {
    let parts: Vec<&str> = channel.split('.').collect();
    let numeric = parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    if !numeric || !(2..=3).contains(&parts.len()) {
        return None;
    }
    let version = rustc_line.split_whitespace().nth(1)?;
    let core = version.split('-').next().unwrap_or(version);
    Some(if parts.len() == 3 {
        core == channel
    } else {
        core.strip_prefix(channel).is_some_and(|r| r.starts_with('.'))
    })
}

/// `gh api user` succeeds only with a live token. The answer is the login
/// name. gh does not print the token, and neither does this probe.
fn gh_auth_probe(tmp: &Path, gh_runs: bool) -> Probe {
    const NAME: &str = "gh-auth";
    if !gh_runs {
        return Probe::new(NAME, Verdict::Missing, "gh does not answer, so no token is tested");
    }
    match proc::run_in(tmp, "gh", &["api", "user", "--jq", ".login"], NETWORK_LIMIT) {
        Err(e) => Probe::new(NAME, Verdict::Unknown, format!("gh does not start: {e}")),
        Ok(out) if out.timed_out => {
            Probe::new(NAME, Verdict::Unknown, format!("`gh api user` gave no answer in {}s", NETWORK_LIMIT.as_secs()))
        }
        Ok(out) if out.success() => match first_line(&out.stdout) {
            Some(login) => {
                Probe::new(NAME, Verdict::Ok, format!("`gh api user` answers: the token is live, login {login}"))
            }
            None => Probe::new(NAME, Verdict::Unknown, "`gh api user` answered with no login"),
        },
        Ok(out) => {
            let why = first_line(&out.stderr).unwrap_or_else(|| "no message".to_string());
            Probe::new(NAME, gh_failure_verdict(&out.stderr), format!("`gh api user` failed: {why}"))
        }
    }
}

/// An authentication failure is a missing token. Any other failure (no
/// network, a GitHub outage) cannot tell whether the token is live.
pub(crate) fn gh_failure_verdict(stderr: &str) -> Verdict {
    let m = stderr.to_ascii_lowercase();
    let auth = ["http 401", "bad credentials", "gh auth login", "not logged in", "gh_token"];
    if auth.iter().any(|k| m.contains(k)) { Verdict::Missing } else { Verdict::Unknown }
}

/// The first line with text, without control bytes and cut to a length that
/// fits one terminal line. Tool output is data: an escape sequence in it must
/// not reach the terminal.
pub(crate) fn first_line(text: &str) -> Option<String> {
    text.lines().map(str::trim).find(|l| !l.is_empty()).map(clean)
}

pub(crate) fn clean(text: &str) -> String {
    text.chars().take(EVIDENCE_MAX).map(|c| if c.is_control() { '?' } else { c }).collect()
}

fn print_text(probes: &[Probe], notes: &[String]) {
    println!("doctor: what this host can do for development. A probe, not a gate: a missing tool is data.");
    for p in probes {
        println!("  {:8} {:12} {}", p.verdict.word(), p.name, clean(&p.evidence));
    }
    let (ok, missing, unknown) = counts(probes);
    println!("summary: {ok} ok, {missing} missing, {unknown} could not tell (????)");
    for n in notes {
        println!("note: {n}");
    }
}

fn print_json(probes: &[Probe], notes: &[String], ran: bool) {
    let (ok, missing, unknown) = counts(probes);
    let doc = json!({
        "schema": "agent-box-doctor/1",
        "status": if ran { "ran" } else { "not_run" },
        "host": { "os": std::env::consts::OS, "arch": std::env::consts::ARCH },
        "summary": { "ok": ok, "missing": missing, "unknown": unknown },
        "probes": probes.iter().map(|p| json!({
            "name": p.name,
            "status": p.verdict.key(),
            "evidence": p.evidence,
        })).collect::<Vec<_>>(),
        "notes": notes,
    });
    println!("{doc:#}");
}

fn counts(probes: &[Probe]) -> (usize, usize, usize) {
    let n = |v: Verdict| probes.iter().filter(|p| p.verdict == v).count();
    (n(Verdict::Ok), n(Verdict::Missing), n(Verdict::Unknown))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_channel_comes_from_the_toolchain_table() {
        let text = "# a comment\n[toolchain]\nchannel = \"1.99.0\" # pinned\ncomponents = [\"clippy\"]\n";
        assert_eq!(toolchain_channel(text).as_deref(), Some("1.99.0"));
        assert_eq!(toolchain_channel("[other]\nchannel = \"1.0.0\"\n"), None);
        assert_eq!(toolchain_channel("1.98.0\n").as_deref(), Some("1.98.0"));
        assert_eq!(toolchain_channel(""), None);
    }

    #[test]
    fn a_numeric_channel_matches_the_rustc_line() {
        let line = "rustc 1.99.0 (b940084d7 2026-09-28)";
        assert_eq!(channel_matches("1.99.0", line), Some(true));
        assert_eq!(channel_matches("1.99", line), Some(true));
        assert_eq!(channel_matches("1.98.0", line), Some(false));
        assert_eq!(channel_matches("1.9", line), Some(false));
        assert_eq!(channel_matches("stable", line), None);
        assert_eq!(channel_matches("1.99.0", "rustc"), None);
    }

    #[test]
    fn only_an_auth_failure_is_a_missing_token() {
        assert_eq!(gh_failure_verdict("HTTP 401: Bad credentials (https://api.github.com/user)"), Verdict::Missing);
        assert_eq!(gh_failure_verdict("To get started with GitHub CLI, please run:  gh auth login"), Verdict::Missing);
        assert_eq!(gh_failure_verdict("error connecting to api.github.com"), Verdict::Unknown);
    }

    #[test]
    fn evidence_loses_control_bytes_and_length() {
        let esc = char::from(0x1b_u8);
        let planted = format!("\n  {esc}[2Jtitle\tx  \nsecond");
        assert_eq!(first_line(&planted).as_deref(), Some("?[2Jtitle?x"));
        assert_eq!(clean(&"a".repeat(500)).len(), EVIDENCE_MAX);
        assert_eq!(first_line(" \n\t\n"), None);
    }
}
