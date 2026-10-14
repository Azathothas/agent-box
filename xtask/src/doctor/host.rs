//! The probes that only one OS has: on Windows wsl-toolkit, Hyper-V, the
//! Host Compute Service and elevation; on Linux /dev/kvm and a container
//! engine.
//!
//! The defect this module exists to catch: a host capability that is assumed
//! from a file or a feature name. Each probe asks the running system: a query
//! of the hypervisor state, the service state, an open of /dev/kvm.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{Probe, Verdict, first_line, version_probe};
use crate::linux;
use crate::proc;

/// One start of `powershell` answers three questions. It parses an
/// argument list again (podssh measured it on 2026-10-01), so the script
/// travels as `-EncodedCommand`: base64 of its UTF-16LE bytes.
const PS_SCRIPT: &str = "$ErrorActionPreference = 'Stop'
try { 'hyperv=' + (Get-CimInstance -ClassName Win32_ComputerSystem).HypervisorPresent } catch { 'hyperv=?' + $_.Exception.Message }
try { $s = Get-Service -Name vmcompute -ErrorAction SilentlyContinue; if ($null -eq $s) { 'vmcompute=absent' } else { 'vmcompute=' + $s.Status + '/' + $s.StartType } } catch { 'vmcompute=?' + $_.Exception.Message }
try { $p = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent()); 'elevated=' + $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) } catch { 'elevated=?' + $_.Exception.Message }
";

/// `powershell` starts slowly on a cold host, and the CIM query adds to it.
const PS_LIMIT: Duration = Duration::from_secs(60);

/// The first `--kvm` run pulls the image of the Linux runner.
const KVM_LIMIT: Duration = Duration::from_mins(20);

/// The command that the `--kvm` container runs. In a POSIX sh, a failed
/// redirection of `exec` stops the shell, so the mark prints only after a
/// read-write open of /dev/kvm.
///
/// CAUTION: `<>` creates a missing file. MEASURED 2026-10-14 in an alpine
/// container of wsl-toolkit 6.0.0 without `--device /dev/kvm`: the open made a
/// plain file and the mark printed. So the test first requires a character device.
const KVM_TEST: &str = "if ! test -c /dev/kvm; then echo 'kvm-absent: /dev/kvm is not a character device'; exit 1; fi
exec 3<>/dev/kvm && echo kvm-open-rw";
const KVM_MARK: &str = "kvm-open-rw";

pub(super) fn windows(tmp: &Path, kvm: bool) -> Vec<Probe> {
    let wsl_toolkit = version_probe("wsl-toolkit", tmp, "wsl-toolkit", &["version"]);
    let runs = wsl_toolkit.verdict == Verdict::Ok;
    let mut v = vec![wsl_toolkit];
    v.extend(hyperv_probes(tmp));
    if kvm {
        v.push(kvm_through_runner(tmp, runs));
    }
    v
}

pub(super) fn linux(tmp: &Path) -> Vec<Probe> {
    vec![
        kvm_native(),
        engine_probe(tmp, "podman", &["info", "--format", "{{.Version.Version}}"]),
        engine_probe(tmp, "docker", &["version", "--format", "{{.Server.Version}}"]),
    ]
}

/// UTF-16LE then base64: the form that `powershell -EncodedCommand` reads.
pub(crate) fn encoded_command(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    linux::base64(&bytes)
}

fn hyperv_probes(tmp: &Path) -> Vec<Probe> {
    let encoded = encoded_command(PS_SCRIPT);
    let args = ["-NoProfile", "-NonInteractive", "-EncodedCommand", encoded.as_str()];
    let answer = match proc::run_in(tmp, "powershell", &args, PS_LIMIT) {
        Ok(out) if out.success() => Ok(out.stdout),
        Ok(out) if out.timed_out => Err(format!("PowerShell gave no answer in {}s", PS_LIMIT.as_secs())),
        Ok(out) => Err(format!("PowerShell failed: {}", first_line(&out.stderr).unwrap_or_default())),
        Err(e) => Err(format!("PowerShell does not start: {e}")),
    };
    let lines = match answer {
        Ok(text) => text,
        Err(why) => {
            return ["hyper-v", "vmcompute", "elevated"]
                .into_iter()
                .map(|name| Probe::new(name, Verdict::Unknown, why.clone()))
                .collect();
        }
    };
    vec![
        hyperv_verdict(ps_value(&lines, "hyperv")),
        vmcompute_verdict(ps_value(&lines, "vmcompute")),
        elevated_verdict(ps_value(&lines, "elevated")),
    ]
}

