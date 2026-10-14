//! The pure part of `cargo xtask linux`: the command line, and the arguments
//! of `wsl-toolkit run`, `podman run` and `docker run`. Nothing here starts a
//! program, so the unit tests check every argument without a container.
//!
//! The defect this module exists to catch: an argument that arrives changed.
//! Each exclude pattern is one flag and one word that no shell expands, and
//! the command travels as base64 of its bytes.

use std::path::Path;

/// The default image: Debian trixie with rustup and Rust 1.99.0, the channel
/// of rust-toolchain.toml. MEASURED 2026-10-14 with a HEAD request to
/// registry-1.docker.io: the tags 1.99-trixie and 1.99.0-trixie both resolve
/// to this OCI index digest, which has an amd64 and an arm64 image.
pub(crate) const DEFAULT_IMAGE: &str =
    "docker.io/library/rust:1.99-trixie@sha256:81015672d6939b0265f09adf96d5de125482bb1330106076956b0518e74edc90";

/// What wsl-toolkit leaves out of its copy of the workspace, one flag per
/// pattern. Build output is not source. A Windows target/ alone can pass the
/// workspace size limit of wsl-toolkit, 1073741824 bytes (podssh measured
/// 848 MB on 2026-10-01).
///
/// MEASURED 2026-10-14, wsl-toolkit 6.0.0 on Windows 11: `--exclude target`
/// leaves out the directory and all of its contents. `--exclude target/**`
/// leaves out the contents only, so an empty target/ and .git/ were in /work.
const EXCLUDES: [&str; 5] = ["target", ".git", ".refs", ".work", ".claude"];

/// Where the repository is in the container.
pub(crate) const WORKDIR: &str = "/work";

/// The time limit of one wsl-toolkit container. A timeout reports 124.
const WSL_TIMEOUT: &str = "60m";

/// The same limit for podman and docker, in seconds. docker has no flag for
/// it, so the docker command line runs the command under coreutils `timeout`.
const PODMAN_TIMEOUT_SECS: &str = "3600";

/// Cargo jobs in the container when `CARGO_BUILD_JOBS` is not set. On
/// 2026-10-07 three builds with no limit nearly used all the memory of the
/// podssh developer host through WSL.
const DEFAULT_JOBS: &str = "4";

/// The exit code of the container script when the workspace is not at /work.
const EXIT_NO_WORKDIR: u8 = 71;

/// The parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Options {
    pub(crate) kvm: bool,
    pub(crate) image: String,
    pub(crate) native: bool,
    pub(crate) keep: bool,
    /// The words of COMMAND, joined with spaces: one sh command line.
    pub(crate) command: String,
}

/// The two container engines of a Linux host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Engine {
    Podman,
    Docker,
}

impl Engine {
    pub(crate) fn program(self) -> &'static str {
        match self {
            Engine::Podman => "podman",
            Engine::Docker => "docker",
        }
    }
}

/// Parse the arguments. The first word that is not an option starts COMMAND,
/// and so does the word after `--`. Every word after that belongs to COMMAND.
pub(crate) fn parse(args: &[String]) -> Result<Options, String> {
    let mut opts =
        Options { kvm: false, image: DEFAULT_IMAGE.to_string(), native: false, keep: false, command: String::new() };
    let mut image_given = false;
    let mut words: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--" => {
                words.extend(it.by_ref().cloned());
                break;
            }
            "--kvm" => opts.kvm = true,
            "--native" => opts.native = true,
            "--keep" => opts.keep = true,
            "--image" => {
                let Some(value) = it.next() else {
                    return Err("--image needs a value".to_string());
                };
                opts.image = image_value(value)?;
                image_given = true;
            }
            other => {
                if let Some(value) = other.strip_prefix("--image=") {
                    opts.image = image_value(value)?;
                    image_given = true;
                } else if other.starts_with('-') {
                    return Err(format!("unknown option `{other}`; put COMMAND after `--`"));
                } else {
                    words.push(other.to_string());
                    words.extend(it.by_ref().cloned());
                    break;
                }
            }
        }
    }
    if words.iter().all(|w| w.trim().is_empty()) {
        return Err("name a command after `--`".to_string());
    }
    if opts.native && (image_given || opts.keep) {
        return Err("--native runs on the host, so --image and --keep have no meaning with it".to_string());
    }
    opts.command = words.join(" ");
    Ok(opts)
}

