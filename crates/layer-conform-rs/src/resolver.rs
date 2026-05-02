//! Resolve each call/macro/method site in a Rust source file to a
//! [`CallOrigin`](layer_conform_core::call_origin::CallOrigin).
//!
//! For one source file we
//! 1. parse every `use` statement into a leaf-name → resolved-path map,
//! 2. when we see `foo::bar()` or `foo()` look up the head segment in the
//!    map; combined with workspace info this tells us whether the callee
//!    lives in the same package, a sibling package, an external crate, or
//!    the standard library,
//! 3. classify method calls (`x.method()`) as `UnresolvedMethod` (we don't
//!    do type inference) and macros (`println!`) by their root segment.
//!
//! When called without a [`Workspace`] (e.g. in layer-conform-rs unit tests) the
//! classifier falls back to **raw name** encoding so existing tests that
//! assert on identifier values keep working.

use std::collections::HashMap;
use std::path::PathBuf;

use compact_str::CompactString;
use layer_conform_core::call_origin::{intra_package_origin, CallOrigin};
use syn::{File, Item, Path as SynPath, UseTree};

use crate::workspace::{FileLocation, MemberInfo, UseRoot, Workspace};

/// Where a single short identifier (the head of a `use` chain) resolves to.
#[derive(Clone, Debug)]
enum UseTarget {
    /// `use crate::foo::bar` → resolved within current package.
    /// `relative_path` is the `::`-joined module path *after* the leading
    /// `crate::`/`self::`/`super::`.
    IntraPackage { relative_path: Vec<String> },
    /// `use other::foo::bar` where `other` is a workspace member.
    /// `relative_path` is the path *after* the package root segment.
    OtherPackageMember { package: String, relative_path: Vec<String> },
    /// `use anyhow::*` — third-party.
    External { package: String },
    /// `use std::*`, `use core::*`, etc.
    Stdlib,
    /// `use unknown_xyz::*` — not in any manifest. Treat as external so
    /// misconfigured deps don't hard-fail the analyzer.
    UnknownExternal { package: String },
}

/// Per-file classifier.
pub struct ImportClassifier<'a> {
    workspace: Option<&'a Workspace>,
    /// Caller's location within its package. Some workspaces report files
    /// outside any member (e.g. tests/ or examples/) — those get None and
    /// we fall back to raw-name encoding for them.
    location: Option<FileLocation<'a>>,
    use_map: HashMap<String, UseTarget>,
}

impl<'a> ImportClassifier<'a> {
    /// Build a classifier with no resolution; every callee is encoded by
    /// its raw name (current behaviour). Used when no workspace context is
    /// available, e.g. in `layer-conform-rs` unit tests.
    pub fn empty() -> Self {
        Self { workspace: None, location: None, use_map: HashMap::new() }
    }

    /// Build a classifier from a parsed file plus its workspace context.
    pub fn build(workspace: &'a Workspace, file_path: &std::path::Path, file: &File) -> Self {
        let location = workspace.locate_file(file_path);
        let mut use_map: HashMap<String, UseTarget> = HashMap::new();
        for item in &file.items {
            if let Item::Use(u) = item {
                collect_use_tree(&u.tree, &mut Vec::new(), workspace, &mut use_map);
            }
        }
        Self { workspace: Some(workspace), location, use_map }
    }

    /// True when the classifier has both a workspace and a known caller
    /// location — i.e. it can produce `CallOrigin`-encoded values rather
    /// than falling back to raw names.
    pub fn enabled(&self) -> bool {
        self.workspace.is_some() && self.location.is_some()
    }

    /// Resolve a path expression to its [`CallOrigin`] without forcing an
    /// encoding. Returns `None` when:
    /// - the classifier is disabled, or
    /// - the path's head is not in the use map and is not a `crate`/`self`/
    ///   `super` keyword (typically a local identifier).
    pub fn resolve_path(&self, path: &SynPath) -> Option<CallOrigin> {
        self.classify_path(path)
    }

