//! Extract `FunctionDeclaration` from a TS/JS source. MVP: only top-level
//! function declarations are supported. Other kinds (Arrow / Method / ...)
//! are added in Plan #2.

use lc_core::{FunctionKind, FunctionRef, Signature};
use oxc_ast::ast::Statement;

use crate::{normalize, signature};

pub fn extract_function_declarations(source: &str) -> Vec<FunctionRef> {
    let alloc = oxc_allocator::Allocator::default();
    let ret = crate::oxc_compat::parse(&alloc, source);
    let program = &ret.program;
    let imports = signature::collect_imports(program);

    let mut out = Vec::new();
    for stmt in &program.body {
        if let Statement::FunctionDeclaration(decl) = stmt {
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
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_no_function_when_none_present() {
        let v = extract_function_declarations("const x = 1;");
        assert!(v.is_empty());
    }

    #[test]
    fn extracts_single_function_declaration() {
        let v = extract_function_declarations("function useUser() { return useSWR('/u'); }");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "useUser");
        assert_eq!(v[0].kind, FunctionKind::FunctionDeclaration);
        assert_eq!(v[0].calls, vec![compact_str::CompactString::from("useSWR")]);
    }

    #[test]
    fn skips_arrow_functions_in_mvp() {
        let v = extract_function_declarations("const useUser = () => useSWR('/u');");
        assert!(v.is_empty());
    }

    #[test]
    fn captures_param_count() {
        let v = extract_function_declarations("function f(a, b, c) {}");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].signature.param_count, 3);
    }

    #[test]
    fn captures_imports_at_file_level() {
        let src = "import { useSWR } from 'swr';\nfunction f() {}";
        let v = extract_function_declarations(src);
        assert_eq!(v.len(), 1);
        assert!(v[0].imports.iter().any(|s| s == "swr"));
        assert!(v[0].imports.iter().any(|s| s == "useSWR"));
    }
}
