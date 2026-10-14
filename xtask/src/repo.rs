//! The repository: its root, its files and their text.
//!
//! The root comes from this crate's manifest directory at build time, so a
//! command gives the same answer from any working directory. The file lists
//! come from git, so an ignored build output is never scanned.

use std::path::{Path, PathBuf};

use crate::proc;

/// The repository that this xtask belongs to.
#[derive(Debug, Clone)]
pub(crate) struct Repo {
    pub(crate) root: PathBuf,
}

impl Repo {
    /// The repository root: the parent of the xtask manifest directory.
    pub(crate) fn find() -> Result<Self, String> {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest.parent().ok_or("the xtask manifest directory has no parent")?.to_path_buf();
        if !root.join("Cargo.toml").is_file() {
            return Err(format!("{} has no Cargo.toml: this is not the repository root", root.display()));
        }
        Ok(Self { root })
    }

    pub(crate) fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// The tracked files, as paths relative to the root with `/` separators.
    pub(crate) fn tracked_files(&self) -> Result<Vec<String>, String> {
        self.git_list(&["ls-files", "-z"])
    }

    /// The untracked files that git does not ignore. A new file that was never
    /// staged is where a new credential is most likely to be, and the next
    /// `git add -A` takes it.
    pub(crate) fn untracked_files(&self) -> Result<Vec<String>, String> {
        self.git_list(&["ls-files", "-z", "--others", "--exclude-standard"])
    }

    /// Tracked files plus untracked files that are not ignored, without the
    /// files that are deleted in the working tree.
    pub(crate) fn visible_files(&self) -> Result<Vec<String>, String> {
        let mut all = self.tracked_files()?;
        all.extend(self.untracked_files()?);
        all.retain(|f| self.path(f).is_file());
        all.sort();
        all.dedup();
        Ok(all)
    }

    fn git_list(&self, args: &[&str]) -> Result<Vec<String>, String> {
        let out = proc::run_in(&self.root, "git", args, proc::QUICK)?;
        if !out.success() {
            return Err(format!("git {} failed: {}", args.join(" "), out.stderr.trim()));
        }
        Ok(out.stdout.split('\0').filter(|s| !s.is_empty()).map(str::to_string).collect())
    }

    /// The bytes of a file relative to the root.
    pub(crate) fn read_bytes(&self, rel: &str) -> Result<Vec<u8>, String> {
        std::fs::read(self.path(rel)).map_err(|e| format!("{rel}: {e}"))
    }

    /// The text of a file relative to the root. Not UTF-8 is an error.
    pub(crate) fn read_text(&self, rel: &str) -> Result<String, String> {
        let bytes = self.read_bytes(rel)?;
        String::from_utf8(bytes).map_err(|_| format!("{rel}: not UTF-8"))
    }
}

/// File extensions and names that are text in this repository. A file that is
/// not in this list is not read by the text checks.
pub(crate) fn is_text_path(rel: &str) -> bool {
    const NAMES: [&str; 6] = ["LICENSE", ".gitignore", ".gitattributes", ".editorconfig", "commit-msg", "Cargo.lock"];
    const EXTS: [&str; 12] = ["md", "rs", "toml", "yml", "yaml", "json", "sh", "txt", "lock", "cfg", "conf", "config"];
    let name = rel.rsplit('/').next().unwrap_or(rel);
    if NAMES.contains(&name) {
        return true;
    }
    match name.rsplit_once('.') {
        Some((_, ext)) => EXTS.contains(&ext),
        None => false,
    }
}

/// True for a Markdown document.
pub(crate) fn is_markdown(rel: &str) -> bool {
    Path::new(rel).extension().is_some_and(|e| e.eq_ignore_ascii_case("md"))
}
