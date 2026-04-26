//! TypeScript / JavaScript adapter for layer-conform.
//!
//! Wraps oxc_parser and converts oxc AST into the neutral `lc_core::TreeNode` IR.
//! Public entrypoint is `parse_file`, defined in Task 16.

mod extract;
mod normalize;
mod oxc_compat;
mod signature;
