//! Pure logic core for layer-conform.

pub mod apted;
pub mod deviation;
pub mod function_ref;
pub mod ignore_parse;
pub mod matcher;
pub mod pipeline;
pub mod rule;
pub mod similarity;
pub mod tree;
pub mod tsed;

pub use function_ref::{FunctionKind, FunctionRef, Signature};
pub use rule::{GoldenSelector, Rule};