    /// Encode a free-standing call expression's callee.
    /// `path` is the full callee path (e.g. `foo::bar` or `bar`).
    pub fn classify_call(&self, path: &SynPath) -> CompactString {
        if !self.enabled() {
            return raw_path_dotted(path);
        }
        match self.classify_path(path) {
            Some(origin) => origin.encode(),
            None => raw_path_dotted(path),
        }
    }

    /// Encode a method call site (`receiver.method`).
    pub fn classify_method(&self, receiver_path: Option<&SynPath>, method: &str) -> CompactString {
        if !self.enabled() {
            return raw_method_name(receiver_path, method);
        }
        // If the receiver is a single-segment path that resolves via use,
        // we can classify it (e.g. `loader.load_config()` where `loader` is
        // imported). Otherwise it's a method on a local value → unresolved.
        if let Some(recv) = receiver_path {
            if let Some(origin) = self.classify_path(recv) {
                return origin.encode();
            }
        }
        CallOrigin::UnresolvedMethod.encode()
    }

    /// Encode a macro invocation by its root segment's classification when
    /// possible, falling back to `_MACRO` when the macro is built-in or
    /// unresolved.
    pub fn classify_macro(&self, mac_path: &SynPath) -> CompactString {
        if !self.enabled() {
            return raw_macro_name(mac_path);
        }
        // For macros, classifying by package is most informative —
        // `anyhow!` (external) vs `println!` (stdlib) vs a local crate
        // macro should each get their own label.
        match self.classify_path(mac_path) {
            Some(origin) => origin.encode(),
            None => CallOrigin::Macro.encode(),
        }
    }

    /// Common path-classification core. Returns None when we can't
    /// resolve (caller falls back to raw encoding for backwards compat).
    fn classify_path(&self, path: &SynPath) -> Option<CallOrigin> {
        let workspace = self.workspace?;
        let location = self.location.as_ref()?;
        let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        if segs.is_empty() {
            return None;
        }

        // crate:: / self:: / super:: are intra-package.
        // We model them as IntraPackage with relative segments.
        let head = segs[0].as_str();

        let target: UseTarget = match head {
            "crate" => UseTarget::IntraPackage {
                relative_path: segs.iter().skip(1).cloned().collect(),
            },
            "self" => UseTarget::IntraPackage {
                relative_path: segs.iter().skip(1).cloned().collect(),
            },
            "super" => {
                // super::* — caller dir's parent; we don't currently model
                // this precisely, treat as same-package OtherLayer up one.
                return Some(intra_package_origin(
                    &location.src_relative_dir,
                    &super_dir(&location.src_relative_dir),
                ));
            }
            _ => self.use_map.get(head).cloned()?,
        };

        Some(self.target_to_origin(workspace, location, &target, &segs))
    }

    fn target_to_origin(
        &self,
        workspace: &Workspace,
        caller: &FileLocation<'_>,
        target: &UseTarget,
        full_segs: &[String],
    ) -> CallOrigin {
        match target {
            UseTarget::Stdlib => CallOrigin::Stdlib,
            UseTarget::External { package } | UseTarget::UnknownExternal { package } => {
                CallOrigin::ExternalPackage { package: package.clone() }
            }
            UseTarget::OtherPackageMember { package, .. } => {
                CallOrigin::OtherPackage { package: package.clone() }
            }
            UseTarget::IntraPackage { relative_path } => {
                // Reconstruct the full module path from the use map plus the
                // call's own remaining segments, then drop the last element
                // (it's the called function/struct, not a module).
                let mut full: Vec<String> = relative_path.clone();
                if !full_segs.is_empty() {
                    full.extend(full_segs.iter().skip(1).cloned());
                }
                let mod_segs: &[String] = if full.is_empty() {
                    &full
                } else {
                    &full[..full.len() - 1]
                };
                let callee_dir = resolve_intra_module_to_dir(caller.member, mod_segs);
                intra_package_origin(&caller.src_relative_dir, &callee_dir)
            }
        }
    }
}