/// The value of one `key=value` line of the `powershell` answer.
pub(crate) fn ps_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|l| l.trim().strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

/// A value that starts with `?` carries the message of a failed query.
fn failed_query(name: &'static str, value: Option<&str>) -> Option<Probe> {
    match value {
        None => Some(Probe::new(name, Verdict::Unknown, "PowerShell gave no answer to this query")),
        Some(v) => v
            .strip_prefix('?')
            .map(|why| Probe::new(name, Verdict::Unknown, format!("the query failed: {}", super::clean(why)))),
    }
}

pub(crate) fn hyperv_verdict(value: Option<&str>) -> Probe {
    const NAME: &str = "hyper-v";
    if let Some(p) = failed_query(NAME, value) {
        return p;
    }
    match value.unwrap_or_default() {
        "True" => Probe::new(NAME, Verdict::Ok, "Win32_ComputerSystem.HypervisorPresent is True"),
        "False" => Probe::new(
            NAME,
            Verdict::Missing,
            "Win32_ComputerSystem.HypervisorPresent is False: turn on Hyper-V and restart the host",
        ),
        other => Probe::new(NAME, Verdict::Unknown, format!("HypervisorPresent is `{}`", super::clean(other))),
    }
}

pub(crate) fn vmcompute_verdict(value: Option<&str>) -> Probe {
    const NAME: &str = "vmcompute";
    if let Some(p) = failed_query(NAME, value) {
        return p;
    }
    let value = value.unwrap_or_default();
    let (status, start) = value.split_once('/').unwrap_or((value, ""));
    match (status, start) {
        ("absent", _) => {
            Probe::new(NAME, Verdict::Missing, "no vmcompute service: the Host Compute Service is not installed")
        }
        ("Running", _) => {
            Probe::new(NAME, Verdict::Ok, format!("the vmcompute service is Running (start type {start})"))
        }
        (_, "Disabled") => {
            Probe::new(NAME, Verdict::Missing, format!("the vmcompute service is {status} and Disabled"))
        }
        _ => Probe::new(
            NAME,
            Verdict::Unknown,
            format!("the vmcompute service is {status} (start type {start}); it can start on demand"),
        ),
    }
}

pub(crate) fn elevated_verdict(value: Option<&str>) -> Probe {
    const NAME: &str = "elevated";
    if let Some(p) = failed_query(NAME, value) {
        return p;
    }
    match value.unwrap_or_default() {
        "True" => Probe::new(NAME, Verdict::Ok, "this shell has the Administrator role"),
        "False" => Probe::new(
            NAME,
            Verdict::Missing,
            "this shell is not elevated: the Hyper-V engine tests need an administrator shell",
        ),
        other => Probe::new(NAME, Verdict::Unknown, format!("IsInRole(Administrator) is `{}`", super::clean(other))),
    }
}

/// Open /dev/kvm read-write in a container of the Linux runner, through the
/// same arguments that `cargo xtask linux --kvm` gives wsl-toolkit.
fn kvm_through_runner(tmp: &Path, runs: bool) -> Probe {
    const NAME: &str = "kvm-runner";
    if !runs {
        return Probe::new(NAME, Verdict::Missing, "wsl-toolkit does not answer, so no container tests /dev/kvm");
    }
    eprintln!("xtask doctor: --kvm starts a container; the first run pulls {}", linux::DEFAULT_IMAGE);
    let opts = linux::Options {
        kvm: true,
        image: linux::DEFAULT_IMAGE.to_string(),
        native: false,
        keep: false,
        command: KVM_TEST.to_string(),
    };
    let mut cmd = Command::new("wsl-toolkit");
    cmd.args(linux::wsl_args(&opts, None, "1")).current_dir(tmp);
    match proc::run_command(&mut cmd, KVM_LIMIT) {
        Err(e) => Probe::new(NAME, Verdict::Unknown, format!("wsl-toolkit does not start: {e}")),
        Ok(out) if out.timed_out => {
            Probe::new(NAME, Verdict::Unknown, format!("no answer in {} minutes", KVM_LIMIT.as_secs() / 60))
        }
        Ok(out) if out.success() && out.stdout.contains(KVM_MARK) => {
            Probe::new(NAME, Verdict::Ok, "a container of the Linux runner opened /dev/kvm read-write")
        }
        Ok(out) => {
            let all = format!("{}\n{}", out.stdout, out.stderr);
            let why = all.lines().rev().map(str::trim).find(|l| !l.is_empty()).map(super::clean).unwrap_or_default();
            // The job ran when sh names the device. Else wsl-toolkit stopped
            // before the job, and that says nothing about /dev/kvm.
            let verdict = if all.contains("/dev/kvm") { Verdict::Missing } else { Verdict::Unknown };
            Probe::new(NAME, verdict, format!("the container did not open /dev/kvm read-write: {why}"))
        }
    }
}

