//! `cargo xtask setup`: prepare this checkout for development.
//!
//! The defect this command exists to catch: a checkout that looks ready and is
//! not. git does not clone hooks, so a fresh clone commits without the commit
//! hook and nothing says so. A hook without its executable mode does not run
//! on Linux, and git says nothing about that either. A gate without cargo-deny
//! cannot run its last step.
//!
//! Setup arms the hook, sets its mode, adds the musl targets of the guest
//! agent and installs cargo-deny. Then it prints each item with its state. It
//! exits 0 only when every required item is present.

use std::process::Command;

use crate::proc;
use crate::repo::Repo;
use crate::report;

const USAGE: &str = "usage: cargo xtask setup [--no-install]

  Arms the commit hook (core.hooksPath), sets its mode to 100755 in the index,
  adds the musl targets of box-guest, and installs cargo-deny. On Windows it
  also asks wsl-toolkit for its version.

  --no-install  do not download or build anything; report what is missing

Exit codes: 0 every required item is present, 1 an item is missing, 2 could not
run, 64 usage error.";

/// The directory of the hooks, relative to the repository root.
const HOOKS_DIR: &str = ".githooks";
/// The commit hook, relative to the repository root.
const HOOK: &str = ".githooks/commit-msg";
/// The index mode of an executable file. git checks out a file with this mode
/// as executable on Linux, and git runs a hook only when it is executable.
const EXEC_MODE: &str = "100755";
/// The targets of box-guest, the static musl binary in the guest.
const MUSL_TARGETS: [&str; 2] = ["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"];

/// The state of one item after setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// The item was correct before setup ran.
    Present,
    /// Setup made the item correct now.
    Done,
    /// The item is not correct, and setup did not make it correct.
    Missing,
}

impl State {
    fn word(self) -> &'static str {
        match self {
            State::Present => "present",
            State::Done => "done",
            State::Missing => "MISSING",
        }
    }
}

/// One item that setup checks or makes.
#[derive(Debug)]
struct Item {
    name: &'static str,
    /// A missing required item makes setup exit 1. An optional item is a note.
    required: bool,
    state: State,
    detail: String,
}

impl Item {
    fn new(name: &'static str, state: State, detail: impl Into<String>) -> Self {
        Self { name, required: true, state, detail: detail.into() }
    }

    fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

/// `cargo xtask setup [--no-install]`
pub(crate) fn run(args: &[String]) -> u8 {
    let mut install = true;
    for arg in args {
        match arg.as_str() {
            "--no-install" => install = false,
            "-h" | "--help" => {
                println!("{USAGE}");
                return report::EXIT_PASS;
            }
            other => {
                eprintln!("xtask setup: unknown argument `{other}`\n\n{USAGE}");
                return report::EXIT_USAGE;
            }
        }
    }
    let repo = match Repo::find() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("xtask setup: {e}");
            return report::EXIT_NOT_RUN;
        }
    };
    let mut items = vec![hooks_path(&repo), hook_index_mode(&repo)];
    #[cfg(unix)]
    items.push(hook_file_mode(&repo));
    items.push(musl_targets(&repo, install));
    items.push(cargo_deny(&repo, install));
    if cfg!(windows) {
        items.push(wsl_toolkit());
    }
    print_items(&items);
    exit_code(&items)
}

/// Step 1: point git at the hooks of the repository.
fn hooks_path(repo: &Repo) -> Item {
    const NAME: &str = "commit hook armed";
    if !repo.path(HOOK).is_file() {
        return Item::new(NAME, State::Missing, format!("{HOOK} is not in the working tree"));
    }
    let before = match git(repo, &["config", "--get", "core.hooksPath"]) {
        Ok(out) => out.stdout.trim().to_string(),
        Err(e) => return Item::new(NAME, State::Missing, e),
    };
    if before == HOOKS_DIR {
        return Item::new(NAME, State::Present, format!("core.hooksPath is {HOOKS_DIR}"));
    }
    match git(repo, &["config", "core.hooksPath", HOOKS_DIR]) {
        Ok(out) if out.success() && before.is_empty() => {
            Item::new(NAME, State::Done, format!("set core.hooksPath to {HOOKS_DIR}"))
        }
        Ok(out) if out.success() => {
            Item::new(NAME, State::Done, format!("changed core.hooksPath from {before} to {HOOKS_DIR}"))
        }
        Ok(out) => Item::new(NAME, State::Missing, format!("git config failed: {}", first_line(&out.stderr))),
        Err(e) => Item::new(NAME, State::Missing, e),
    }
}

