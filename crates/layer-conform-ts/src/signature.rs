//! Extract calls / imports / signature from oxc AST.

use compact_str::CompactString;
use oxc_ast::ast::{
    Expression, ForStatementInit, ImportDeclarationSpecifier, ObjectPropertyKind, Program,
    PropertyKey, Statement,
};

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
        Statement::VariableDeclaration(decl) => {
            for declarator in &decl.declarations {
                if let Some(init) = &declarator.init {
                    walk_expression(init, acc);
                }
            }
        }
        Statement::IfStatement(s) => {
            walk_expression(&s.test, acc);
            walk_statement(&s.consequent, acc);
            if let Some(alt) = &s.alternate {
                walk_statement(alt, acc);
            }
        }
        Statement::ForStatement(s) => {
            if let Some(init) = &s.init {
                match init {
                    ForStatementInit::VariableDeclaration(decl) => {
                        for declarator in &decl.declarations {
                            if let Some(e) = &declarator.init {
                                walk_expression(e, acc);
                            }
                        }
                    }
                    other => {
                        if let Some(e) = other.as_expression() {
                            walk_expression(e, acc);
                        }
                    }
                }
            }
            if let Some(test) = &s.test {
                walk_expression(test, acc);
            }
            if let Some(update) = &s.update {
                walk_expression(update, acc);
            }
            walk_statement(&s.body, acc);
        }
        Statement::ForOfStatement(s) => {
            walk_expression(&s.right, acc);
            walk_statement(&s.body, acc);
        }
        Statement::ForInStatement(s) => {
            walk_expression(&s.right, acc);
            walk_statement(&s.body, acc);
        }
        Statement::WhileStatement(s) => {
            walk_expression(&s.test, acc);
            walk_statement(&s.body, acc);
        }
        Statement::DoWhileStatement(s) => {
            walk_expression(&s.test, acc);
            walk_statement(&s.body, acc);
        }
        Statement::TryStatement(s) => {
            for stmt in &s.block.body {
                walk_statement(stmt, acc);
            }
            if let Some(handler) = &s.handler {
                for stmt in &handler.body.body {
                    walk_statement(stmt, acc);
                }
            }
            if let Some(finalizer) = &s.finalizer {
                for stmt in &finalizer.body {
                    walk_statement(stmt, acc);
                }
            }
        }
        Statement::ThrowStatement(s) => {
            walk_expression(&s.argument, acc);
        }
        Statement::SwitchStatement(s) => {
            walk_expression(&s.discriminant, acc);
            for case in &s.cases {
                if let Some(test) = &case.test {
                    walk_expression(test, acc);
                }
                for stmt in &case.consequent {
                    walk_statement(stmt, acc);
                }
            }
        }
        Statement::LabeledStatement(s) => walk_statement(&s.body, acc),
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
            // Recurse into the callee subexpression to capture nested calls
            // (e.g. `(getFn())()` or `obj.foo().bar()`).
            walk_expression(&c.callee, acc);
            for arg in &c.arguments {
                if let Some(inner) = arg.as_expression() {
                    walk_expression(inner, acc);
                }
            }
        }
        Expression::NewExpression(e) => {
            walk_expression(&e.callee, acc);
            for arg in &e.arguments {
                if let Some(inner) = arg.as_expression() {
                    walk_expression(inner, acc);
                }
            }
        }
        Expression::AwaitExpression(e) => walk_expression(&e.argument, acc),
        Expression::AssignmentExpression(e) => walk_expression(&e.right, acc),
        Expression::BinaryExpression(e) => {
            walk_expression(&e.left, acc);
            walk_expression(&e.right, acc);
        }
        Expression::LogicalExpression(e) => {
            walk_expression(&e.left, acc);
            walk_expression(&e.right, acc);
        }
        Expression::ConditionalExpression(e) => {
            walk_expression(&e.test, acc);
            walk_expression(&e.consequent, acc);
            walk_expression(&e.alternate, acc);
        }
        Expression::SequenceExpression(e) => {
            for inner in &e.expressions {
                walk_expression(inner, acc);
            }
        }
        Expression::UnaryExpression(e) => walk_expression(&e.argument, acc),
        Expression::ArrayExpression(e) => {
            for elem in &e.elements {
                if let Some(inner) = elem.as_expression() {
                    walk_expression(inner, acc);
                }
            }
        }
        Expression::ObjectExpression(e) => {
            for prop in &e.properties {
                if let ObjectPropertyKind::ObjectProperty(p) = prop {
                    walk_expression(&p.value, acc);
                    // Computed keys may contain calls.
                    if p.computed {
                        if let Some(inner) = property_key_as_expression(&p.key) {
                            walk_expression(inner, acc);
                        }
                    }
                }
            }
        }
        Expression::ArrowFunctionExpression(e) => {
            for stmt in &e.body.statements {
                walk_statement(stmt, acc);
            }
        }
        Expression::FunctionExpression(e) => {
            if let Some(body) = &e.body {
                for stmt in &body.statements {
                    walk_statement(stmt, acc);
                }
            }
        }
        Expression::TemplateLiteral(e) => {
            for inner in &e.expressions {
                walk_expression(inner, acc);
            }
        }
        Expression::TaggedTemplateExpression(e) => {
            walk_expression(&e.tag, acc);
            for inner in &e.quasi.expressions {
                walk_expression(inner, acc);
            }
        }
        Expression::StaticMemberExpression(e) => walk_expression(&e.object, acc),
        Expression::ComputedMemberExpression(e) => {
            walk_expression(&e.object, acc);
            walk_expression(&e.expression, acc);
        }
        Expression::PrivateFieldExpression(e) => walk_expression(&e.object, acc),
        Expression::ChainExpression(e) => {
            // Chain wraps either a call or a member expression.
            use oxc_ast::ast::ChainElement;
            match &e.expression {
                ChainElement::CallExpression(c) => {
                    if let Some(name) = callee_name(&c.callee) {
                        acc.push(name);
                    }
                    walk_expression(&c.callee, acc);
                    for arg in &c.arguments {
                        if let Some(inner) = arg.as_expression() {
                            walk_expression(inner, acc);
                        }
                    }
                }
                ChainElement::ComputedMemberExpression(m) => {
                    walk_expression(&m.object, acc);
                    walk_expression(&m.expression, acc);
                }
                ChainElement::StaticMemberExpression(m) => walk_expression(&m.object, acc),
                ChainElement::PrivateFieldExpression(m) => walk_expression(&m.object, acc),
                _ => {}
            }
        }
        Expression::TSAsExpression(e) => walk_expression(&e.expression, acc),
        Expression::TSSatisfiesExpression(e) => walk_expression(&e.expression, acc),
        Expression::TSNonNullExpression(e) => walk_expression(&e.expression, acc),
        Expression::TSTypeAssertion(e) => walk_expression(&e.expression, acc),
        Expression::TSInstantiationExpression(e) => walk_expression(&e.expression, acc),
        Expression::YieldExpression(e) => {
            if let Some(arg) = &e.argument {
                walk_expression(arg, acc);
            }
        }
        _ => {}
    }
}

