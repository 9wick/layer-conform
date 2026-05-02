//! Convert syn AST nodes into the neutral `layer_conform_core::tree::TreeNode` IR.
//!
//! The optional [`ImportClassifier`] re-encodes call/method/macro identifiers
//! as `_LAYER:../`, `_PKG:layer_conform_core`, `_STDLIB`, etc. so that two functions
//! that delegate across the same boundary get identical leaf values even
//! when their concrete callee names differ.

use compact_str::CompactString;
use layer_conform_core::tree::{NodeKind, TreeNode};
use syn::{Block, Expr, Member, Path, Stmt};

use crate::resolver::ImportClassifier;

const ANON_IDENT: &str = "_IDENT";
const ANON_LIT: &str = "_LIT";

/// Convert a function body (`syn::Block`) into a Block subtree.
pub fn normalize_block(block: &Block, c: &ImportClassifier<'_>) -> TreeNode {
    let children: Vec<TreeNode> = block.stmts.iter().map(|s| normalize_statement(s, c)).collect();
    let mut node = TreeNode::branch(NodeKind::Block, children);
    node.finalize();
    node
}

fn normalize_statement(stmt: &Stmt, c: &ImportClassifier<'_>) -> TreeNode {
    match stmt {
        Stmt::Expr(e, _) => normalize_expression(e, c),
        Stmt::Local(l) => {
            let children = l
                .init
                .as_ref()
                .map(|init| vec![normalize_expression(&init.expr, c)])
                .unwrap_or_default();
            TreeNode::branch(NodeKind::Other, children)
        }
        Stmt::Macro(m) => normalize_macro_call(&m.mac.path, c),
        Stmt::Item(_) => TreeNode::branch(NodeKind::Other, Vec::new()),
    }
}

fn normalize_expression(expr: &Expr, c: &ImportClassifier<'_>) -> TreeNode {
    match expr {
        Expr::Paren(p) => normalize_expression(&p.expr, c),
        Expr::Try(t) => normalize_expression(&t.expr, c),
        Expr::Call(call) => {
            let mut children = vec![normalize_callee(&call.func, c)];
            for arg in &call.args {
                children.push(normalize_expression(arg, c));
            }
            TreeNode::branch(NodeKind::CallExpression, children)
        }
        Expr::MethodCall(mc) => {
            let object = normalize_expression(&mc.receiver, c);
            let property_value = c.classify_method(receiver_path(&mc.receiver), &mc.method.to_string());
            let property = TreeNode::leaf(NodeKind::Identifier, Some(property_value));
            let callee = TreeNode::branch(NodeKind::MemberExpression, vec![object, property]);
            let mut children = vec![callee];
            for arg in &mc.args {
                children.push(normalize_expression(arg, c));
            }
            TreeNode::branch(NodeKind::CallExpression, children)
        }
        Expr::Macro(m) => normalize_macro_call(&m.mac.path, c),
        Expr::Path(p) => normalize_path(&p.path, c),
        Expr::Lit(_) => TreeNode::leaf(NodeKind::Literal, Some(CompactString::new(ANON_LIT))),
        Expr::Block(b) => {
            let kids: Vec<TreeNode> = b.block.stmts.iter().map(|s| normalize_statement(s, c)).collect();
            TreeNode::branch(NodeKind::Block, kids)
        }
        Expr::If(i) => {
            let cond = normalize_expression(&i.cond, c);
            let then_kids: Vec<TreeNode> =
                i.then_branch.stmts.iter().map(|s| normalize_statement(s, c)).collect();
            let then_blk = TreeNode::branch(NodeKind::Block, then_kids);
            let mut kids = vec![cond, then_blk];
            if let Some((_, else_expr)) = &i.else_branch {
                kids.push(normalize_expression(else_expr, c));
            }
            TreeNode::branch(NodeKind::Other, kids)
        }
        Expr::Match(m) => {
            let mut kids = vec![normalize_expression(&m.expr, c)];
            for arm in &m.arms {
                kids.push(normalize_expression(&arm.body, c));
            }
            TreeNode::branch(NodeKind::Other, kids)
        }
        Expr::Field(f) => {
            let object = normalize_expression(&f.base, c);
            let prop = match &f.member {
                Member::Named(id) => id.to_string(),
                Member::Unnamed(idx) => idx.index.to_string(),
            };
            let property = TreeNode::leaf(NodeKind::Identifier, Some(CompactString::from(prop)));
            TreeNode::branch(NodeKind::MemberExpression, vec![object, property])
        }
        Expr::Reference(r) => normalize_expression(&r.expr, c),
        Expr::Await(a) => normalize_expression(&a.base, c),
        Expr::Return(r) => {
            let kids = r
                .expr
                .as_ref()
                .map(|e| vec![normalize_expression(e, c)])
                .unwrap_or_default();
            TreeNode::branch(NodeKind::Other, kids)
        }
        _ => TreeNode::leaf(NodeKind::Other, None),
    }
}

