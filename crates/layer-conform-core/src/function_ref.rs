//! Per-function metadata returned by language analyzers.

use compact_str::CompactString;

use crate::ignore_parse::IgnoreDirective;
use crate::tree::TreeNode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionKind {
    FunctionDeclaration,
    VariableArrow,
    ObjectMethod,
    ClassMethod,
    ClassPropertyArrow,
    DefaultExportFunction,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Signature {
    pub param_count: u32,
}

#[derive(Clone, Debug)]
pub struct FunctionRef {
    pub symbol: CompactString,
    pub kind: FunctionKind,
    pub start_line: u32,
    pub end_line: u32,
    pub byte_range: (u32, u32),
    pub tree: TreeNode,
    pub signature: Signature,
    pub calls: Vec<CompactString>,
    pub imports: Vec<CompactString>,
    pub ast_hash: [u8; 32],
    /// `Some` when a `layer-conform-ignore` directive precedes the function.
    /// Pipeline must skip ignored functions when detecting deviations.
    pub ignore: Option<IgnoreDirective>,
}
