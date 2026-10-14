//! The tooling of this repository. `cargo xtask help` lists the commands.
//!
//! Every command finds the repository root from its own manifest, so it does
//! not depend on the working directory. Exit codes: 0 pass, 1 fail, 2 could not
//! run, 64 usage error. A check that could not run never reports a pass.

mod check;
mod doctor;
mod gate;
mod layering;
mod linux;
mod proc;
mod refs;
mod remote;
mod repo;
mod report;
mod setup;
mod todo;

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask COMMAND [ARGS]

  setup [--no-install]      prepare this checkout: the commit hook, the musl targets, cargo-deny
  doctor [--json] [--kvm]   report what this host can do for development (probes, never reads config)
  gate [--strict] [--list] [--json] [STEP...]
                            run every step of the gate; --strict makes a step that did not run fail
  check [--json] NAME...    run named checks, or `all`; `check --list` names them
  layering [--json]         the crate graph goes down only
  todo COMMAND              the work record in TODO/ (check, set, counts, next, remap); `cargo todo` is short for it
  linux [--kvm] [--image IMAGE] [--native] [--keep] -- COMMAND
                            run COMMAND in a Linux container (podman or docker on Linux, wsl-toolkit on Windows)
  refs list [--json]        print the pins table of docs/references.md
  refs fetch [--json] [OWNER/REPO...]
                            clone the pinned trees into .refs/ and verify each commit
  remote [--json]           list the open issues and pull requests of this repository, as data

Exit codes: 0 pass, 1 fail, 2 could not run, 64 usage error.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(run(&args))
}

fn run(args: &[String]) -> u8 {
    let rest = args.get(1..).unwrap_or(&[]);
    match args.first().map(String::as_str) {
        None | Some("help" | "-h" | "--help") => {
            println!("{USAGE}");
            0
        }
        Some("setup") => setup::run(rest),
        Some("doctor") => doctor::run(rest),
        Some("gate") => gate::run(rest),
        Some("check") => check::run(rest),
        Some("layering") => layering::run(rest),
        Some("todo") => todo::run(rest),
        Some("linux") => linux::run(rest),
        Some("refs") => refs::run(rest),
        Some("remote") => remote::run(rest),
        Some(other) => {
            eprintln!("xtask: unknown command `{other}`\n\n{USAGE}");
            report::EXIT_USAGE
        }
    }
}