fn normalize_callee(expr: &Expr, c: &ImportClassifier<'_>) -> TreeNode {
    if let Expr::Path(p) = expr {
        let value = c.classify_call(&p.path);
        return TreeNode::leaf(NodeKind::Identifier, Some(value));
    }
    normalize_expression(expr, c)
}

fn normalize_path(path: &Path, c: &ImportClassifier<'_>) -> TreeNode {
    if path.segments.is_empty() {
        return TreeNode::leaf(NodeKind::Identifier, Some(CompactString::new(ANON_IDENT)));
    }
    // In enabled (workspace-aware) mode, fold *any* path reference into a
    // single leaf carrying the origin label — locals collapse to `_IDENT`.
    if c.enabled() {
        let value = c
            .resolve_path(path)
            .map_or_else(|| CompactString::new(ANON_IDENT), |o| o.encode());
        return TreeNode::leaf(NodeKind::Identifier, Some(value));
    }
    // Disabled (no workspace) mode: preserve raw structure for backwards
    // compatibility with layer-conform-rs unit tests.
    if path.segments.len() == 1 {
        let name = path.segments[0].ident.to_string();
        return TreeNode::leaf(NodeKind::Identifier, Some(CompactString::from(name)));
    }
    let mut node: Option<TreeNode> = None;
    for seg in &path.segments {
        let leaf = TreeNode::leaf(
            NodeKind::Identifier,
            Some(CompactString::from(seg.ident.to_string())),
        );
        node = Some(match node {
            None => leaf,
            Some(prev) => TreeNode::branch(NodeKind::MemberExpression, vec![prev, leaf]),
        });
    }
    node.unwrap_or_else(|| TreeNode::leaf(NodeKind::Identifier, Some(CompactString::new(ANON_IDENT))))
}

fn normalize_macro_call(path: &Path, c: &ImportClassifier<'_>) -> TreeNode {
    let value = c.classify_macro(path);
    let callee = TreeNode::leaf(NodeKind::Identifier, Some(value));
    TreeNode::branch(NodeKind::CallExpression, vec![callee])
}

/// Extract the receiver as a `syn::Path` when the receiver is a single-segment
/// path expression (i.e. `loader.foo()` with `loader` imported). Returns None
/// when the receiver is anything else (a local value, a chained call, etc.).
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
    fn empty_body_yields_block_with_zero_children() {
        let b = parse_block("fn f() {}");
        let tree = normalize_block(&b, &ImportClassifier::empty());
        assert_eq!(tree.kind, NodeKind::Block);
        assert_eq!(tree.children.len(), 0);
        assert_eq!(tree.subtree_size, 1);
    }

    #[test]
    fn call_expression_preserves_callee_name_in_disabled_mode() {
        let b = parse_block("fn f() { use_swr(); }");
        let tree = normalize_block(&b, &ImportClassifier::empty());
        assert_eq!(tree.children.len(), 1);
        let call = &tree.children[0];
        assert_eq!(call.kind, NodeKind::CallExpression);
        assert_eq!(call.children[0].kind, NodeKind::Identifier);
        assert_eq!(call.children[0].value.as_deref(), Some("use_swr"));
    }

    #[test]
    fn method_call_uses_member_expression_callee() {
        let b = parse_block("fn f() { axios.get(); }");
        let tree = normalize_block(&b, &ImportClassifier::empty());
        let call = &tree.children[0];
        assert_eq!(call.kind, NodeKind::CallExpression);
        assert_eq!(call.children[0].kind, NodeKind::MemberExpression);
        assert_eq!(call.children[0].children[0].value.as_deref(), Some("axios"));
        // Method node carries the dotted call name so tree and calls bag
        // are consistent.
        assert_eq!(call.children[0].children[1].value.as_deref(), Some("axios.get"));
    }

    #[test]
    fn macro_call_records_path_in_disabled_mode() {
        let b = parse_block("fn f() { println!(\"x\"); }");
        let tree = normalize_block(&b, &ImportClassifier::empty());
        let call = &tree.children[0];
        assert_eq!(call.kind, NodeKind::CallExpression);
        assert_eq!(call.children[0].value.as_deref(), Some("println!"));
    }
}
