//! Convert oxc AST nodes into the neutral `lc_core::tree::TreeNode` IR.

use compact_str::CompactString;
use lc_core::tree::{NodeKind, TreeNode};
use oxc_ast::ast::{Expression, Statement};

const ANON_IDENT: &str = "_IDENT";
const ANON_LIT: &str = "_LIT";

/// Convert a function body (statement list) into a Block subtree.
pub fn normalize_block(body: &[Statement<'_>]) -> TreeNode {
    let children: Vec<TreeNode> = body.iter().map(normalize_statement).collect();
    let mut node = TreeNode::branch(NodeKind::Block, children);
    node.finalize();
    node
}

fn normalize_statement(stmt: &Statement<'_>) -> TreeNode {
    match stmt {
        Statement::ExpressionStatement(es) => normalize_expression(&es.expression),
        Statement::ReturnStatement(rs) => {
            let children = rs
                .argument
                .as_ref()
                .map(|e| vec![normalize_expression(e)])
                .unwrap_or_default();
            TreeNode::branch(NodeKind::Other, children)
        }
        _ => TreeNode::branch(NodeKind::Other, Vec::new()),
    }
}

fn normalize_expression(expr: &Expression<'_>) -> TreeNode {
    match expr {
        // 括弧式は構造的に意味を持たないため透過させる。
        Expression::ParenthesizedExpression(p) => normalize_expression(&p.expression),
        Expression::CallExpression(c) => {
            let mut children = vec![normalize_callee(&c.callee)];
            for arg in &c.arguments {
                if let Some(e) = arg.as_expression() {
                    children.push(normalize_expression(e));
                } else {
                    children.push(TreeNode::leaf(NodeKind::Other, None));
                }
            }
            TreeNode::branch(NodeKind::CallExpression, children)
        }
        Expression::Identifier(_) => {
            TreeNode::leaf(NodeKind::Identifier, Some(CompactString::new(ANON_IDENT)))
        }
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::TemplateLiteral(_) => {
            TreeNode::leaf(NodeKind::Literal, Some(CompactString::new(ANON_LIT)))
        }
        Expression::BooleanLiteral(b) => TreeNode::leaf(
            NodeKind::Literal,
            Some(CompactString::new(if b.value { "true" } else { "false" })),
        ),
        Expression::NullLiteral(_) => {
            TreeNode::leaf(NodeKind::Literal, Some(CompactString::new("null")))
        }
        _ => TreeNode::leaf(NodeKind::Other, None),
    }
}

fn normalize_callee(callee: &Expression<'_>) -> TreeNode {
    match callee {
        Expression::Identifier(id) => {
            TreeNode::leaf(NodeKind::Identifier, Some(CompactString::from(id.name.as_str())))
        }
        Expression::StaticMemberExpression(m) => {
            let object = match &m.object {
                Expression::Identifier(id) => TreeNode::leaf(
                    NodeKind::Identifier,
                    Some(CompactString::from(id.name.as_str())),
                ),
                other => normalize_expression(other),
            };
            let property = TreeNode::leaf(
                NodeKind::Identifier,
                Some(CompactString::from(m.property.name.as_str())),
            );
            TreeNode::branch(NodeKind::MemberExpression, vec![object, property])
        }
        other => normalize_expression(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oxc_compat::parse;
    use oxc_allocator::Allocator;

    #[test]
    fn empty_body_yields_block_with_zero_children() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() {}");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else {
            panic!("expected FunctionDeclaration");
        };
        let body = &f.body.as_ref().unwrap().statements;
        let tree = normalize_block(body);
        assert_eq!(tree.kind, NodeKind::Block);
        assert_eq!(tree.children.len(), 0);
        assert_eq!(tree.subtree_size, 1);
    }

    #[test]
    fn call_expression_preserves_callee_name() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { useSWR(); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        let tree = normalize_block(body);
        assert_eq!(tree.children.len(), 1);
        let call = &tree.children[0];
        assert_eq!(call.kind, NodeKind::CallExpression);
        assert_eq!(call.children[0].kind, NodeKind::Identifier);
        assert_eq!(call.children[0].value.as_deref(), Some("useSWR"));
    }

    #[test]
    fn member_call_preserves_both_names() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { axios.get(); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        let tree = normalize_block(body);
        let call = &tree.children[0];
        assert_eq!(call.children[0].kind, NodeKind::MemberExpression);
        assert_eq!(call.children[0].children[0].value.as_deref(), Some("axios"));
        assert_eq!(call.children[0].children[1].value.as_deref(), Some("get"));
    }

    #[test]
    fn local_identifier_is_anonymized() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { x; }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        let tree = normalize_block(body);
        let id = &tree.children[0];
        assert_eq!(id.kind, NodeKind::Identifier);
        assert_eq!(id.value.as_deref(), Some(ANON_IDENT));
    }

    #[test]
    fn string_literal_is_anonymized() {
        // 関数本体先頭の単独 StringLiteral は ECMAScript の Directive Prologue として
        // 解釈され Statement にならないため、評価式として認識される位置に置く。
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { ('hello'); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        let tree = normalize_block(body);
        let lit = &tree.children[0];
        assert_eq!(lit.kind, NodeKind::Literal);
        assert_eq!(lit.value.as_deref(), Some(ANON_LIT));
    }
}