fn image_value(value: &str) -> Result<String, String> {
    if value.is_empty() || value.starts_with('-') || value.chars().any(char::is_whitespace) {
        return Err(format!("`{value}` is not an image reference"));
    }
    Ok(value.to_string())
}

/// The number of cargo jobs: `CARGO_BUILD_JOBS` when it is set, else 4.
pub(crate) fn jobs_from(value: Option<&str>) -> Result<String, String> {
    let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(DEFAULT_JOBS.to_string());
    };
    match v.parse::<u16>() {
        Ok(n) if n > 0 => Ok(n.to_string()),
        _ => Err(format!("CARGO_BUILD_JOBS is `{v}`; give a positive whole number")),
    }
}

/// The script that the container runs: go to the workspace, then COMMAND.
/// The exit code of the script is the exit code of COMMAND.
pub(crate) fn container_script(command: &str) -> String {
    format!("cd {WORKDIR} || exit {EXIT_NO_WORKDIR}\n{command}\n")
}

/// The arguments of `wsl-toolkit run`. It runs nothing. With a workspace, the
/// repository is copied in without EXCLUDES and the script starts in /work.
/// The flag names are the ones that wsl-toolkit 6.0.0 prints for
/// `wsl-toolkit run --help`, read 2026-10-14.
pub(crate) fn wsl_args(opts: &Options, workspace: Option<&str>, jobs: &str) -> Vec<String> {
    let mut a: Vec<String> = vec!["run".into(), "--image".into(), opts.image.clone()];
    let script = match workspace {
        Some(dir) => {
            a.push("--workspace".into());
            a.push(dir.into());
            for pattern in EXCLUDES {
                a.push("--exclude".into());
                a.push(pattern.into());
            }
            container_script(&opts.command)
        }
        None => format!("{}\n", opts.command),
    };
    if opts.kvm {
        a.push("--device".into());
        a.push("/dev/kvm".into());
    }
    a.push("--env".into());
    a.push(format!("CARGO_BUILD_JOBS={jobs}"));
    a.push("--container-lifecycle".into());
    a.push(if opts.keep { "persistent" } else { "ephemeral" }.into());
    a.push("--timeout".into());
    a.push(WSL_TIMEOUT.into());
    a.push("--command-base64".into());
    a.push(base64(script.as_bytes()));
    a
}

/// The arguments of `podman run` or `docker run`. It runs nothing.
pub(crate) fn engine_args(
    engine: Engine,
    opts: &Options,
    root: &str,
    jobs: &str,
    tty: bool,
) -> Result<Vec<String>, String> {
    // The --volume value separates its fields with `:`, so a path with `:` in
    // it would mount the wrong directory.
    if root.contains(':') {
        return Err(format!("the repository path {root} contains `:`, which --volume cannot carry"));
    }
    let mut a: Vec<String> = vec!["run".into()];
    if !opts.keep {
        a.push("--rm".into());
    }
    if tty {
        a.push("--interactive".into());
        a.push("--tty".into());
    }
    // label=disable lets the container read the mount on an SELinux host
    // without a relabel (`:Z`), which would change the labels of the host tree.
    a.extend(["--security-opt".into(), "label=disable".into()]);
    a.extend(["--volume".into(), format!("{root}:{WORKDIR}")]);
    a.extend(["--workdir".into(), WORKDIR.into()]);
    a.extend(["--env".into(), format!("CARGO_BUILD_JOBS={jobs}")]);
    // The host build uses target/ with the host toolchain. A separate
    // directory stops the two builds from invalidating each other.
    a.extend(["--env".into(), format!("CARGO_TARGET_DIR={WORKDIR}/target/linux-container")]);
    if engine == Engine::Podman {
        a.extend(["--timeout".into(), PODMAN_TIMEOUT_SECS.into()]);
    }
    if opts.kvm {
        a.extend(["--device".into(), "/dev/kvm".into()]);
        // Rootless podman drops the supplementary groups of the user, and
        // /dev/kvm is usually open to the kvm group only.
        if engine == Engine::Podman {
            a.extend(["--group-add".into(), "keep-groups".into()]);
        }
    }
    a.push(opts.image.clone());
    if engine == Engine::Docker {
        // docker run has no time limit of its own, so coreutils `timeout` in
        // the image holds the same limit. A timeout exits 124.
        a.extend(["timeout".into(), PODMAN_TIMEOUT_SECS.into()]);
    }
    a.extend(["sh".into(), "-c".into(), opts.command.clone()]);
    Ok(a)
}