fn kvm_native() -> Probe {
    const NAME: &str = "kvm";
    match std::fs::OpenOptions::new().read(true).write(true).open("/dev/kvm") {
        Ok(_) => Probe::new(NAME, Verdict::Ok, "/dev/kvm opens read-write"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Probe::new(NAME, Verdict::Missing, "no /dev/kvm: KVM is off, or this is a VM without nested virtualization")
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Probe::new(
            NAME,
            Verdict::Missing,
            format!("/dev/kvm does not open read-write ({e}): add this user to the kvm group"),
        ),
        Err(e) => Probe::new(NAME, Verdict::Unknown, format!("/dev/kvm: {e}")),
    }
}

/// A container engine answers only when its service or its storage works,
/// so the probe asks the engine itself and not only its client.
fn engine_probe(tmp: &Path, name: &'static str, args: &[&str]) -> Probe {
    let mut p = version_probe(name, tmp, name, args);
    if p.verdict == Verdict::Ok {
        p.evidence = format!("`{name} {}` answers: version {}", args[0], p.evidence);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_is_utf16le_base64() {
        // "a" is 0x61 0x00 in UTF-16LE, and base64 of those bytes is "YQA=".
        assert_eq!(encoded_command("a"), "YQA=");
        assert_eq!(encoded_command("ab"), "YQBiAA==");
        assert!(encoded_command(PS_SCRIPT).is_ascii());
    }

    #[test]
    fn the_kvm_test_needs_a_device_before_the_open() {
        let device = KVM_TEST.find("test -c /dev/kvm");
        let open = KVM_TEST.find("3<>/dev/kvm");
        assert!(device.is_some() && open.is_some() && device < open);
    }

    #[test]
    fn the_powershell_answer_parses_by_key() {
        let out = "hyperv=True\r\nvmcompute=Running/Manual\r\nelevated=False\r\n";
        assert_eq!(ps_value(out, "hyperv"), Some("True"));
        assert_eq!(ps_value(out, "vmcompute"), Some("Running/Manual"));
        assert_eq!(ps_value(out, "elevated"), Some("False"));
        assert_eq!(ps_value(out, "other"), None);
        assert_eq!(hyperv_verdict(ps_value(out, "hyperv")).verdict, Verdict::Ok);
        assert_eq!(vmcompute_verdict(ps_value(out, "vmcompute")).verdict, Verdict::Ok);
        assert_eq!(elevated_verdict(ps_value(out, "elevated")).verdict, Verdict::Missing);
    }

    #[test]
    fn each_state_has_one_verdict() {
        assert_eq!(hyperv_verdict(Some("False")).verdict, Verdict::Missing);
        assert_eq!(hyperv_verdict(Some("?Access denied")).verdict, Verdict::Unknown);
        assert_eq!(hyperv_verdict(None).verdict, Verdict::Unknown);
        assert_eq!(vmcompute_verdict(Some("absent")).verdict, Verdict::Missing);
        assert_eq!(vmcompute_verdict(Some("Stopped/Disabled")).verdict, Verdict::Missing);
        assert_eq!(vmcompute_verdict(Some("Stopped/Manual")).verdict, Verdict::Unknown);
        assert_eq!(elevated_verdict(Some("True")).verdict, Verdict::Ok);
    }
}
