//! TypeScript / JavaScript adapter for layer-conform.
//!
//! Wraps `oxc_parser` and converts oxc AST into the neutral `layer_conform_core::TreeNode` IR.

mod extract;
mod normalize;
mod oxc_compat;
mod signature;

use layer_conform_core::FunctionRef;

/// Parse a TS/JS source string and return all extractable functions.
/// Returns top-level `FunctionDeclarations` and class methods.
pub fn parse_file(source: &str) -> Vec<FunctionRef> {
    extract::extract_all_functions(source)
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
