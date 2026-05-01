//! Rust adapter for layer-conform.
//!
//! Wraps `syn` and converts Rust AST into the neutral `lc_core::TreeNode` IR.
//! When called with a [`workspace::Workspace`] context, every call/method/macro
//! site is encoded by its [`CallOrigin`] (relative-layer signature, inter-
//! package, external, stdlib, …) instead of by its raw name — see
//! `crates/core/src/call_origin.rs` for the language-neutral model.

mod extract;
mod normalize;
mod resolver;
mod signature;
pub mod workspace;

pub use extract::FileContext;
use lc_core::FunctionRef;

/// Parse a Rust source string and return all extractable functions.
/// Without a workspace context, callees keep their raw names (preserving
/// backwards compatibility with existing lc-rs unit tests).
pub fn parse_file(source: &str) -> Vec<FunctionRef> {
    extract::extract_all_functions(source, None)
}

/// Workspace-aware variant: every callee gets a relative-layer / package
/// label via [`lc_core::call_origin::CallOrigin`].
pub fn parse_file_with_context(source: &str, ctx: &FileContext<'_>) -> Vec<FunctionRef> {
    extract::extract_all_functions(source, Some(ctx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_file_returns_extractable_functions() {
        let v = parse_file("fn foo() {}\nfn bar() {}");
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn parse_file_returns_empty_on_invalid_source() {
        let v = parse_file("this is not rust");
        assert!(v.is_empty());
    }
}
