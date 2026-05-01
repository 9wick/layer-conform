//! Extract calls / imports from `syn::File` and `syn::Block`.
//!
//! Calls go through the optional [`ImportClassifier`]: in workspace-aware
//! mode each entry is encoded as `_LAYER:..`, `_PKG:lc_core`, `_STDLIB`,
//! etc.; without a workspace we fall back to the raw dotted name so the
//! lc-rs unit tests keep passing.

use compact_str::CompactString;
use syn::{Block, Expr, File, Item, Path, Stmt, UseTree};

use crate::resolver::ImportClassifier;

/// Walk a function body and collect callee labels.
/// Returns a sorted, deduplicated `Vec<CompactString>`.
pub fn collect_calls(block: &Block, c: &ImportClassifier<'_>) -> Vec<CompactString> {
    let mut acc = Vec::new();
    for s in &block.stmts {
        walk_stmt(s, c, &mut acc);
    }
    acc.sort();
    acc.dedup();
    acc
}

/// Collect top-level `use` paths' segment names.
pub fn collect_imports(file: &File) -> Vec<CompactString> {
    let mut acc = Vec::new();
    for item in &file.items {
        if let Item::Use(u) = item {
            walk_use_tree(&u.tree, &mut acc);
        }
    }
    acc.sort();
    acc.dedup();
    acc
}

