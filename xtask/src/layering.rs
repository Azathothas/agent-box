//! `cargo xtask layering` and the check `layering`: the crate graph goes down
//! only.
//!
//! The defect this check exists to catch: a dependency edge that goes up or
//! across the layers of the workspace. One such edge can put host code into the
//! guest agent, or let a crate other than the helper start a microVM. Cargo
//! accepts these edges, and a review of one manifest does not see the graph.
//!
//! The check reads the graph from `cargo metadata`. The layer table below is
//! the one home of the layers; `Cargo.toml` and docs/architecture.md describe
//! them. The rules work on a plain graph, so the tests do not run cargo.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::proc;
use crate::repo::Repo;
use crate::report::{self, CheckReport, Finding};

pub(crate) const DEFECT: &str =
    "a crate depends on a crate of its own layer or a higher one, or breaks a crate graph rule";

const NAME: &str = "layering";

/// The workspace always has xtask. Zero crates read means that the metadata
/// is wrong, not that the graph is clean.
const FLOOR: usize = 1;

/// The time limit of `cargo metadata`. The first call in the repository can
/// make rustup install the pinned toolchain.
const LIMIT: Duration = Duration::from_mins(10);

/// The layer of each crate. A normal or build dependency points to a strictly
/// lower layer. xtask is above everything, and a rule below keeps it alone.
#[rustfmt::skip]
const LAYERS: &[(&str, u8)] = &[
    ("box-core", 0),
    ("box-proto", 1), ("box-policy", 1), ("box-image", 1), ("box-transport", 1),
    ("box-vmm", 2), ("box-net", 2), ("box-broker", 2),
    ("box-daemon", 3),
    ("box-chat", 4), ("box-web", 4), ("box-pool", 4),
    ("box-cli", 5), ("box-helper", 5), ("box-guest", 5),
    ("xtask", 9),
];

/// The binary crates. No crate depends on one, not even as a dev-dependency.
/// A member without a library target is a binary crate too.
const BINARIES: [&str; 4] = ["box-cli", "box-helper", "box-guest", "xtask"];

/// Crates that depend only on the crates in their list.
#[rustfmt::skip]
const ONLY: &[(&str, &[&str])] = &[
    ("box-guest", &["box-core", "box-proto"]),
    ("box-helper", &["box-core", "box-proto", "box-vmm"]),
    ("xtask", &[]),
];

/// Crates that never depend on the crates in their list. The late crates reach
/// the broker and the engines through the daemon.
#[rustfmt::skip]
const NEVER: &[(&str, &[&str])] = &[
    ("box-chat", &["box-broker", "box-vmm"]),
    ("box-web", &["box-broker", "box-vmm"]),
    ("box-pool", &["box-broker", "box-vmm"]),
];

/// Only the helper uses the engine crate. The daemon reaches the engines
/// through the helper process.
const VMM: &str = "box-vmm";
const VMM_USER: &str = "box-helper";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DepKind {
    Normal,
    Build,
    Dev,
}

/// One dependency on another workspace member.
#[derive(Debug, Clone)]
struct Dep {
    name: String,
    kind: DepKind,
}

/// One workspace member.
#[derive(Debug, Clone)]
struct Crate {
    name: String,
    /// The manifest path relative to the repository root, for the findings.
    manifest: String,
    /// True when the package has no library target.
    binary: bool,
    /// The dependencies on other workspace members only.
    deps: Vec<Dep>,
}

/// `cargo xtask layering [--json]`
pub(crate) fn run(args: &[String]) -> u8 {
    let mut as_json = false;
    for arg in args {
        if arg == "--json" {
            as_json = true;
        } else {
            eprintln!("xtask layering: unknown argument `{arg}`\n\nusage: cargo xtask layering [--json]");
            return report::EXIT_USAGE;
        }
    }
    let repo = match Repo::find() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("xtask layering: {e}");
            return report::EXIT_NOT_RUN;
        }
    };
    report::emit(&[check(&repo)], as_json)
}

/// The check `layering`. Read-only: it runs `cargo metadata` and reads the
/// answer.
pub(crate) fn check(repo: &Repo) -> CheckReport {
    let text = match metadata(repo) {
        Ok(t) => t,
        Err(e) => return CheckReport::not_run(NAME, e),
    };
    let graph = match parse_metadata(&text, &repo.root) {
        Ok(g) => g,
        Err(e) => return CheckReport::not_run(NAME, e),
    };
    let edges: usize = graph.iter().map(|c| c.deps.len()).sum();
    CheckReport::from_findings(NAME, graph.len(), FLOOR, findings(&graph))
        .note(format!("read {} workspace crates and {edges} edges between them", graph.len()))
}