fn property_key_as_expression<'a, 'b>(key: &'b PropertyKey<'a>) -> Option<&'b Expression<'a>> {
    // PropertyKey inherits Expression variants; map back when computed.
    key.as_expression()
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

    #[test]
    fn collects_calls_inside_variable_declaration() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { const x = useSWR('/u'); return x; }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        let calls = collect_calls(body);
        assert!(calls.contains(&CompactString::from("useSWR")));
    }

    #[test]
    fn collects_calls_inside_chained_member_calls() {
        let alloc = Allocator::default();
        let ret = parse(
            &alloc,
            "function f() { return this.prisma.getClient().equipment.findMany(); }",
        );
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        let calls = collect_calls(body);
        // The outermost call's callee is `<something>.findMany`, so we get e.g. `equipment.findMany`.
        // Also the inner `getClient` call should be picked up via the recursion.
        assert!(calls.iter().any(|s| s.contains("findMany")));
        assert!(calls.iter().any(|s| s.contains("getClient")));
    }

    #[test]
    fn collects_calls_inside_if_and_for() {
        let alloc = Allocator::default();
        let ret = parse(
            &alloc,
            "function f() { if (cond()) { foo(); } for (let i=0;i<n();i++) { bar(); } }",
        );
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        let calls = collect_calls(body);
        assert!(calls.iter().any(|s| s == "cond"));
        assert!(calls.iter().any(|s| s == "foo"));
        assert!(calls.iter().any(|s| s == "n"));
        assert!(calls.iter().any(|s| s == "bar"));
    }

    #[test]
    fn collects_calls_inside_arrow_in_arg() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { arr.map(x => bar(x)); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        let calls = collect_calls(body);
        assert!(calls.iter().any(|s| s.contains("map")));
        assert!(calls.iter().any(|s| s == "bar"));
    }
}