/// Step 2: the hook is executable in the index, so each clone gets it
/// executable.
fn hook_index_mode(repo: &Repo) -> Item {
    const NAME: &str = "hook mode in the index";
    let out = match git(repo, &["ls-files", "--stage", "--", HOOK]) {
        Ok(out) if out.success() => out,
        Ok(out) => return Item::new(NAME, State::Missing, format!("git ls-files failed: {}", first_line(&out.stderr))),
        Err(e) => return Item::new(NAME, State::Missing, e),
    };
    let Some(mode) = index_mode(&out.stdout) else {
        return Item::new(NAME, State::Missing, format!("git does not track {HOOK}"));
    };
    if mode == EXEC_MODE {
        return Item::new(NAME, State::Present, format!("{HOOK} has mode {EXEC_MODE}"));
    }
    match git(repo, &["update-index", "--chmod=+x", "--", HOOK]) {
        Ok(out) if out.success() => Item::new(
            NAME,
            State::Done,
            format!(
                "the index mode of {HOOK} was {mode}; ran `git update-index --chmod=+x {HOOK}`; commit this change"
            ),
        ),
        Ok(out) => Item::new(NAME, State::Missing, format!("git update-index failed: {}", first_line(&out.stderr))),
        Err(e) => Item::new(NAME, State::Missing, e),
    }
}

/// The mode in the first line of `git ls-files --stage`, for example
/// `100755 <object> 0<TAB>path`. `None` when git does not track the file.
fn index_mode(stage: &str) -> Option<&str> {
    stage.lines().next()?.split_whitespace().next()
}

/// Step 2, Unix only: the hook in the working tree is executable. The index
/// mode does not change the file that is already checked out.
#[cfg(unix)]
fn hook_file_mode(repo: &Repo) -> Item {
    use std::os::unix::fs::PermissionsExt;
    const NAME: &str = "hook file executable";
    let path = repo.path(HOOK);
    let mode = match std::fs::metadata(&path) {
        Ok(m) => m.permissions().mode(),
        Err(e) => return Item::new(NAME, State::Missing, format!("{HOOK}: {e}")),
    };
    if (mode & 0o111) == 0o111 {
        return Item::new(NAME, State::Present, format!("{HOOK} has the executable bits"));
    }
    match std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode | 0o111)) {
        Ok(()) => Item::new(NAME, State::Done, format!("set the executable bits of {HOOK}")),
        Err(e) => Item::new(NAME, State::Missing, format!("{HOOK}: {e}")),
    }
}

/// Step 3: the musl targets of the pinned toolchain.
fn musl_targets(repo: &Repo, install: bool) -> Item {
    const NAME: &str = "musl targets (box-guest)";
    let mut list = rustup(repo);
    list.args(["target", "list", "--installed"]);
    let installed = match proc::run_command(&mut list, proc::QUICK) {
        Ok(out) if out.success() => out.stdout,
        Ok(out) => {
            return Item::new(NAME, State::Missing, format!("rustup target list failed: {}", first_line(&out.stderr)));
        }
        Err(e) => return Item::new(NAME, State::Missing, format!("rustup did not start ({e}); https://rustup.rs")),
    };
    let absent = missing_targets(&installed);
    if absent.is_empty() {
        return Item::new(NAME, State::Present, MUSL_TARGETS.join(", "));
    }
    let shown = format!("rustup target add {}", absent.join(" "));
    if !install {
        return Item::new(NAME, State::Missing, format!("not installed: {}; run `{shown}`", absent.join(", ")));
    }
    println!("setup: {shown}");
    let mut add = rustup(repo);
    add.args(["target", "add"]).args(absent);
    match proc::run_attached(&mut add) {
        Some(0) => Item::new(NAME, State::Done, format!("ran `{shown}`")),
        Some(code) => Item::new(NAME, State::Missing, format!("`{shown}` exited with code {code}")),
        None => Item::new(NAME, State::Missing, format!("`{shown}` did not start, or a signal stopped it")),
    }
}

/// A rustup command in the repository root, so rust-toolchain.toml selects
/// the toolchain. `RUSTUP_TOOLCHAIN` is removed: a `cargo +other xtask` call
/// sets it, and the targets belong to the pinned toolchain.
fn rustup(repo: &Repo) -> Command {
    let mut cmd = Command::new("rustup");
    cmd.current_dir(&repo.root).env_remove("RUSTUP_TOOLCHAIN");
    cmd
}

/// The musl targets that are not in the output of `rustup target list --installed`.
fn missing_targets(installed: &str) -> Vec<&'static str> {
    MUSL_TARGETS.iter().copied().filter(|t| !installed.lines().any(|l| l.trim() == *t)).collect()
}

/// The cargo-deny version that CI installs (.github/workflows/ci.yml). A
/// developer uses the same version, so the step `deny` gives the same answer.
const CARGO_DENY_VERSION: &str = "0.20.2";

