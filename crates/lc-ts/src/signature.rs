//! Extract calls / imports / signature from oxc AST.

use compact_str::CompactString;
use oxc_ast::ast::{Expression, ImportDeclarationSpecifier, Program, Statement};

/// Walk a function body and collect callee names.
/// Returns a sorted, deduplicated `Vec<CompactString>`.
pub fn collect_calls(body: &[Statement<'_>]) -> Vec<CompactString> {
    let mut acc = Vec::new();
    for s in body {
        walk_statement(s, &mut acc);
    }
    acc.sort();
    acc.dedup();
    acc
}

/// Collect top-level import sources and specifier names from a program.
pub fn collect_imports(program: &Program<'_>) -> Vec<CompactString> {
    let mut acc = Vec::new();
    for s in &program.body {
        if let Statement::ImportDeclaration(imp) = s {
            acc.push(CompactString::from(imp.source.value.as_str()));
            if let Some(specifiers) = &imp.specifiers {
                for spec in specifiers {
                    acc.push(local_name_of_specifier(spec));
                }
            }
        }
    }
    acc.sort();
    acc.dedup();
    acc
}

fn local_name_of_specifier(spec: &ImportDeclarationSpecifier<'_>) -> CompactString {
    match spec {
        ImportDeclarationSpecifier::ImportSpecifier(s) => {
            CompactString::from(s.imported.name().as_str())
        }
        ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
            CompactString::from(s.local.name.as_str())
        }
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
            CompactString::from(s.local.name.as_str())
        }
    }
}

fn walk_statement(s: &Statement<'_>, acc: &mut Vec<CompactString>) {
    match s {
        Statement::ExpressionStatement(es) => walk_expression(&es.expression, acc),
        Statement::ReturnStatement(rs) => {
            if let Some(e) = &rs.argument {
                walk_expression(e, acc);
            }
        }
        Statement::BlockStatement(b) => {
            for s in &b.body {
                walk_statement(s, acc);
            }
        }
        _ => {}
    }
}

fn walk_expression(e: &Expression<'_>, acc: &mut Vec<CompactString>) {
    match e {
        // 括弧式は構造的に意味を持たないため透過させる。
        Expression::ParenthesizedExpression(p) => walk_expression(&p.expression, acc),
        Expression::CallExpression(c) => {
            if let Some(name) = callee_name(&c.callee) {
                acc.push(name);
            }
            for arg in &c.arguments {
                if let Some(inner) = arg.as_expression() {
                    walk_expression(inner, acc);
                }
            }
        }
        _ => {}
    }
}

fn callee_name(e: &Expression<'_>) -> Option<CompactString> {
    match e {
        Expression::ParenthesizedExpression(p) => callee_name(&p.expression),
        Expression::Identifier(id) => Some(CompactString::from(id.name.as_str())),
        Expression::StaticMemberExpression(m) => {
            if let Expression::Identifier(obj) = &m.object {
                let mut s = CompactString::from(obj.name.as_str());
                s.push('.');
                s.push_str(m.property.name.as_str());
                Some(s)
            } else {
                Some(CompactString::from(m.property.name.as_str()))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oxc_compat::parse;
    use oxc_allocator::Allocator;

    #[test]
    fn collects_simple_call() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { useSWR(); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        assert_eq!(collect_calls(body), vec![CompactString::from("useSWR")]);
    }

    #[test]
    fn collects_member_call_as_dotted() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { axios.get(); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        assert_eq!(collect_calls(body), vec![CompactString::from("axios.get")]);
    }

    #[test]
    fn collects_calls_sorted_and_deduped() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { b(); a(); a(); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        assert_eq!(
            collect_calls(body),
            vec![CompactString::from("a"), CompactString::from("b")]
        );
    }

    #[test]
    fn collects_imports_with_specifiers() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "import { useSWR } from 'swr'; function f() {}");
        let imports = collect_imports(&ret.program);
        assert_eq!(
            imports,
            vec![CompactString::from("swr"), CompactString::from("useSWR")]
        );
    }
}