/// Resolve a module path within a package to its source-relative directory.
/// Walks `<src>/seg1/seg2/...` while each segment is an existing dir; once
/// a segment doesn't have a corresponding dir it's treated as a leaf file
/// (e.g. `crate::loader` → `src/loader.rs` → dir = `""`).
fn resolve_intra_module_to_dir(member: &MemberInfo, segments: &[String]) -> PathBuf {
    let mut dir = PathBuf::new();
    for seg in segments {
        let as_dir = member.src_dir.join(&dir).join(seg);
        if as_dir.is_dir() {
            dir.push(seg);
        } else {
            // Leaf file (or unknown) — stop walking; current `dir` is the
            // containing folder of the leaf file.
            return dir;
        }
    }
    dir
}

fn super_dir(caller_dir: &std::path::Path) -> PathBuf {
    caller_dir
        .parent()
        .map_or_else(PathBuf::new, std::path::Path::to_path_buf)
}

fn collect_use_tree(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    workspace: &Workspace,
    out: &mut HashMap<String, UseTarget>,
) {
    match tree {
        UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            collect_use_tree(&p.tree, prefix, workspace, out);
            prefix.pop();
        }
        UseTree::Name(n) => {
            let leaf = n.ident.to_string();
            insert_resolution(prefix, &leaf, &leaf, workspace, out);
        }
        UseTree::Rename(r) => {
            let original = r.ident.to_string();
            let alias = r.rename.to_string();
            insert_resolution(prefix, &original, &alias, workspace, out);
        }
        UseTree::Group(g) => {
            for inner in &g.items {
                collect_use_tree(inner, prefix, workspace, out);
            }
        }
        UseTree::Glob(_) => {
            // `use foo::*;` — record `foo` itself as resolvable so that
            // subsequent `foo::bar()` still classifies. We use the prefix's
            // first segment as the head.
            if let Some(head) = prefix.first().cloned() {
                let target = build_target_for_root(&head, prefix, workspace);
                out.insert(head, target);
            }
        }
    }
}

fn insert_resolution(
    prefix: &[String],
    original_leaf: &str,
    alias: &str,
    workspace: &Workspace,
    out: &mut HashMap<String, UseTarget>,
) {
    // The "head" of the use chain is what we want to resolve. If there is
    // no prefix (e.g. `use foo;` with no `::`), the leaf IS the root.
    let root = prefix.first().map_or(original_leaf, String::as_str);
    let mut full_chain: Vec<String> = prefix.to_vec();
    full_chain.push(original_leaf.to_string());

    let target = build_target_for_root(root, &full_chain, workspace);
    out.insert(alias.to_string(), target);
}

fn build_target_for_root(
    root: &str,
    full_chain: &[String],
    workspace: &Workspace,
) -> UseTarget {
    match root {
        "crate" | "self" => UseTarget::IntraPackage {
            relative_path: full_chain.iter().skip(1).cloned().collect(),
        },
        "super" => UseTarget::IntraPackage {
            // Modeled as same-package; layer signature handles the up step.
            relative_path: full_chain.iter().skip(1).cloned().collect(),
        },
        other => match workspace.classify_use_root(other) {
            UseRoot::Stdlib => UseTarget::Stdlib,
            UseRoot::Member(m) => UseTarget::OtherPackageMember {
                package: m.crate_name.clone(),
                relative_path: full_chain.iter().skip(1).cloned().collect(),
            },
            UseRoot::External(name) => UseTarget::External { package: name },
            UseRoot::Unknown(name) => UseTarget::UnknownExternal { package: name },
        },
    }
}

fn raw_path_dotted(p: &SynPath) -> CompactString {
    let segs: Vec<String> = p.segments.iter().map(|s| s.ident.to_string()).collect();
    CompactString::from(segs.join("."))
}

fn raw_method_name(receiver: Option<&SynPath>, method: &str) -> CompactString {
    if let Some(recv) = receiver {
        if recv.segments.len() == 1 {
            let mut s = CompactString::from(recv.segments[0].ident.to_string());
            s.push('.');
            s.push_str(method);
            return s;
        }
    }
    CompactString::from(method)
}

