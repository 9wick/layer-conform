//! Extract every supported function kind from a TS/JS source.
//!
//! All emit sites flow through `Builder::build` so that `start_line`,
//! `end_line`, `ast_hash`, and the `ignore` directive are computed in one
//! place — which is also the only place that knows how to walk source bytes.

use compact_str::CompactString;
use lc_core::ignore_parse::{CommentKind as IgKind, CommentToken};
use lc_core::{FunctionKind, FunctionRef, Signature};
use oxc_ast::ast::{
    BindingPatternKind, Class, ClassElement, Expression, ExportDefaultDeclarationKind,
    MethodDefinitionKind, ObjectExpression, ObjectPropertyKind, PropertyKey, Statement,
    VariableDeclaration,
};

use crate::{normalize, signature};

/// Extract every top-level function (declarations, arrows, methods, defaults).
pub fn extract_all_functions(source: &str) -> Vec<FunctionRef> {
    let alloc = oxc_allocator::Allocator::default();
    let ret = crate::oxc_compat::parse(&alloc, source);
    let program = &ret.program;
    let imports = signature::collect_imports(program);
    let comments = collect_comment_tokens(source, &program.comments);
    let builder = Builder { source, imports, comments };

    let mut out = Vec::new();
    for stmt in &program.body {
        match stmt {
            Statement::FunctionDeclaration(decl) => {
                if let Some(name) = decl.id.as_ref().map(|i| i.name.as_str()) {
                    let body = decl.body.as_ref().map(|b| b.statements.as_slice()).unwrap_or(&[]);
                    out.push(builder.build(
                        CompactString::from(name),
                        FunctionKind::FunctionDeclaration,
                        body,
                        decl.params.items.len(),
                        decl.span.start,
                        decl.span.end,
                    ));
                }
            }
            Statement::ClassDeclaration(class) => extract_class(class, &builder, &mut out),
            Statement::VariableDeclaration(decl) => extract_variable(decl, &builder, &mut out),
            Statement::ExportNamedDeclaration(exp) => {
                if let Some(decl) = &exp.declaration {
                    use oxc_ast::ast::Declaration;
                    match decl {
                        Declaration::FunctionDeclaration(f) => {
                            if let Some(name) = f.id.as_ref().map(|i| i.name.as_str()) {
                                let body =
                                    f.body.as_ref().map(|b| b.statements.as_slice()).unwrap_or(&[]);
                                out.push(builder.build(
                                    CompactString::from(name),
                                    FunctionKind::FunctionDeclaration,
                                    body,
                                    f.params.items.len(),
                                    f.span.start,
                                    f.span.end,
                                ));
                            }
                        }
                        Declaration::ClassDeclaration(class) => {
                            extract_class(class, &builder, &mut out);
                        }
                        Declaration::VariableDeclaration(vd) => {
                            extract_variable(vd, &builder, &mut out);
                        }
                        _ => {}
                    }
                }
            }
            Statement::ExportDefaultDeclaration(exp) => {
                extract_default_export(&exp.declaration, &builder, &mut out);
            }
            _ => {}
        }
    }
    out
}

// --- Builder: single emit site for every FunctionRef -------------------------

struct Builder<'a> {
    source: &'a str,
    imports: Vec<CompactString>,
    comments: Vec<CommentToken>,
}

impl Builder<'_> {
    fn build(
        &self,
        symbol: CompactString,
        kind: FunctionKind,
        body: &[Statement<'_>],
        param_count: usize,
        span_start: u32,
        span_end: u32,
    ) -> FunctionRef {
        let tree = normalize::normalize_block(body);
        let ast_hash = tree.canonical_hash();
        let start_line = byte_to_line(self.source, span_start);
        let end_line = byte_to_line(self.source, span_end);
        let ignore = lc_core::ignore_parse::parse_directive(&self.comments, start_line);
        FunctionRef {
            symbol,
            kind,
            start_line,
            end_line,
            byte_range: (span_start, span_end),
            tree,
            signature: Signature { param_count: param_count as u32 },
            calls: signature::collect_calls(body),
            imports: self.imports.clone(),
            ast_hash,
            ignore,
        }
    }
}

fn byte_to_line(source: &str, byte_offset: u32) -> u32 {
    let cap = (byte_offset as usize).min(source.len());
    source.as_bytes()[..cap].iter().filter(|b| **b == b'\n').count() as u32 + 1
}

