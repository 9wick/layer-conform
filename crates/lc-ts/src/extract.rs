//! Extract `FunctionDeclaration` and class methods from a TS/JS source.

use compact_str::CompactString;
use lc_core::{FunctionKind, FunctionRef, Signature};
use oxc_ast::ast::{
    Class, ClassElement, ExportDefaultDeclarationKind, MethodDefinitionKind, PropertyKey,
    Statement,
};

use crate::{normalize, signature};

/// Extract top-level FunctionDeclarations and class methods.
pub fn extract_all_functions(source: &str) -> Vec<FunctionRef> {
    let alloc = oxc_allocator::Allocator::default();
    let ret = crate::oxc_compat::parse(&alloc, source);
    let program = &ret.program;
    let imports = signature::collect_imports(program);

    let mut out = Vec::new();
    for stmt in &program.body {
        match stmt {
            Statement::FunctionDeclaration(decl) => {
                let Some(name) = decl.id.as_ref().map(|i| i.name.as_str()) else { continue };
                let body = decl.body.as_ref().map(|b| b.statements.as_slice()).unwrap_or(&[]);
                let mut tree = normalize::normalize_block(body);
                tree.finalize();
                let ast_hash = tree.canonical_hash();
                let calls = signature::collect_calls(body);
                let span = decl.span;
                out.push(FunctionRef {
                    symbol: name.into(),
                    kind: FunctionKind::FunctionDeclaration,
                    start_line: 0,
                    end_line: 0,
                    byte_range: (span.start, span.end),
                    tree,
                    signature: Signature { param_count: decl.params.items.len() as u32 },
                    calls,
                    imports: imports.clone(),
                    ast_hash,
                });
            }
            Statement::ClassDeclaration(class) => {
                extract_class_methods(class, &imports, &mut out);
            }
            Statement::ExportNamedDeclaration(exp) => {
                if let Some(decl) = &exp.declaration {
                    use oxc_ast::ast::Declaration;
                    match decl {
                        Declaration::FunctionDeclaration(f) => {
                            let Some(name) = f.id.as_ref().map(|i| i.name.as_str()) else {
                                continue;
                            };
                            let body =
                                f.body.as_ref().map(|b| b.statements.as_slice()).unwrap_or(&[]);
                            let mut tree = normalize::normalize_block(body);
                            tree.finalize();
                            let ast_hash = tree.canonical_hash();
                            let calls = signature::collect_calls(body);
                            let span = f.span;
                            out.push(FunctionRef {
                                symbol: name.into(),
                                kind: FunctionKind::FunctionDeclaration,
                                start_line: 0,
                                end_line: 0,
                                byte_range: (span.start, span.end),
                                tree,
                                signature: Signature {
                                    param_count: f.params.items.len() as u32,
                                },
                                calls,
                                imports: imports.clone(),
                                ast_hash,
                            });
                        }
                        Declaration::ClassDeclaration(class) => {
                            extract_class_methods(class, &imports, &mut out);
                        }
                        _ => {}
                    }
                }
            }
            Statement::ExportDefaultDeclaration(exp) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &exp.declaration {
                    extract_class_methods(class, &imports, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}

fn extract_class_methods(
    class: &Class<'_>,
    imports: &[CompactString],
    out: &mut Vec<FunctionRef>,
) {
    let Some(class_name) = class.id.as_ref().map(|i| i.name.as_str()) else { return };
    for elem in &class.body.body {
        let ClassElement::MethodDefinition(method) = elem else { continue };
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
        let mut tree = normalize::normalize_block(body);
        tree.finalize();
        let ast_hash = tree.canonical_hash();
        let calls = signature::collect_calls(body);
        let span = method.span;
        let mut symbol = CompactString::from(class_name);
        symbol.push('.');
        symbol.push_str(&method_name);
        out.push(FunctionRef {
            symbol,
            kind: FunctionKind::ClassMethod,
            start_line: 0,
            end_line: 0,
            byte_range: (span.start, span.end),
            tree,
            signature: Signature { param_count: method.value.params.items.len() as u32 },
            calls,
            imports: imports.to_vec(),
            ast_hash,
        });
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
    fn skips_arrow_functions_in_mvp() {
        let v = extract_all_functions("const useUser = () => useSWR('/u');");
        assert!(v.is_empty());
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
}
