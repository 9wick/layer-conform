//! Pure logic core for layer-conform.
//!
//! This crate contains zero I/O. It exposes the AST IR (`tree`),
//! similarity algorithms (`apted`, `tsed`), and deviation data model.

pub mod apted;
pub mod deviation;
pub mod similarity;
pub mod tree;
pub mod tsed;