/// The workspace path as wsl-toolkit reads it: a plain Windows path, without
/// the `\\?\` prefix of a verbatim path.
pub(crate) fn workspace_text(root: &Path) -> Result<String, String> {
    let text = root.to_str().ok_or_else(|| format!("{} is not valid UTF-8", root.display()))?;
    Ok(text.strip_prefix(r"\\?\").unwrap_or(text).to_string())
}

/// Standard base64 (RFC 4648, with padding) of some bytes.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let sym = |i: u8| char::from(TABLE[usize::from(i & 0x3f)]);
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(sym(b0 >> 2));
        out.push(sym((b0 << 4) | (b1 >> 4)));
        out.push(if chunk.len() > 1 { sym((b1 << 2) | (b2 >> 6)) } else { '=' });
        out.push(if chunk.len() > 2 { sym(b2) } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    fn opts(command: &str) -> Options {
        Options { kvm: false, image: DEFAULT_IMAGE.to_string(), native: false, keep: false, command: command.into() }
    }

    /// The value after each occurrence of a flag.
    fn values<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
        args.windows(2).filter(|w| w[0] == flag).map(|w| w[1].as_str()).collect()
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(base64(plain.as_bytes()), encoded, "{plain}");
        }
        // The example of the wsl-toolkit manual: `echo hi` and a newline.
        assert_eq!(base64(b"echo hi\n"), "ZWNobyBoaQo=");
        // Every byte value, so that no bit pattern is lost.
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(base64(&all).len(), 344);
    }

    #[test]
    fn the_command_travels_as_base64_of_the_script() {
        let o = opts("cargo test -p box-core && ls /dev");
        let args = wsl_args(&o, Some(r"C:\src\agent-box"), "4");
        assert_eq!(values(&args, "--command-base64"), [base64(container_script(&o.command).as_bytes()).as_str()]);
        // No word of the command is an argument of its own.
        assert!(!args.iter().any(|a| a == "/dev" || a == "cargo"));
        assert_eq!(container_script("true"), "cd /work || exit 71\ntrue\n");
    }

    #[test]
    fn each_exclude_is_one_flag_and_one_word() {
        let args = wsl_args(&opts("true"), Some(r"C:\src\agent-box"), "4");
        assert_eq!(values(&args, "--exclude"), ["target", ".git", ".refs", ".work", ".claude"]);
        assert_eq!(values(&args, "--workspace"), [r"C:\src\agent-box"]);
        // Each flag has its own value: no pattern is glued to a flag, and no
        // pattern holds a space that a second parse could split.
        let flags = args.iter().filter(|a| *a == "--exclude").count();
        assert_eq!(flags, EXCLUDES.len());
        assert!(values(&args, "--exclude").iter().all(|p| !p.contains(' ') && !p.starts_with('-')));
    }

    #[test]
    fn wsl_lifecycle_timeout_jobs_and_kvm() {
        let mut o = opts("true");
        let args = wsl_args(&o, Some("C:/r"), "2");
        assert_eq!(args[0], "run");
        assert_eq!(values(&args, "--container-lifecycle"), ["ephemeral"]);
        assert_eq!(values(&args, "--timeout"), ["60m"]);
        assert_eq!(values(&args, "--env"), ["CARGO_BUILD_JOBS=2"]);
        assert_eq!(values(&args, "--device"), Vec::<&str>::new());
        o.kvm = true;
        o.keep = true;
        let args = wsl_args(&o, None, "4");
        assert_eq!(values(&args, "--device"), ["/dev/kvm"]);
        assert_eq!(values(&args, "--container-lifecycle"), ["persistent"]);
        assert!(values(&args, "--workspace").is_empty() && values(&args, "--exclude").is_empty());
        assert_eq!(values(&args, "--command-base64"), [base64(b"true\n").as_str()]);
    }

    #[test]
    fn parse_takes_options_then_the_command() {
        let o = parse(&words(&["--kvm", "--", "ls", "-l", "/dev/kvm"])).unwrap();
        assert!(o.kvm && !o.keep && !o.native);
        assert_eq!(o.command, "ls -l /dev/kvm");
        assert_eq!(o.image, DEFAULT_IMAGE);
        // Without `--`, the first word that is not an option starts COMMAND,
        // and an option after it belongs to COMMAND.
        let o = parse(&words(&["--image", "docker.io/library/alpine:3", "cargo", "--kvm"])).unwrap();
        assert!(!o.kvm);
        assert_eq!(o.command, "cargo --kvm");
        assert_eq!(o.image, "docker.io/library/alpine:3");
        let o = parse(&words(&["--image=img", "--keep", "--", "true"])).unwrap();
        assert!(o.keep);
        assert_eq!(o.image, "img");
    }

    #[test]
    fn parse_refuses_what_it_cannot_run() {
        assert!(parse(&[]).is_err());
        assert!(parse(&words(&["--kvm", "--"])).is_err());
        assert!(parse(&words(&["--image"])).is_err());
        assert!(parse(&words(&["--image", "--kvm", "--", "true"])).is_err());
        assert!(parse(&words(&["--bogus", "--", "true"])).is_err());
        assert!(parse(&words(&["--native", "--keep", "--", "true"])).is_err());
        assert!(parse(&words(&["--native", "--image", "x", "--", "true"])).is_err());
    }

    #[test]
    fn jobs_default_to_four_and_refuse_nonsense() {
        assert_eq!(jobs_from(None).unwrap(), "4");
        assert_eq!(jobs_from(Some(" ")).unwrap(), "4");
        assert_eq!(jobs_from(Some("2")).unwrap(), "2");
        assert!(jobs_from(Some("0")).is_err());
        assert!(jobs_from(Some("two")).is_err());
    }

    #[test]
    fn engine_args_mount_the_repository_at_work() {
        let mut o = opts("cargo test");
        o.kvm = true;
        let args = engine_args(Engine::Podman, &o, "/home/dev/agent-box", "4", false).unwrap();
        assert_eq!(args[..2], ["run", "--rm"]);
        assert_eq!(values(&args, "--volume"), ["/home/dev/agent-box:/work"]);
        assert_eq!(values(&args, "--workdir"), ["/work"]);
        assert_eq!(values(&args, "--device"), ["/dev/kvm"]);
        assert_eq!(values(&args, "--group-add"), ["keep-groups"]);
        assert_eq!(args[args.len() - 4..], [DEFAULT_IMAGE, "sh", "-c", "cargo test"]);
        let docker = engine_args(Engine::Docker, &o, "/r", "4", true).unwrap();
        assert!(values(&docker, "--timeout").is_empty() && values(&docker, "--group-add").is_empty());
        assert!(docker.iter().any(|a| a == "--tty"));
        assert!(engine_args(Engine::Podman, &o, "/a:b", "4", false).is_err());
    }

    #[test]
    fn workspace_drops_the_verbatim_prefix() {
        let verbatim = format!("{0}{0}?{0}C:{0}src{0}agent-box", '\\');
        assert_eq!(workspace_text(Path::new(&verbatim)).unwrap(), r"C:\src\agent-box");
        assert_eq!(workspace_text(Path::new("/home/dev/agent-box")).unwrap(), "/home/dev/agent-box");
    }
}