fn raw_macro_name(p: &SynPath) -> CompactString {
    let segs: Vec<String> = p.segments.iter().map(|s| s.ident.to_string()).collect();
    let mut s = CompactString::from(segs.join("::"));
    s.push('!');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::discover_workspace;
    use std::fs;
    use tempfile::tempdir;

    fn write(root: &std::path::Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    fn build_classifier_for(
        root: &std::path::Path,
        file_rel: &str,
        source: &str,
    ) -> (Workspace, std::path::PathBuf, syn::File) {
        let ws = discover_workspace(root).unwrap().unwrap();
        let file = root.join(file_rel);
        let parsed: syn::File = syn::parse_str(source).unwrap();
        (ws, file, parsed)
    }

    #[test]
    fn classify_call_resolves_use_to_external_package() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/cli\"]\n",
        );
        write(
            dir.path(),
            "crates/cli/Cargo.toml",
            "[package]\nname = \"cli\"\n[dependencies]\nanyhow = \"1\"\n",
        );
        write(
            dir.path(),
            "crates/cli/src/main.rs",
            "use anyhow::Result;\nfn run() -> Result<()> { Ok(()) }",
        );
        let (ws, file_path, parsed) = build_classifier_for(
            dir.path(),
            "crates/cli/src/main.rs",
            "use anyhow::Result;\nfn run() -> Result<()> { Ok(()) }",
        );
        let c = ImportClassifier::build(&ws, &file_path, &parsed);
        let path: SynPath = syn::parse_str("Result").unwrap();
        assert_eq!(c.classify_call(&path).as_str(), "_EXT:anyhow");
    }

    #[test]
    fn classify_call_to_other_workspace_member() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/cli\", \"crates/core\"]\n",
        );
        write(
            dir.path(),
            "crates/core/Cargo.toml",
            "[package]\nname = \"layer-conform-core\"\n",
        );
        write(dir.path(), "crates/core/src/lib.rs", "");
        write(
            dir.path(),
            "crates/cli/Cargo.toml",
            "[package]\nname = \"cli\"\n[dependencies]\nlc-core = { path = \"../core\" }\n",
        );
        write(dir.path(), "crates/cli/src/main.rs", "use layer_conform_core::pipeline;\nfn run() {}");
        let (ws, file_path, parsed) = build_classifier_for(
            dir.path(),
            "crates/cli/src/main.rs",
            "use layer_conform_core::pipeline;\nfn run() {}",
        );
        let c = ImportClassifier::build(&ws, &file_path, &parsed);
        let path: SynPath = syn::parse_str("pipeline::detect_deviations").unwrap();
        assert_eq!(c.classify_call(&path).as_str(), "_PKG:layer_conform_core");
    }

    #[test]
    fn classify_call_to_stdlib() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"a\"\n",
        );
        write(dir.path(), "src/lib.rs", "use std::io::stdout;\nfn run() {}");
        let (ws, file_path, parsed) = build_classifier_for(
            dir.path(),
            "src/lib.rs",
            "use std::io::stdout;\nfn run() {}",
        );
        let c = ImportClassifier::build(&ws, &file_path, &parsed);
        let path: SynPath = syn::parse_str("stdout").unwrap();
        assert_eq!(c.classify_call(&path).as_str(), "_STDLIB");
    }

    #[test]
    fn classify_call_intra_package_uses_layer_signature() {
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
        write(dir.path(), "crates/cli/src/loader.rs", "");
        // caller is in commands/check.rs, callee `loader` lives at src/loader.rs
        write(
            dir.path(),
            "crates/cli/src/commands/check.rs",
            "use crate::loader;\nfn run() {}",
        );
        let (ws, file_path, parsed) = build_classifier_for(
            dir.path(),
            "crates/cli/src/commands/check.rs",
            "use crate::loader;\nfn run() {}",
        );
        let c = ImportClassifier::build(&ws, &file_path, &parsed);
        let path: SynPath = syn::parse_str("loader::load_config").unwrap();
        // caller dir = "commands", callee dir = "" → up one
        assert_eq!(c.classify_call(&path).as_str(), "_LAYER:../");
    }

    #[test]
    fn fallback_classifier_returns_raw_names() {
        let c = ImportClassifier::empty();
        let path: SynPath = syn::parse_str("foo::bar").unwrap();
        assert_eq!(c.classify_call(&path).as_str(), "foo.bar");
    }
}