/// The output of `cargo metadata --no-deps`, first offline, then online. The
/// online call is for a host that has no local copy of the registry index.
fn metadata(repo: &Repo) -> Result<String, String> {
    const ONLINE: [&str; 4] = ["metadata", "--format-version", "1", "--no-deps"];
    const OFFLINE: [&str; 5] = ["metadata", "--format-version", "1", "--no-deps", "--offline"];
    let mut last = String::new();
    for args in [&OFFLINE[..], &ONLINE[..]] {
        let shown = format!("cargo {}", args.join(" "));
        let out = proc::run_in(&repo.root, "cargo", args, LIMIT).map_err(|e| format!("cargo did not start: {e}"))?;
        if out.success() {
            return Ok(out.stdout);
        }
        last = if out.timed_out {
            format!("`{shown}` did not answer in {} s", LIMIT.as_secs())
        } else {
            format!("`{shown}` failed: {}", first_line(&out.stderr))
        };
    }
    Err(last)
}

/// The workspace members and their edges to each other, from the JSON of
/// `cargo metadata`.
fn parse_metadata(text: &str, root: &Path) -> Result<Vec<Crate>, String> {
    let doc: Value =
        serde_json::from_str(text).map_err(|e| format!("the JSON of cargo metadata does not parse: {e}"))?;
    let members: Vec<&str> = doc["workspace_members"]
        .as_array()
        .ok_or("cargo metadata gave no workspace_members")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let packages: Vec<&Value> = doc["packages"]
        .as_array()
        .ok_or("cargo metadata gave no packages")?
        .iter()
        .filter(|p| p["id"].as_str().is_some_and(|id| members.contains(&id)))
        .collect();
    let names: Vec<&str> = packages.iter().filter_map(|p| p["name"].as_str()).collect();
    let mut graph = Vec::new();
    for p in packages {
        let Some(name) = p["name"].as_str() else {
            return Err("cargo metadata gave a workspace package with no name".to_string());
        };
        let manifest = p["manifest_path"].as_str().map_or_else(|| name.to_string(), |m| relative(root, m));
        let binary = !p["targets"].as_array().is_some_and(|ts| ts.iter().any(is_library_target));
        let deps = p["dependencies"]
            .as_array()
            .map(|ds| ds.iter().filter_map(|d| workspace_dep(d, &names)).collect())
            .unwrap_or_default();
        graph.push(Crate { name: name.to_string(), manifest, binary, deps });
    }
    Ok(graph)
}

fn is_library_target(target: &Value) -> bool {
    target["kind"].as_array().is_some_and(|kinds| {
        kinds
            .iter()
            .filter_map(Value::as_str)
            .any(|k| matches!(k, "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro"))
    })
}

/// A dependency on a workspace member, or `None`. A path dependency has no
/// source. A registry crate with the name of a member is a different crate.
fn workspace_dep(dep: &Value, members: &[&str]) -> Option<Dep> {
    let name = dep["name"].as_str()?;
    if !dep["source"].is_null() || !members.contains(&name) {
        return None;
    }
    let kind = match dep["kind"].as_str() {
        Some("dev") => DepKind::Dev,
        Some("build") => DepKind::Build,
        _ => DepKind::Normal,
    };
    Some(Dep { name: name.to_string(), kind })
}

/// A path relative to the repository root, with `/` separators. A path
/// outside the root stays as it is.
fn relative(root: &Path, path: &str) -> String {
    let full = Path::new(path);
    full.strip_prefix(root).unwrap_or(full).to_string_lossy().replace('\\', "/")
}

fn layer(name: &str) -> Option<u8> {
    LAYERS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l)
}

/// Every rule, on every crate and every edge. One finding for each crate
/// without a layer, and one finding for each bad edge with all its reasons.
fn findings(graph: &[Crate]) -> Vec<Finding> {
    let mut out = Vec::new();
    for c in graph {
        let from = layer(&c.name);
        if from.is_none() {
            out.push(Finding::new(
                &c.manifest,
                0,
                format!("{} is a workspace member with no layer; add it to LAYERS in xtask/src/layering.rs", c.name),
            ));
        }
        for d in &c.deps {
            let reasons = edge_reasons(c, from, d, graph);
            if !reasons.is_empty() {
                let kind = match d.kind {
                    DepKind::Normal => "",
                    DepKind::Build => " (build-dependency)",
                    DepKind::Dev => " (dev-dependency)",
                };
                out.push(Finding::new(
                    &c.manifest,
                    0,
                    format!("{} -> {}{kind}: {}", c.name, d.name, reasons.join("; ")),
                ));
            }
        }
    }
    out
}

