//! `cargo xtask linux [--kvm] [--image IMAGE] [--native] [--keep] -- COMMAND...`
//!
//! Runs COMMAND in a Linux container, with the repository as the working
//! directory. On Windows the container comes from wsl-toolkit. On Linux it
//! comes from podman or docker, and `--native` runs the command on the host.
//!
//! The defect this module exists to catch: a command that a shell changes on
//! its way from Windows into Linux. The sibling repository podssh measured
//! three such changes on 2026-10-01 (podssh scripts/dev-wsl.sh). Git Bash
//! rewrites an argument that starts with `/` into a Windows path. `powershell.exe`
//! parses an argument list again and splits it. A shell expands an exclude
//! pattern such as `target/**` into every file under target/. Here no shell is
//! between this program and wsl-toolkit: `std::process::Command` gives each
//! argument as one word, and the command travels as base64 of its bytes.

mod args;

use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

pub(crate) use args::{DEFAULT_IMAGE, Options, base64, wsl_args};
use args::{Engine, WORKDIR, engine_args, jobs_from, parse, workspace_text};

use crate::proc;
use crate::repo::Repo;
use crate::report;

const USAGE: &str = "usage: cargo xtask linux [--kvm] [--image IMAGE] [--native] [--keep] -- COMMAND...

Runs COMMAND in a Linux container with the repository at /work as the working
directory, and exits with the exit code of COMMAND. The words of COMMAND are
joined with spaces and sh reads the result, so put a compound command in one
argument: cargo xtask linux -- 'cargo build && cargo test'.

  --kvm          pass /dev/kvm into the container
  --image IMAGE  the container image (default: the pinned Rust image)
  --native       Linux only: run COMMAND with sh -c on the host, in the repository
  --keep         keep the stopped container (and its job directory on Windows)

On Windows the container comes from wsl-toolkit, which copies the repository
without target/, .git/, .refs/, .work/ and .claude/. Changes in the container
do not come back. On Linux the repository is mounted read-write, and cargo
writes to target/linux-container. CARGO_BUILD_JOBS (default 4) is passed in.";

pub(crate) fn run(args: &[String]) -> u8 {
    if args.first().is_some_and(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return report::EXIT_PASS;
    }
    let opts = match parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("xtask linux: {e}\n\n{USAGE}");
            return report::EXIT_USAGE;
        }
    };
    let jobs = match jobs_from(std::env::var("CARGO_BUILD_JOBS").ok().as_deref()) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("xtask linux: {e}");
            return report::EXIT_USAGE;
        }
    };
    let repo = match Repo::find() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("xtask linux: {e}");
            return report::EXIT_NOT_RUN;
        }
    };
    if cfg!(windows) {
        if opts.native {
            eprintln!(
                "xtask linux: --native runs on a Linux host only; on Windows the command runs through wsl-toolkit"
            );
            return report::EXIT_USAGE;
        }
        run_wsl(&opts, &repo.root, &jobs)
    } else if opts.native {
        run_native(&opts, &repo.root, &jobs)
    } else {
        run_engine(&opts, &repo.root, &jobs)
    }
}

fn run_wsl(opts: &Options, root: &Path, jobs: &str) -> u8 {
    let workspace = match workspace_text(root) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("xtask linux: {e}");
            return report::EXIT_NOT_RUN;
        }
    };
    let args = wsl_args(opts, Some(&workspace), jobs);
    eprintln!(
        "xtask linux: wsl-toolkit run --image {} --workspace {workspace}{}, command: {}",
        opts.image,
        if opts.kvm { " --device /dev/kvm" } else { "" },
        opts.command
    );
    let mut cmd = Command::new("wsl-toolkit");
    cmd.args(&args);
    stream(
        &mut cmd,
        "wsl-toolkit",
        "install wsl-toolkit and put it on PATH; `cargo xtask doctor` shows what is missing",
    )
}

fn run_engine(opts: &Options, root: &Path, jobs: &str) -> u8 {
    let engine = if proc::probe_version("podman", &["--version"]).is_some() {
        Engine::Podman
    } else if proc::probe_version("docker", &["--version"]).is_some() {
        Engine::Docker
    } else {
        eprintln!("xtask linux: neither podman nor docker answers `--version`; install one, or use --native");
        return report::EXIT_NOT_RUN;
    };
    let Some(root_text) = root.to_str() else {
        eprintln!("xtask linux: {} is not valid UTF-8", root.display());
        return report::EXIT_NOT_RUN;
    };
    let tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let args = match engine_args(engine, opts, root_text, jobs, tty) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("xtask linux: {e}");
            return report::EXIT_NOT_RUN;
        }
    };
    eprintln!(
        "xtask linux: {} run {} with {root_text} at {WORKDIR}, command: {}",
        engine.program(),
        opts.image,
        opts.command
    );
    if engine == Engine::Docker {
        eprintln!("xtask linux: CAUTION: docker runs as root, so files that it writes to target/ belong to root");
    }
    let mut cmd = Command::new(engine.program());
    cmd.args(&args);
    stream(&mut cmd, engine.program(), "install podman or docker, or use --native")
}

fn run_native(opts: &Options, root: &Path, jobs: &str) -> u8 {
    if opts.kvm
        && let Err(e) = std::fs::OpenOptions::new().read(true).write(true).open("/dev/kvm")
    {
        eprintln!("xtask linux: --kvm: /dev/kvm does not open read-write: {e}");
        return report::EXIT_NOT_RUN;
    }
    eprintln!("xtask linux: sh -c on this host in {}, command: {}", root.display(), opts.command);
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&opts.command).current_dir(root).env("CARGO_BUILD_JOBS", jobs);
    stream(&mut cmd, "sh", "install a POSIX sh")
}

/// Run with the terminal attached and return the exit code of the program.
/// A program that does not start is "could not run" (2), with a hint.
fn stream(cmd: &mut Command, name: &str, hint: &str) -> u8 {
    match cmd.status() {
        Ok(status) => {
            if let Some(code) = status.code() {
                exit_byte(code)
            } else {
                eprintln!("xtask linux: {name} stopped with no exit code (a signal stopped it)");
                report::EXIT_FAIL
            }
        }
        Err(e) => {
            eprintln!("xtask linux: could not start {name}: {e}; {hint}");
            report::EXIT_NOT_RUN
        }
    }
}

/// An exit code as a process exit byte. A code outside 0 to 255 (a Windows
/// status such as `0xC0000005`) is a failure, never a pass.
pub(crate) fn exit_byte(code: i32) -> u8 {
    u8::try_from(code).unwrap_or(report::EXIT_FAIL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_map_to_bytes_and_a_wide_code_is_a_failure() {
        assert_eq!(exit_byte(0), 0);
        assert_eq!(exit_byte(124), 124);
        assert_eq!(exit_byte(-1_073_741_819), report::EXIT_FAIL);
        assert_eq!(exit_byte(256), report::EXIT_FAIL);
    }
}
