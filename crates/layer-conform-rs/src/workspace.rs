//! Discover the Rust workspace layout from `Cargo.toml` files.
//!
//! The classifier needs to know, for any callee path like `layer_conform_core::pipeline`,
//! whether `layer_conform_core` is:
//! 1. another member of this workspace (→ `OtherPackage`)
//! 2. a third-party dependency (→ `ExternalPackage`)
//! 3. the standard library (`std` / `core` / `alloc`).
//!
//! We cover this by parsing the root manifest's `[workspace].members` plus
//! each member's `[package].name` and `[dependencies]`. We deliberately do
//! NOT shell out to `cargo metadata` — keeping the discovery pure-fs makes
//! the tool fast and avoids requiring a working cargo install.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const STDLIB_CRATES: &[&str] = &["std", "core", "alloc", "proc_macro", "test"];

#[derive(Debug, Clone)]
pub struct Workspace {
    /// Absolute path to the directory containing the root `Cargo.toml`.
    pub root: PathBuf,
    /// Members keyed by their declared package name (with `-` → `_`).
    pub members: HashMap<String, MemberInfo>,
    /// Union of every dep declared in any member's `[dependencies]` /
    /// `[dev-dependencies]` / `[build-dependencies]`, minus workspace members.
    pub external_deps: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct MemberInfo {
    /// Display name (with `-`, e.g. "layer-conform-rs"). Use `crate_name` for matching.
    pub name: String,
    /// Underscore-normalized name used in `use` paths (e.g. "layer_conform_rs").
    pub crate_name: String,
    /// Absolute path to the member directory (containing the member Cargo.toml).
    pub package_dir: PathBuf,
    /// Absolute path to the member's `src/` directory.
    pub src_dir: PathBuf,
}

impl Workspace {
    /// Resolve the package + (optional) src-relative dir for a given source
    /// file path. Returns `None` when the file is outside any known member.
    pub fn locate_file(&self, file: &Path) -> Option<FileLocation<'_>> {
        for m in self.members.values() {
            if let Ok(rel) = file.strip_prefix(&m.src_dir) {
                let dir = rel.parent().map_or_else(PathBuf::new, Path::to_path_buf);
                return Some(FileLocation { member: m, src_relative_dir: dir });
            }
        }
        None
    }

    /// Look up a top-level `use` segment.
    pub fn classify_use_root(&self, root_seg: &str) -> UseRoot<'_> {
        if STDLIB_CRATES.contains(&root_seg) {
            return UseRoot::Stdlib;
        }
        // Both forms accepted: `layer-conform-core` and `layer_conform_core`.
        let normalized = root_seg.replace('-', "_");
        if let Some(m) = self.members.values().find(|m| m.crate_name == normalized) {
            return UseRoot::Member(m);
        }
        if self.external_deps.contains(&normalized)
            || self.external_deps.contains(root_seg)
        {
            return UseRoot::External(normalized);
        }
        UseRoot::Unknown(normalized)
    }
}

#[derive(Debug)]
pub struct FileLocation<'a> {
    pub member: &'a MemberInfo,
    /// Caller's directory relative to its package's `src/`.
    /// Empty `PathBuf` means "directly under src/".
    pub src_relative_dir: PathBuf,
}

#[derive(Debug)]
pub enum UseRoot<'a> {
    Stdlib,
    Member(&'a MemberInfo),
    External(String),
    /// Unrecognized — likely an unlisted crate. We treat it as external
    /// rather than failing so that misconfigured manifests don't tank scoring.
    Unknown(String),
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("read {0}: {1}")]
    Read(PathBuf, std::io::Error),
    #[error("parse {0}: {1}")]
    Parse(PathBuf, toml::de::Error),
    #[error("no Cargo.toml at workspace root {0}")]
    NoRootManifest(PathBuf),
}

#[derive(Debug, Default, Deserialize)]
struct CargoManifest {
    package: Option<PackageTable>,
    workspace: Option<WorkspaceTable>,
    #[serde(default)]
    dependencies: HashMap<String, toml::Value>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: HashMap<String, toml::Value>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: HashMap<String, toml::Value>,
}