/// The rules that one edge breaks. Empty when the edge is correct.
fn edge_reasons(from_crate: &Crate, from: Option<u8>, dep: &Dep, graph: &[Crate]) -> Vec<String> {
    let mut reasons = Vec::new();
    let me = from_crate.name.as_str();
    let to = dep.name.as_str();
    let to_binary = BINARIES.contains(&to) || graph.iter().any(|g| g.name == to && g.binary);
    if to_binary {
        reasons.push(format!("{to} is a binary crate, and no crate depends on a binary crate"));
    }
    // A dev-dependency builds tests only. It can point to any library crate.
    if dep.kind == DepKind::Dev {
        return reasons;
    }
    if let (Some(a), Some(b)) = (from, layer(to))
        && b >= a
    {
        reasons.push(format!("layer {b} is not lower than layer {a}"));
    }
    if let Some((_, allowed)) = ONLY.iter().find(|(n, _)| *n == me)
        && !allowed.contains(&to)
    {
        if allowed.is_empty() {
            reasons.push(format!("{me} depends on no workspace crate"));
        } else {
            reasons.push(format!("{me} depends only on {}", allowed.join(", ")));
        }
    }
    if to == VMM && me != VMM_USER {
        reasons.push(format!("only {VMM_USER} uses {VMM}"));
    }
    if let Some((_, banned)) = NEVER.iter().find(|(n, _)| *n == me)
        && banned.contains(&to)
    {
        reasons.push(format!("{me} never depends on {to}"));
    }
    reasons
}

