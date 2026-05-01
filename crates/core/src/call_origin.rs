//! Language-neutral classification of a call's origin relative to its caller.
//!
//! Layer-conform's similarity model treats two calls as equivalent when they
//! cross the *same kind of boundary*, even if their concrete callee names
//! differ. This module owns the boundary vocabulary and the path arithmetic
//! that produces the relative-layer signature; language adapters (`lc-rs`,
//! `lc-ts`) are responsible only for resolving each call to a (package, dir)
//! pair and then asking this module to encode it.
//!
//! The signature is **truncated at the first divergent folder** when going
//! from caller to callee — once we leave our own layer to enter a sibling
//! layer, we treat that sibling as opaque (we don't drill into its internals).

use std::path::{Component, Path, PathBuf};

use compact_str::CompactString;

/// How a call's callee relates to the caller, in terms of package / layer
/// boundaries. Encoded into the AST node value via [`encode`](Self::encode)
/// so that the shape (TSED) and calls (Jaccard) axes both treat
/// "same boundary, different name" as a match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallOrigin {
    /// Caller and callee live in the same layer (same folder of the same
    /// package).
    SameLayer,
    /// Same package, different layer. `signature` is the relative dir path
    /// from caller to callee, **truncated at the first folder past the
    /// common ancestor** (e.g. `"../loader/"`, `"../"`, `"../../foo/"`).
    OtherLayer { signature: String },
    /// Different package within the same workspace.
    OtherPackage { package: String },
    /// Third-party / external dependency.
    ExternalPackage { package: String },
    /// Standard library (Rust `std`/`core`/`alloc`, TS `node:*`, etc.).
    Stdlib,
    /// Method call where the receiver type couldn't be resolved.
    UnresolvedMethod,
    /// Macro / macro-like construct (Rust `println!`, TS tagged templates).
    Macro,
}

impl CallOrigin {
    /// Stable string used as the `value` of an `Identifier` tree node and as
    /// the entry in the `calls` Jaccard set.
    pub fn encode(&self) -> CompactString {
        match self {
            Self::SameLayer => CompactString::new("_SAME_LAYER"),
            Self::OtherLayer { signature } => {
                let mut s = CompactString::new("_LAYER:");
                s.push_str(signature);
                s
            }
            Self::OtherPackage { package } => {
                let mut s = CompactString::new("_PKG:");
                s.push_str(package);
                s
            }
            Self::ExternalPackage { package } => {
                let mut s = CompactString::new("_EXT:");
                s.push_str(package);
                s
            }
            Self::Stdlib => CompactString::new("_STDLIB"),
            Self::UnresolvedMethod => CompactString::new("_METHOD"),
            Self::Macro => CompactString::new("_MACRO"),
        }
    }
}

/// Build the relative-layer signature for a call from `caller_dir` to
/// `callee_dir`, **truncated at the first divergent folder**.
///
/// Both inputs must be directory paths within the same package. Components
/// must not contain `..` (callers should normalize first).
///
/// Returns `None` when both directories are equal (`SameLayer` case — the
/// caller should construct that variant directly).
pub fn layer_signature(caller_dir: &Path, callee_dir: &Path) -> Option<String> {
    let caller_parts: Vec<&str> = path_components(caller_dir);
    let callee_parts: Vec<&str> = path_components(callee_dir);

    let common_len = caller_parts
        .iter()
        .zip(callee_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    if common_len == caller_parts.len() && common_len == callee_parts.len() {
        return None;
    }

    let up_count = caller_parts.len() - common_len;
    let mut out = String::new();
    for _ in 0..up_count {
        out.push_str("../");
    }
    // Take only the first folder past the common ancestor on the callee side
    // — internals of the destination layer are intentionally hidden.
    if common_len < callee_parts.len() {
        out.push_str(callee_parts[common_len]);
        out.push('/');
    }
    Some(out)
}

fn path_components(p: &Path) -> Vec<&str> {
    p.components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            Component::CurDir => None,
            // RootDir / Prefix / ParentDir aren't expected; callers should
            // pass already-relativized package-internal paths.
            _ => None,
        })
        .collect()
}