#[derive(Debug, Deserialize)]
struct PackageTable {
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct WorkspaceTable {
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    dependencies: HashMap<String, toml::Value>,
}

/// Discover a Rust workspace rooted at `root`. Returns `Ok(None)` when no
/// `Cargo.toml` lives at `root` (caller may operate without resolution then).
pub fn discover_workspace(root: &Path) -> Result<Option<Workspace>, WorkspaceError> {
    let manifest_path = root.join("Cargo.toml");
    if !manifest_path.exists() {
        return Ok(None);
    }
    let root_manifest = read_manifest(&manifest_path)?;

    let mut members: HashMap<String, MemberInfo> = HashMap::new();
    let mut external_deps: HashSet<String> = HashSet::new();

    let member_globs = root_manifest
        .workspace
        .as_ref()
        .map(|w| w.members.clone())
        .unwrap_or_default();
    let workspace_deps = root_manifest
        .workspace
        .as_ref()
        .map(|w| w.dependencies.clone())
        .unwrap_or_default();

    // If the root manifest itself declares a [package], treat it as a member.
    if let Some(name) = root_manifest.package.as_ref().and_then(|p| p.name.as_ref()) {
        let info = build_member_info(root, name)?;
        members.insert(info.crate_name.clone(), info);
        collect_external_deps(&root_manifest, &mut external_deps);
    }

    for member_glob in &member_globs {
        // We don't expand globs (yet) — most workspaces list explicit dirs.
        // Wildcards would require globset which is outside layer-conform-rs's scope.
        let member_dir = root.join(member_glob);
        if !member_dir.is_dir() {
            continue;
        }
        let member_manifest_path = member_dir.join("Cargo.toml");
        let member_manifest = read_manifest(&member_manifest_path)?;
        let Some(name) = member_manifest.package.as_ref().and_then(|p| p.name.as_ref()) else {
            continue;
        };
        let info = build_member_info(&member_dir, name)?;
        members.insert(info.crate_name.clone(), info);
        collect_external_deps(&member_manifest, &mut external_deps);
    }

    // Workspace-level [workspace.dependencies] are also external by definition.
    for k in workspace_deps.keys() {
        external_deps.insert(k.replace('-', "_"));
    }

    // Workspace members shouldn't appear as "external".
    for member_name in members.keys() {
        external_deps.remove(member_name);
    }

    Ok(Some(Workspace { root: root.to_path_buf(), members, external_deps }))
}

fn build_member_info(member_dir: &Path, declared_name: &str) -> Result<MemberInfo, WorkspaceError> {
    let crate_name = declared_name.replace('-', "_");
    Ok(MemberInfo {
        name: declared_name.to_string(),
        crate_name,
        package_dir: member_dir.to_path_buf(),
        src_dir: member_dir.join("src"),
    })
}

fn collect_external_deps(manifest: &CargoManifest, acc: &mut HashSet<String>) {
    for k in manifest.dependencies.keys() {
        acc.insert(k.replace('-', "_"));
    }
    for k in manifest.dev_dependencies.keys() {
        acc.insert(k.replace('-', "_"));
    }
    for k in manifest.build_dependencies.keys() {
        acc.insert(k.replace('-', "_"));
    }
}

fn read_manifest(path: &Path) -> Result<CargoManifest, WorkspaceError> {
    let raw = fs::read_to_string(path)
        .map_err(|e| WorkspaceError::Read(path.to_path_buf(), e))?;
    toml::from_str::<CargoManifest>(&raw)
        .map_err(|e| WorkspaceError::Parse(path.to_path_buf(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    #[test]
    fn discover_returns_none_when_no_cargo_toml() {
        let dir = tempdir().unwrap();
        let ws = discover_workspace(dir.path()).unwrap();
        assert!(ws.is_none());
    }

    #[test]
    fn discover_single_package_no_workspace() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"alone\"\n[dependencies]\nanyhow = \"1\"\n",
        );
        write(dir.path(), "src/lib.rs", "");
        let ws = discover_workspace(dir.path()).unwrap().unwrap();
        assert!(ws.members.contains_key("alone"));
        assert!(ws.external_deps.contains("anyhow"));
    }

    #[test]
    fn discover_workspace_with_members() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n",
        );
        write(
            dir.path(),
            "crates/a/Cargo.toml",
            "[package]\nname = \"a-crate\"\n[dependencies]\nserde = \"1\"\n",
        );
        write(dir.path(), "crates/a/src/lib.rs", "");
        write(
            dir.path(),
            "crates/b/Cargo.toml",
            "[package]\nname = \"b\"\n[dependencies]\na-crate = { path = \"../a\" }\n",
        );
        write(dir.path(), "crates/b/src/lib.rs", "");

        let ws = discover_workspace(dir.path()).unwrap().unwrap();
        assert!(ws.members.contains_key("a_crate"));
        assert!(ws.members.contains_key("b"));
        // a-crate is a workspace member, not external.
        assert!(!ws.external_deps.contains("a_crate"));
        assert!(ws.external_deps.contains("serde"));
    }

    #[test]
    fn classify_use_root_distinguishes_stdlib_member_external() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\"]\n",
        );
        write(
            dir.path(),
            "crates/a/Cargo.toml",
            "[package]\nname = \"a\"\n[dependencies]\nanyhow = \"1\"\n",
        );
        write(dir.path(), "crates/a/src/lib.rs", "");
        let ws = discover_workspace(dir.path()).unwrap().unwrap();

        match ws.classify_use_root("std") {
            UseRoot::Stdlib => {}
            other => panic!("std should be Stdlib, got {other:?}"),
        }
        match ws.classify_use_root("a") {
            UseRoot::Member(m) => assert_eq!(m.name, "a"),
            other => panic!("a should be Member, got {other:?}"),
        }
        match ws.classify_use_root("anyhow") {
            UseRoot::External(n) => assert_eq!(n, "anyhow"),
            other => panic!("anyhow should be External, got {other:?}"),
        }
        match ws.classify_use_root("unknown_xyz") {
            UseRoot::Unknown(n) => assert_eq!(n, "unknown_xyz"),
            other => panic!("unknown_xyz should be Unknown, got {other:?}"),
        }
    }

    #[test]
    fn locate_file_resolves_member_and_relative_dir() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/cli\"]\n",
        );
        write(
            dir.path(),
            "crates/cli/Cargo.toml",
            "[package]\nname = \"cli\"\n",
        );
        let file = dir.path().join("crates/cli/src/commands/check.rs");
        write(dir.path(), "crates/cli/src/commands/check.rs", "");
        let ws = discover_workspace(dir.path()).unwrap().unwrap();
        let loc = ws.locate_file(&file).unwrap();
        assert_eq!(loc.member.name, "cli");
        assert_eq!(loc.src_relative_dir, PathBuf::from("commands"));
    }
}