/// Step 4: cargo-deny at the pinned version, for the last step of the gate.
fn cargo_deny(repo: &Repo, install: bool) -> Item {
    const NAME: &str = "cargo-deny";
    let shown = format!("cargo install --locked cargo-deny@{CARGO_DENY_VERSION}");
    let found = proc::probe_version("cargo-deny", &["--version"]);
    if let Some(version) = &found
        && version_matches(version)
    {
        return Item::new(NAME, State::Present, version.clone());
    }
    let why = match &found {
        Some(version) => format!("{version} is not the pinned {CARGO_DENY_VERSION}"),
        None => "not installed".to_string(),
    };
    if !install {
        return Item::new(NAME, State::Missing, format!("{why}; run `{shown}`"));
    }
    println!("setup: {shown} ({why}; this builds it from source, and takes some minutes)");
    let mut cmd = Command::new("cargo");
    cmd.args(["install", "--locked", "--force", &format!("cargo-deny@{CARGO_DENY_VERSION}")]).current_dir(&repo.root);
    if std::env::var_os("CARGO_BUILD_JOBS").is_none() {
        cmd.env("CARGO_BUILD_JOBS", "4");
    }
    match proc::run_attached(&mut cmd) {
        Some(0) => match proc::probe_version("cargo-deny", &["--version"]) {
            Some(version) if version_matches(&version) => Item::new(NAME, State::Done, format!("installed {version}")),
            Some(version) => Item::new(NAME, State::Missing, format!("installed, but PATH finds {version} first")),
            None => {
                Item::new(NAME, State::Missing, "installed, but cargo-deny does not answer; is ~/.cargo/bin on PATH?")
            }
        },
        Some(code) => Item::new(NAME, State::Missing, format!("`{shown}` exited with code {code}")),
        None => Item::new(NAME, State::Missing, format!("`{shown}` did not start, or a signal stopped it")),
    }
}

/// True when `cargo-deny --version` names the pinned version as a whole word.
fn version_matches(answer: &str) -> bool {
    answer.split_whitespace().any(|w| w == CARGO_DENY_VERSION)
}

/// Step 5, Windows only: wsl-toolkit runs the Linux tests from Windows
/// (`cargo xtask linux`). The gate does not need it, so it is optional.
fn wsl_toolkit() -> Item {
    const NAME: &str = "wsl-toolkit";
    match proc::probe_version("wsl-toolkit", &["version"]) {
        Some(version) => Item::new(NAME, State::Present, version).optional(),
        None => Item::new(NAME, State::Missing, "does not answer `wsl-toolkit version`; `cargo xtask linux` needs it")
            .optional(),
    }
}

/// Step 6: what setup did, and what is missing.
fn print_items(items: &[Item]) {
    println!();
    println!("setup summary");
    for i in items {
        let kind = if i.required { "" } else { " (optional)" };
        println!("  {:8} {}{kind}: {}", i.state.word(), i.name, i.detail);
    }
    let missing: Vec<&str> = items.iter().filter(|i| i.required && i.state == State::Missing).map(|i| i.name).collect();
    if missing.is_empty() {
        println!("setup: ready. Every required item is present.");
    } else {
        println!("setup: NOT READY. Missing: {}.", missing.join(", "));
    }
}

fn exit_code(items: &[Item]) -> u8 {
    if items.iter().any(|i| i.required && i.state == State::Missing) { report::EXIT_FAIL } else { report::EXIT_PASS }
}

/// Run git in the repository root. `Err` when git does not start.
fn git(repo: &Repo, args: &[&str]) -> Result<proc::Output, String> {
    proc::run_in(&repo.root, "git", args, proc::QUICK).map_err(|e| format!("git did not start: {e}"))
}

fn first_line(text: &str) -> &str {
    text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("no message")
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_the_pinned_cargo_deny_version_matches() {
        assert!(super::version_matches("cargo-deny 0.20.2"));
        assert!(!super::version_matches("cargo-deny 0.20.21"));
        assert!(!super::version_matches("cargo-deny 0.19.0"));
    }

    use super::*;

    #[test]
    fn the_index_mode_is_the_first_field() {
        let line = format!("100644 {} 0\t{HOOK}\n", "0".repeat(40));
        assert_eq!(index_mode(&line), Some("100644"));
        assert_eq!(index_mode(""), None);
    }

    #[test]
    fn a_target_on_its_own_line_is_installed() {
        let all = "x86_64-pc-windows-msvc\nx86_64-unknown-linux-musl\naarch64-unknown-linux-musl\n";
        assert_eq!(missing_targets(all), Vec::<&str>::new());
        assert_eq!(missing_targets("x86_64-unknown-linux-musl\r\n"), vec!["aarch64-unknown-linux-musl"]);
        assert_eq!(missing_targets(""), MUSL_TARGETS.to_vec());
    }

    #[test]
    fn a_missing_required_item_fails_and_an_optional_one_does_not() {
        let ok = Item::new("a", State::Present, "");
        let done = Item::new("b", State::Done, "");
        let opt = Item::new("c", State::Missing, "").optional();
        assert_eq!(exit_code(&[ok, done, opt]), report::EXIT_PASS);
        assert_eq!(exit_code(&[Item::new("d", State::Missing, "")]), report::EXIT_FAIL);
    }
}