fn first_line(text: &str) -> &str {
    text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("no message")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use super::DepKind::{Build, Dev, Normal};

    fn krate(name: &str, deps: &[(&str, DepKind)]) -> Crate {
        Crate {
            name: name.to_string(),
            manifest: format!("crates/{name}/Cargo.toml"),
            binary: BINARIES.contains(&name),
            deps: deps.iter().map(|(n, k)| Dep { name: (*n).to_string(), kind: *k }).collect(),
        }
    }

    /// The planned graph of the workspace (Cargo.toml), which obeys every rule.
    fn planned() -> Vec<Crate> {
        let n = |names: &[&'static str]| names.iter().map(|x| (*x, Normal)).collect::<Vec<_>>();
        vec![
            krate("box-core", &[]),
            krate("box-proto", &n(&["box-core"])),
            krate("box-policy", &n(&["box-core"])),
            krate("box-image", &n(&["box-core"])),
            krate("box-transport", &n(&["box-core"])),
            krate("box-vmm", &n(&["box-core"])),
            krate("box-net", &n(&["box-core", "box-policy", "box-proto"])),
            krate("box-broker", &n(&["box-core", "box-policy"])),
            krate(
                "box-daemon",
                &n(&["box-core", "box-proto", "box-policy", "box-image", "box-transport", "box-net", "box-broker"]),
            ),
            krate("box-chat", &n(&["box-core", "box-daemon"])),
            krate("box-cli", &n(&["box-core", "box-daemon", "box-policy", "box-proto"])),
            krate("box-helper", &n(&["box-core", "box-proto", "box-vmm"])),
            krate("box-guest", &n(&["box-core", "box-proto"])),
            krate("xtask", &[]),
        ]
    }

    /// The planned graph with one more edge.
    fn with_edge(from: &str, to: &str, kind: DepKind) -> Vec<Crate> {
        let mut g = planned();
        let c = g.iter_mut().find(|c| c.name == from).expect("the crate is in the planned graph");
        c.deps.push(Dep { name: to.to_string(), kind });
        g
    }

    fn messages(graph: &[Crate]) -> Vec<String> {
        findings(graph).into_iter().map(|f| f.message).collect()
    }

    #[test]
    fn the_planned_graph_obeys_every_rule() {
        assert_eq!(messages(&planned()), Vec::<String>::new());
    }

    #[test]
    fn a_planted_upward_edge_is_a_finding() {
        let m = messages(&with_edge("box-core", "box-daemon", Normal));
        assert_eq!(m, vec!["box-core -> box-daemon: layer 3 is not lower than layer 0".to_string()]);
    }

    #[test]
    fn an_edge_inside_one_layer_is_a_finding() {
        let m = messages(&with_edge("box-net", "box-broker", Build));
        assert_eq!(m.len(), 1);
        assert!(m[0].contains("(build-dependency)") && m[0].contains("layer 2 is not lower than layer 2"));
    }

    #[test]
    fn the_guest_and_the_helper_have_short_lists() {
        let m = messages(&with_edge("box-guest", "box-policy", Normal));
        assert_eq!(m, vec!["box-guest -> box-policy: box-guest depends only on box-core, box-proto".to_string()]);
        let m = messages(&with_edge("box-helper", "box-net", Normal));
        assert!(m[0].contains("box-helper depends only on"));
    }

    #[test]
    fn only_the_helper_uses_the_engine_crate() {
        let m = messages(&with_edge("box-daemon", "box-vmm", Normal));
        assert_eq!(m, vec!["box-daemon -> box-vmm: only box-helper uses box-vmm".to_string()]);
    }

    #[test]
    fn nothing_depends_on_a_binary_crate() {
        let m = messages(&with_edge("box-cli", "box-helper", Normal));
        assert!(m[0].contains("box-helper is a binary crate"));
        let m = messages(&with_edge("box-core", "box-guest", Dev));
        assert_eq!(m.len(), 1);
        assert!(m[0].contains("(dev-dependency)") && m[0].contains("binary crate"));
    }

    #[test]
    fn a_dev_dependency_can_point_up_to_a_library() {
        assert_eq!(messages(&with_edge("box-core", "box-daemon", Dev)), Vec::<String>::new());
    }

    #[test]
    fn the_late_crates_never_reach_the_broker() {
        let m = messages(&with_edge("box-chat", "box-broker", Normal));
        assert_eq!(m, vec!["box-chat -> box-broker: box-chat never depends on box-broker".to_string()]);
    }

    #[test]
    fn xtask_depends_on_no_workspace_crate() {
        let m = messages(&with_edge("xtask", "box-core", Normal));
        assert_eq!(m, vec!["xtask -> box-core: xtask depends on no workspace crate".to_string()]);
    }

    #[test]
    fn a_member_without_a_layer_is_a_finding() {
        let mut g = planned();
        g.push(krate("box-extra", &[("box-core", Normal)]));
        let f = findings(&g);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].path, "crates/box-extra/Cargo.toml");
        assert!(f[0].message.contains("no layer"));
    }

    #[test]
    fn the_metadata_parser_keeps_member_edges_only() {
        let root = std::env::temp_dir().join("agent-box-layering-test");
        let manifest = |c: &str| root.join(c).join("Cargo.toml").to_string_lossy().into_owned();
        let doc = json!({
            "workspace_members": ["core-id", "cli-id"],
            "packages": [
                {
                    "id": "core-id", "name": "box-core", "manifest_path": manifest("box-core"),
                    "targets": [{"kind": ["lib"]}],
                    "dependencies": [
                        {"name": "serde_json", "source": "registry+https://github.com/rust-lang/crates.io-index", "kind": null}
                    ]
                },
                {
                    "id": "cli-id", "name": "box-cli", "manifest_path": manifest("box-cli"),
                    "targets": [{"kind": ["bin"]}],
                    "dependencies": [
                        {"name": "box-core", "source": null, "kind": null},
                        {"name": "box-core", "source": null, "kind": "dev"},
                        {"name": "box-proto", "source": "registry+https://github.com/rust-lang/crates.io-index", "kind": null}
                    ]
                }
            ]
        });
        let g = parse_metadata(&doc.to_string(), &root).unwrap();
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].manifest, "box-core/Cargo.toml");
        assert!(!g[0].binary);
        assert_eq!(g[0].deps.len(), 0);
        assert!(g[1].binary);
        let kinds: Vec<(&str, DepKind)> = g[1].deps.iter().map(|d| (d.name.as_str(), d.kind)).collect();
        assert_eq!(kinds, vec![("box-core", Normal), ("box-core", Dev)]);
    }

    #[test]
    fn metadata_without_members_is_an_error() {
        assert!(parse_metadata("{}", Path::new(".")).is_err());
        assert!(parse_metadata("not json", Path::new(".")).is_err());
    }
}