fn collect_comment_tokens(source: &str, comments: &[oxc_ast::Comment]) -> Vec<CommentToken> {
    comments
        .iter()
        .filter_map(|c| {
            let start = c.span.start as usize;
            let end = c.span.end as usize;
            let raw = source.get(start..end)?;
            let (kind, text) = if let Some(rest) = raw.strip_prefix("/**") {
                (IgKind::Doc, rest.strip_suffix("*/").unwrap_or(rest).to_string())
            } else if let Some(rest) = raw.strip_prefix("/*") {
                (IgKind::Block, rest.strip_suffix("*/").unwrap_or(rest).to_string())
            } else {
                let rest = raw.strip_prefix("//")?;
                (IgKind::Line, rest.to_string())
            };
            Some(CommentToken { text, kind, end_line: byte_to_line(source, c.span.end) })
        })
        .collect()
}

// --- Per-kind extractors ------------------------------------------------------

fn extract_default_export(
    declaration: &ExportDefaultDeclarationKind<'_>,
    builder: &Builder<'_>,
    out: &mut Vec<FunctionRef>,
) {
    let symbol = CompactString::from("default");
    match declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
            let body = f.body.as_ref().map(|b| b.statements.as_slice()).unwrap_or(&[]);
            out.push(builder.build(
                symbol,
                FunctionKind::DefaultExportFunction,
                body,
                f.params.items.len(),
                f.span.start,
                f.span.end,
            ));
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            extract_class(class, builder, out);
        }
        ExportDefaultDeclarationKind::ArrowFunctionExpression(arrow) => {
            out.push(builder.build(
                symbol,
                FunctionKind::DefaultExportFunction,
                arrow.body.statements.as_slice(),
                arrow.params.items.len(),
                arrow.span.start,
                arrow.span.end,
            ));
        }
        ExportDefaultDeclarationKind::FunctionExpression(fn_expr) => {
            let body = fn_expr.body.as_ref().map(|b| b.statements.as_slice()).unwrap_or(&[]);
            out.push(builder.build(
                symbol,
                FunctionKind::DefaultExportFunction,
                body,
                fn_expr.params.items.len(),
                fn_expr.span.start,
                fn_expr.span.end,
            ));
        }
        _ => {}
    }
}

fn extract_variable(
    decl: &VariableDeclaration<'_>,
    builder: &Builder<'_>,
    out: &mut Vec<FunctionRef>,
) {
    for declarator in &decl.declarations {
        let BindingPatternKind::BindingIdentifier(name_id) = &declarator.id.kind else { continue };
        let Some(init) = &declarator.init else { continue };
        let var_name = name_id.name.as_str();

        if let Expression::ObjectExpression(obj) = init {
            extract_object_methods(var_name, obj, builder, out);
            continue;
        }

        let (body, param_count) = match init {
            Expression::ArrowFunctionExpression(arrow) => {
                (arrow.body.statements.as_slice(), arrow.params.items.len())
            }
            Expression::FunctionExpression(fn_expr) => {
                let body = fn_expr.body.as_ref().map(|b| b.statements.as_slice()).unwrap_or(&[]);
                (body, fn_expr.params.items.len())
            }
            _ => continue,
        };
        out.push(builder.build(
            CompactString::from(var_name),
            FunctionKind::VariableArrow,
            body,
            param_count,
            declarator.span.start,
            declarator.span.end,
        ));
    }
}

fn extract_object_methods(
    obj_name: &str,
    obj: &ObjectExpression<'_>,
    builder: &Builder<'_>,
    out: &mut Vec<FunctionRef>,
) {
    for prop in &obj.properties {
        let ObjectPropertyKind::ObjectProperty(prop) = prop else { continue };
        let Some(method_name) = property_key_name(&prop.key) else { continue };
        let (body, param_count) = match &prop.value {
            Expression::ArrowFunctionExpression(arrow) => {
                (arrow.body.statements.as_slice(), arrow.params.items.len())
            }
            Expression::FunctionExpression(fn_expr) => {
                let body = fn_expr.body.as_ref().map(|b| b.statements.as_slice()).unwrap_or(&[]);
                (body, fn_expr.params.items.len())
            }
            _ => continue,
        };
        let mut symbol = CompactString::from(obj_name);
        symbol.push('.');
        symbol.push_str(&method_name);
        out.push(builder.build(
            symbol,
            FunctionKind::ObjectMethod,
            body,
            param_count,
            prop.span.start,
            prop.span.end,
        ));
    }
}

