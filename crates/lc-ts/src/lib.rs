//! TypeScript / JavaScript adapter for layer-conform.
//!
//! Wraps oxc_parser and converts oxc AST into the neutral `lc_core::TreeNode` IR.

mod extract;
mod normalize;
mod oxc_compat;
mod signature;

use lc_core::FunctionRef;

/// Parse a TS/JS source string and return all extractable functions.
/// MVP supports only `FunctionDeclaration`.
pub fn parse_file(source: &str) -> Vec<FunctionRef> {
    extract::extract_function_declarations(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_file_returns_extractable_functions() {
        let v = parse_file("function foo() {}\nfunction bar() {}");
        assert_eq!(v.len(), 2);
    }
}