/// Convenience: classify a call into [`CallOrigin::SameLayer`] when caller
/// and callee dirs match, else [`CallOrigin::OtherLayer`] with the truncated
/// signature.
pub fn intra_package_origin(caller_dir: &Path, callee_dir: &Path) -> CallOrigin {
    match layer_signature(caller_dir, callee_dir) {
        None => CallOrigin::SameLayer,
        Some(signature) => CallOrigin::OtherLayer { signature },
    }
}

/// Returns the directory containing `file` (POSIX-normalized component slice).
/// `file` may be a relative path inside a package's source root.
pub fn dir_of(file: &Path) -> PathBuf {
    file.parent().map_or_else(PathBuf::new, Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn same_dir_returns_none_signature() {
        assert_eq!(layer_signature(&p("src/commands"), &p("src/commands")), None);
    }

    #[test]
    fn parent_dir_of_caller() {
        // commands/ → src/  =>  go up once
        assert_eq!(layer_signature(&p("src/commands"), &p("src")).as_deref(), Some("../"));
    }

    #[test]
    fn sibling_layer_signature() {
        // commands/ → loader/ at src/  =>  ../loader/
        assert_eq!(
            layer_signature(&p("src/commands"), &p("src/loader")).as_deref(),
            Some("../loader/"),
        );
    }

    #[test]
    fn deeper_callee_is_truncated_to_first_folder() {
        // user spec: don't drill into the destination layer.
        assert_eq!(
            layer_signature(&p("src/commands"), &p("src/loader/deep/inner")).as_deref(),
            Some("../loader/"),
        );
    }

    #[test]
    fn cousin_at_two_levels_up() {
        // src/commands/sub → src/anotherLayer  =>  ../../anotherLayer/
        assert_eq!(
            layer_signature(&p("src/commands/sub"), &p("src/anotherLayer")).as_deref(),
            Some("../../anotherLayer/"),
        );
    }

    #[test]
    fn descendant_layer_no_up() {
        // src/commands → src/commands/sub  =>  sub/
        assert_eq!(
            layer_signature(&p("src/commands"), &p("src/commands/sub")).as_deref(),
            Some("sub/"),
        );
    }

    #[test]
    fn intra_package_helper_returns_same_layer_when_equal() {
        match intra_package_origin(&p("src/commands"), &p("src/commands")) {
            CallOrigin::SameLayer => {}
            other => panic!("expected SameLayer, got {other:?}"),
        }
    }

    #[test]
    fn intra_package_helper_returns_other_layer_when_diff() {
        match intra_package_origin(&p("src/commands"), &p("src/loader")) {
            CallOrigin::OtherLayer { signature } => assert_eq!(signature, "../loader/"),
            other => panic!("expected OtherLayer, got {other:?}"),
        }
    }

    #[test]
    fn encode_same_layer() {
        assert_eq!(CallOrigin::SameLayer.encode().as_str(), "_SAME_LAYER");
    }

    #[test]
    fn encode_other_layer_includes_signature() {
        let s = CallOrigin::OtherLayer { signature: "../loader/".into() }.encode();
        assert_eq!(s.as_str(), "_LAYER:../loader/");
    }

    #[test]
    fn encode_other_package() {
        let s = CallOrigin::OtherPackage { package: "lc_core".into() }.encode();
        assert_eq!(s.as_str(), "_PKG:lc_core");
    }

    #[test]
    fn encode_external_package() {
        let s = CallOrigin::ExternalPackage { package: "anyhow".into() }.encode();
        assert_eq!(s.as_str(), "_EXT:anyhow");
    }

    #[test]
    fn encode_stdlib_method_macro() {
        assert_eq!(CallOrigin::Stdlib.encode().as_str(), "_STDLIB");
        assert_eq!(CallOrigin::UnresolvedMethod.encode().as_str(), "_METHOD");
        assert_eq!(CallOrigin::Macro.encode().as_str(), "_MACRO");
    }

    #[test]
    fn dir_of_returns_parent() {
        assert_eq!(dir_of(&p("src/commands/check.rs")), p("src/commands"));
    }

    #[test]
    fn dir_of_returns_empty_for_bare_file() {
        assert_eq!(dir_of(&p("check.rs")), PathBuf::new());
    }
}