fn walk_use_tree(t: &UseTree, acc: &mut Vec<CompactString>) {
    match t {
        UseTree::Path(p) => {
            acc.push(CompactString::from(p.ident.to_string()));
            walk_use_tree(&p.tree, acc);
        }
        UseTree::Name(n) => acc.push(CompactString::from(n.ident.to_string())),
        UseTree::Rename(r) => acc.push(CompactString::from(r.rename.to_string())),
        UseTree::Group(g) => {
            for inner in &g.items {
                walk_use_tree(inner, acc);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn walk_stmt(s: &Stmt, c: &ImportClassifier<'_>, acc: &mut Vec<CompactString>) {
    match s {
        Stmt::Expr(e, _) => walk_expr(e, c, acc),
        Stmt::Local(l) => {
            if let Some(init) = &l.init {
                walk_expr(&init.expr, c, acc);
                if let Some((_, diverge)) = &init.diverge {
                    walk_expr(diverge, c, acc);
                }
            }
        }
        Stmt::Macro(m) => acc.push(c.classify_macro(&m.mac.path)),
        Stmt::Item(_) => {}
    }
}

fn walk_expr(e: &Expr, c: &ImportClassifier<'_>, acc: &mut Vec<CompactString>) {
    match e {
        Expr::Paren(p) => walk_expr(&p.expr, c, acc),
        Expr::Try(t) => walk_expr(&t.expr, c, acc),
        Expr::Call(call) => {
            if let Some(label) = call_label(&call.func, c) {
                acc.push(label);
            }
            walk_expr(&call.func, c, acc);
            for a in &call.args {
                walk_expr(a, c, acc);
            }
        }
        Expr::MethodCall(mc) => {
            acc.push(c.classify_method(receiver_path(&mc.receiver), &mc.method.to_string()));
            walk_expr(&mc.receiver, c, acc);
            for a in &mc.args {
                walk_expr(a, c, acc);
            }
        }
        Expr::Macro(m) => acc.push(c.classify_macro(&m.mac.path)),
        Expr::Block(b) => {
            for s in &b.block.stmts {
                walk_stmt(s, c, acc);
            }
        }
        Expr::If(i) => {
            walk_expr(&i.cond, c, acc);
            for s in &i.then_branch.stmts {
                walk_stmt(s, c, acc);
            }
            if let Some((_, else_expr)) = &i.else_branch {
                walk_expr(else_expr, c, acc);
            }
        }
        Expr::Match(m) => {
            walk_expr(&m.expr, c, acc);
            for arm in &m.arms {
                if let Some((_, guard)) = &arm.guard {
                    walk_expr(guard, c, acc);
                }
                walk_expr(&arm.body, c, acc);
            }
        }
        Expr::ForLoop(f) => {
            walk_expr(&f.expr, c, acc);
            for s in &f.body.stmts {
                walk_stmt(s, c, acc);
            }
        }
        Expr::While(w) => {
            walk_expr(&w.cond, c, acc);
            for s in &w.body.stmts {
                walk_stmt(s, c, acc);
            }
        }
        Expr::Loop(l) => {
            for s in &l.body.stmts {
                walk_stmt(s, c, acc);
            }
        }
        Expr::Binary(b) => {
            walk_expr(&b.left, c, acc);
            walk_expr(&b.right, c, acc);
        }
        Expr::Unary(u) => walk_expr(&u.expr, c, acc),
        Expr::Reference(r) => walk_expr(&r.expr, c, acc),
        Expr::Field(f) => walk_expr(&f.base, c, acc),
        Expr::Await(a) => walk_expr(&a.base, c, acc),
        Expr::Return(r) => {
            if let Some(e) = &r.expr {
                walk_expr(e, c, acc);
            }
        }
        Expr::Tuple(t) => {
            for e in &t.elems {
                walk_expr(e, c, acc);
            }
        }
        Expr::Array(a) => {
            for e in &a.elems {
                walk_expr(e, c, acc);
            }
        }
        Expr::Cast(cst) => walk_expr(&cst.expr, c, acc),
        Expr::Closure(cl) => walk_expr(&cl.body, c, acc),
        Expr::Let(l) => walk_expr(&l.expr, c, acc),
        Expr::Range(r) => {
            if let Some(s) = &r.start {
                walk_expr(s, c, acc);
            }
            if let Some(e) = &r.end {
                walk_expr(e, c, acc);
            }
        }
        Expr::Assign(a) => walk_expr(&a.right, c, acc),
        Expr::Index(i) => {
            walk_expr(&i.expr, c, acc);
            walk_expr(&i.index, c, acc);
        }
        Expr::Struct(s) => {
            for field in &s.fields {
                walk_expr(&field.expr, c, acc);
            }
            if let Some(rest) = &s.rest {
                walk_expr(rest, c, acc);
            }
        }
        _ => {}
    }
}

fn call_label(callee: &Expr, c: &ImportClassifier<'_>) -> Option<CompactString> {
    match callee {
        Expr::Paren(p) => call_label(&p.expr, c),
        Expr::Path(p) => Some(c.classify_call(&p.path)),
        _ => None,
    }
}

fn receiver_path(expr: &Expr) -> Option<&Path> {
    if let Expr::Path(p) = expr {
        return Some(&p.path);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_block(src: &str) -> Block {
        let f: syn::ItemFn = syn::parse_str(src).unwrap();
        *f.block
    }

    #[test]
    fn collects_simple_call() {
        let b = parse_block("fn f() { use_swr(); }");
        assert_eq!(
            collect_calls(&b, &ImportClassifier::empty()),
            vec![CompactString::from("use_swr")]
        );
    }

    #[test]
    fn collects_method_call_as_dotted() {
        let b = parse_block("fn f() { axios.get(); }");
        assert_eq!(
            collect_calls(&b, &ImportClassifier::empty()),
            vec![CompactString::from("axios.get")]
        );
    }

    #[test]
    fn collects_macro_call_with_bang_suffix() {
        let b = parse_block("fn f() { println!(\"x\"); }");
        assert_eq!(
            collect_calls(&b, &ImportClassifier::empty()),
            vec![CompactString::from("println!")]
        );
    }

    #[test]
    fn collects_calls_inside_let_init() {
        let b = parse_block("fn f() { let x = use_swr(\"/u\"); }");
        let calls = collect_calls(&b, &ImportClassifier::empty());
        assert!(calls.iter().any(|s| s == "use_swr"));
    }

    #[test]
    fn collects_calls_inside_if() {
        let b = parse_block("fn f() { if cond() { foo(); } }");
        let calls = collect_calls(&b, &ImportClassifier::empty());
        assert!(calls.iter().any(|s| s == "cond"));
        assert!(calls.iter().any(|s| s == "foo"));
    }

    #[test]
    fn collects_calls_through_try_op() {
        let b = parse_block("fn f() { foo()?; }");
        assert!(collect_calls(&b, &ImportClassifier::empty()).iter().any(|s| s == "foo"));
    }

    #[test]
    fn collects_imports_with_group_and_path() {
        let src = "use foo::{bar, baz};\nuse other::Renamed as R;\nfn f() {}";
        let file: File = syn::parse_str(src).unwrap();
        let imports = collect_imports(&file);
        assert!(imports.iter().any(|s| s == "foo"));
        assert!(imports.iter().any(|s| s == "bar"));
        assert!(imports.iter().any(|s| s == "baz"));
        assert!(imports.iter().any(|s| s == "R"));
    }
}