fn extract_class(class: &Class<'_>, builder: &Builder<'_>, out: &mut Vec<FunctionRef>) {
    let Some(class_name) = class.id.as_ref().map(|i| i.name.as_str()) else { return };
    for elem in &class.body.body {
        match elem {
            ClassElement::MethodDefinition(method) => {
                if method.kind == MethodDefinitionKind::Constructor {
                    continue;
                }
                let Some(method_name) = property_key_name(&method.key) else { continue };
                let body = method
                    .value
                    .body
                    .as_ref()
                    .map(|b| b.statements.as_slice())
                    .unwrap_or(&[]);
                let mut symbol = CompactString::from(class_name);
                symbol.push('.');
                symbol.push_str(&method_name);
                out.push(builder.build(
                    symbol,
                    FunctionKind::ClassMethod,
                    body,
                    method.value.params.items.len(),
                    method.span.start,
                    method.span.end,
                ));
            }
            ClassElement::PropertyDefinition(prop) => {
                let Some(prop_name) = property_key_name(&prop.key) else { continue };
                let Some(value) = &prop.value else { continue };
                let Expression::ArrowFunctionExpression(arrow) = value else { continue };
                let mut symbol = CompactString::from(class_name);
                symbol.push('.');
                symbol.push_str(&prop_name);
                out.push(builder.build(
                    symbol,
                    FunctionKind::ClassPropertyArrow,
                    arrow.body.statements.as_slice(),
                    arrow.params.items.len(),
                    prop.span.start,
                    prop.span.end,
                ));
            }
            _ => {}
        }
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<CompactString> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(CompactString::from(id.name.as_str())),
        PropertyKey::StringLiteral(s) => Some(CompactString::from(s.value.as_str())),
        // Computed/private keys are intentionally skipped.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_no_function_when_none_present() {
        let v = extract_all_functions("const x = 1;");
        assert!(v.is_empty());
    }

    #[test]
    fn extracts_single_function_declaration() {
        let v = extract_all_functions("function useUser() { return useSWR('/u'); }");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "useUser");
        assert_eq!(v[0].kind, FunctionKind::FunctionDeclaration);
        assert_eq!(v[0].calls, vec![compact_str::CompactString::from("useSWR")]);
    }

    #[test]
    fn captures_param_count() {
        let v = extract_all_functions("function f(a, b, c) {}");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].signature.param_count, 3);
    }

    #[test]
    fn captures_imports_at_file_level() {
        let src = "import { useSWR } from 'swr';\nfunction f() {}";
        let v = extract_all_functions(src);
        assert_eq!(v.len(), 1);
        assert!(v[0].imports.iter().any(|s| s == "swr"));
        assert!(v[0].imports.iter().any(|s| s == "useSWR"));
    }

    #[test]
    fn extracts_class_methods_with_dotted_symbol() {
        let src = "class Foo { async list() { return this.svc.findAll(); } }";
        let v = extract_all_functions(src);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "Foo.list");
        assert_eq!(v[0].kind, FunctionKind::ClassMethod);
    }

    #[test]
    fn extracts_class_methods_skips_constructor() {
        let src = "class Foo { constructor() {} bar() {} }";
        let v = extract_all_functions(src);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "Foo.bar");
    }

    #[test]
    fn extracts_exported_class_methods() {
        let src = "export class Foo { one() {} two() {} }";
        let v = extract_all_functions(src);
        assert_eq!(v.len(), 2);
        assert!(v.iter().any(|f| f.symbol.as_str() == "Foo.one"));
        assert!(v.iter().any(|f| f.symbol.as_str() == "Foo.two"));
    }

    #[test]
    fn extracts_default_exported_class_methods() {
        let src = "export default class Foo { one() {} }";
        let v = extract_all_functions(src);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "Foo.one");
    }

    // --- VariableArrow ---------------------------------------------------------

    #[test]
    fn extracts_variable_arrow() {
        let v = extract_all_functions("const useUser = () => useSWR('/u');");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "useUser");
        assert_eq!(v[0].kind, FunctionKind::VariableArrow);
        assert!(v[0].calls.iter().any(|s| s == "useSWR"));
    }

    #[test]
    fn extracts_variable_function_expression() {
        let v = extract_all_functions("const foo = function() { return 1; };");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "foo");
        assert_eq!(v[0].kind, FunctionKind::VariableArrow);
    }

    #[test]
    fn extracts_exported_variable_arrow() {
        let v = extract_all_functions("export const foo = () => 1;");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "foo");
        assert_eq!(v[0].kind, FunctionKind::VariableArrow);
    }

    #[test]
    fn skips_variable_with_non_function_init() {
        let v = extract_all_functions("const x = 1; export const y = 'a';");
        assert!(v.is_empty());
    }

    // --- ObjectMethod ----------------------------------------------------------

    #[test]
    fn extracts_object_method_shorthand() {
        let v = extract_all_functions("const obj = { foo() { return useSWR(); } };");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "obj.foo");
        assert_eq!(v[0].kind, FunctionKind::ObjectMethod);
    }

    #[test]
    fn extracts_object_arrow_property() {
        let v = extract_all_functions("const obj = { foo: () => 1, bar: () => 2 };");
        assert_eq!(v.len(), 2);
        assert!(v.iter().any(|f| f.symbol.as_str() == "obj.foo" && f.kind == FunctionKind::ObjectMethod));
        assert!(v.iter().any(|f| f.symbol.as_str() == "obj.bar" && f.kind == FunctionKind::ObjectMethod));
    }

    #[test]
    fn skips_non_function_object_property() {
        let v = extract_all_functions("const obj = { foo: 1, bar: 'x' };");
        assert!(v.is_empty());
    }

    // --- ClassPropertyArrow ----------------------------------------------------

    #[test]
    fn extracts_class_property_arrow() {
        let v = extract_all_functions("class C { foo = () => useSWR(); }");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "C.foo");
        assert_eq!(v[0].kind, FunctionKind::ClassPropertyArrow);
    }

    #[test]
    fn skips_class_property_non_function() {
        let v = extract_all_functions("class C { foo = 1; bar = 'x'; }");
        assert!(v.is_empty());
    }

    #[test]
    fn extracts_methods_and_property_arrows_together() {
        let v = extract_all_functions("class C { method() {} prop = () => 1; }");
        assert_eq!(v.len(), 2);
        assert!(v.iter().any(|f| f.symbol.as_str() == "C.method" && f.kind == FunctionKind::ClassMethod));
        assert!(v.iter().any(|f| f.symbol.as_str() == "C.prop" && f.kind == FunctionKind::ClassPropertyArrow));
    }

    // --- DefaultExportFunction -------------------------------------------------

    #[test]
    fn extracts_default_export_named_function() {
        let v = extract_all_functions("export default function bar() { return 1; }");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "default");
        assert_eq!(v[0].kind, FunctionKind::DefaultExportFunction);
    }

    #[test]
    fn extracts_default_export_anonymous_function() {
        let v = extract_all_functions("export default function() { return 1; }");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "default");
        assert_eq!(v[0].kind, FunctionKind::DefaultExportFunction);
    }

    #[test]
    fn extracts_default_export_arrow() {
        let v = extract_all_functions("export default () => useSWR();");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "default");
        assert_eq!(v[0].kind, FunctionKind::DefaultExportFunction);
    }

    #[test]
    fn skips_default_export_value() {
        let v = extract_all_functions("export default 42;");
        assert!(v.is_empty());
    }

    // --- meta ------------------------------------------------------------------

    #[test]
    fn populates_byte_range_and_ast_hash_for_all_kinds() {
        let src = r"
            function fnDecl() { return 1; }
            const arrow = () => useSWR();
            const obj = { foo() { return 1; } };
            class C { method() { return 1; } prop = () => 1; }
            export default function() { return 1; }
        ";
        let v = extract_all_functions(src);
        assert!(v.len() >= 5);
        for f in &v {
            assert!(f.byte_range.0 < f.byte_range.1, "byte_range invalid for {}", f.symbol);
            assert!(f.ast_hash != [0u8; 32], "ast_hash zero for {}", f.symbol);
            assert!(f.start_line > 0);
        }
    }

    // --- ignore directive integration ------------------------------------------

    #[test]
    fn function_without_directive_has_no_ignore() {
        let v = extract_all_functions("function f() { return 1; }");
        assert_eq!(v.len(), 1);
        assert!(v[0].ignore.is_none());
    }

    #[test]
    fn function_with_line_directive_is_ignored() {
        let src = "// layer-conform-ignore: testing\nfunction f() { return 1; }";
        let v = extract_all_functions(src);
        assert_eq!(v.len(), 1);
        let directive = v[0].ignore.as_ref().expect("directive present");
        assert_eq!(directive.reason.as_deref(), Some("testing"));
    }

    #[test]
    fn arrow_with_directive_is_ignored() {
        let src = "/* layer-conform-ignore: skip */\nconst foo = () => 1;";
        let v = extract_all_functions(src);
        assert_eq!(v.len(), 1);
        assert!(v[0].ignore.is_some());
    }

    #[test]
    fn directive_three_lines_above_class_method_is_ignored() {
        let src = "// layer-conform-ignore: legacy adapter\n\n\nclass C { method() {} }";
        let v = extract_all_functions(src);
        let m = v.iter().find(|f| f.symbol.as_str() == "C.method").expect("method");
        assert!(m.ignore.is_some());
    }
}
