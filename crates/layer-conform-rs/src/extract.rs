//! Extract top-level `fn` items and `impl` block methods from a Rust source.
//!
//! Optionally takes a [`FileContext`] (caller path + workspace) so that call
//! sites can be classified by relative-layer signature instead of literal
//! callee name. Without a context every callee falls back to its raw name —
//! this preserves the layer-conform-rs unit tests' expectations.

use std::path::Path;

use compact_str::CompactString;
use layer_conform_core::{FunctionKind, FunctionRef, Signature};
use syn::{ImplItem, ImplItemFn, Item, ItemFn, ItemImpl};

use crate::resolver::ImportClassifier;
use crate::workspace::Workspace;
use crate::{normalize, signature};

/// Per-file workspace context — same shape regardless of language.
pub struct FileContext<'a> {
    pub workspace: &'a Workspace,
    /// Absolute path of the file being parsed.
    pub file_path: &'a Path,
}

pub fn extract_all_functions(source: &str, ctx: Option<&FileContext<'_>>) -> Vec<FunctionRef> {
    let Ok(file) = syn::parse_file(source) else { return Vec::new() };
    let imports = signature::collect_imports(&file);
    let classifier = ctx.map_or_else(ImportClassifier::empty, |c| {
        ImportClassifier::build(c.workspace, c.file_path, &file)
    });

    let mut out = Vec::new();
    for item in &file.items {
        match item {
            Item::Fn(f) => push_top_fn(f, &imports, &classifier, &mut out),
            Item::Impl(imp) => push_impl_methods(imp, &imports, &classifier, &mut out),
            _ => {}
        }
    }
    out
}

fn push_top_fn(
    f: &ItemFn,
    imports: &[CompactString],
    classifier: &ImportClassifier<'_>,
    out: &mut Vec<FunctionRef>,
) {
    let name = f.sig.ident.to_string();
    let mut tree = normalize::normalize_block(&f.block, classifier);
    tree.finalize();
    let ast_hash = tree.canonical_hash();
    let calls = signature::collect_calls(&f.block, classifier);
    out.push(FunctionRef {
        symbol: CompactString::from(name),
        kind: FunctionKind::FunctionDeclaration,
        start_line: 0,
        end_line: 0,
        byte_range: (0, 0),
        tree,
        signature: Signature { param_count: u32::try_from(f.sig.inputs.len()).unwrap_or(u32::MAX) },
        calls,
        imports: imports.to_vec(),
        ast_hash,
        ignore: None,
    });
}

fn push_impl_methods(
    imp: &ItemImpl,
    imports: &[CompactString],
    classifier: &ImportClassifier<'_>,
    out: &mut Vec<FunctionRef>,
) {
    let Some(type_name) = impl_type_name(imp) else { return };
    for ii in &imp.items {
        if let ImplItem::Fn(m) = ii {
            push_impl_fn(&type_name, m, imports, classifier, out);
        }
    }
}

fn push_impl_fn(
    type_name: &str,
    m: &ImplItemFn,
    imports: &[CompactString],
    classifier: &ImportClassifier<'_>,
    out: &mut Vec<FunctionRef>,
) {
    let mut tree = normalize::normalize_block(&m.block, classifier);
    tree.finalize();
    let ast_hash = tree.canonical_hash();
    let calls = signature::collect_calls(&m.block, classifier);
    let mut symbol = CompactString::from(type_name);
    symbol.push('.');
    symbol.push_str(&m.sig.ident.to_string());
    out.push(FunctionRef {
        symbol,
        kind: FunctionKind::ClassMethod,
        start_line: 0,
        end_line: 0,
        byte_range: (0, 0),
        tree,
        signature: Signature { param_count: u32::try_from(m.sig.inputs.len()).unwrap_or(u32::MAX) },
        calls,
        imports: imports.to_vec(),
        ast_hash,
        ignore: None,
    });
}

fn impl_type_name(imp: &ItemImpl) -> Option<String> {
    if let syn::Type::Path(tp) = &*imp.self_ty {
        tp.path.segments.last().map(|s| s.ident.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_no_function_when_none_present() {
        let v = extract_all_functions("const X: u32 = 1;", None);
        assert!(v.is_empty());
    }

    #[test]
    fn extracts_single_top_level_fn() {
        let v = extract_all_functions("fn run() { foo(); }", None);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "run");
        assert_eq!(v[0].kind, FunctionKind::FunctionDeclaration);
        assert!(v[0].calls.iter().any(|s| s == "foo"));
    }

    #[test]
    fn captures_param_count() {
        let v = extract_all_functions("fn f(a: u32, b: &str, c: bool) {}", None);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].signature.param_count, 3);
    }

    #[test]
    fn captures_imports_at_file_level() {
        let src = "use foo::bar;\nfn f() {}";
        let v = extract_all_functions(src, None);
        assert_eq!(v.len(), 1);
        assert!(v[0].imports.iter().any(|s| s == "foo"));
        assert!(v[0].imports.iter().any(|s| s == "bar"));
    }

    #[test]
    fn extracts_impl_methods_with_dotted_symbol() {
        let src = "struct Foo; impl Foo { fn list(&self) { self.svc.find_all(); } }";
        let v = extract_all_functions(src, None);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "Foo.list");
        assert_eq!(v[0].kind, FunctionKind::ClassMethod);
    }

    #[test]
    fn extracts_trait_impl_methods() {
        let src = "trait T { fn f(&self); }\nstruct S; impl T for S { fn f(&self) {} }";
        let v = extract_all_functions(src, None);
        assert!(v.iter().any(|f| f.symbol.as_str() == "S.f"));
    }
}
