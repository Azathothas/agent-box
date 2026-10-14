//! Run another program with a time limit, and probe a tool by running it.
//!
//! Every program can wait forever, so every call has a limit. A timeout is a
//! different fact from a failure and is reported as such. Presence on PATH is
//! not capability: a probe runs the tool and reads its answer.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// For a call that answers at once (git ls-files, a version query).
pub(crate) const QUICK: Duration = Duration::from_secs(60);
/// For a build or a test run.
pub(crate) const LONG: Duration = Duration::from_hours(1);

/// What a program did.
#[derive(Debug, Clone)]
pub(crate) struct Output {
    /// The exit code. `None` when the program was killed or timed out.
    pub(crate) code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) timed_out: bool,
}

impl Output {
    pub(crate) fn success(&self) -> bool {
        self.code == Some(0) && !self.timed_out
    }
}

/// Run `program args` in `dir`, capture both streams, and stop it after `limit`.
/// An error means the program could not start (for example it is not installed).
pub(crate) fn run_in(dir: &Path, program: &str, args: &[&str], limit: Duration) -> Result<Output, String> {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(dir);
    run_command(&mut cmd, limit).map_err(|e| format!("{program}: {e}"))
}

/// Run a prepared command with captured output and a time limit.
pub(crate) fn run_command(cmd: &mut Command, limit: Duration) -> Result<Output, String> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    // Read both pipes on threads, or a full pipe stops the child and the wait never ends.
    let mut out_pipe = child.stdout.take().ok_or("no stdout pipe")?;
    let mut err_pipe = child.stderr.take().ok_or("no stderr pipe")?;
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err_pipe.read_to_end(&mut buf);
        buf
    });
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break Some(status);
        }
        if start.elapsed() >= limit {
            timed_out = true;
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = String::from_utf8_lossy(&out_reader.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err_reader.join().unwrap_or_default()).into_owned();
    Ok(Output { code: status.and_then(|s| s.code()), stdout, stderr, timed_out })
}

/// Run a command with the terminal attached (for the gate's cargo steps, so
/// the developer sees the compiler output as it comes). Returns the exit code,
/// or `None` when the program could not start or was killed by a signal.
pub(crate) fn run_attached(cmd: &mut Command) -> Option<i32> {
    match cmd.status() {
        Ok(status) => status.code(),
        Err(e) => {
            eprintln!("xtask: could not start {}: {e}", cmd.get_program().display());
            None
        }
    }
}

/// Ask a tool for its version by running it. `None` when it is not installed,
/// does not answer within the limit, or exits non-zero.
pub(crate) fn probe_version(program: &str, args: &[&str]) -> Option<String> {
    let dir = std::env::temp_dir();
    let out = run_in(&dir, program, args, Duration::from_secs(20)).ok()?;
    if !out.success() {
        return None;
    }
    let text = if out.stdout.trim().is_empty() { out.stderr } else { out.stdout };
    text.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_string)
}
