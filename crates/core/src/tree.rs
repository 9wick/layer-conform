//! Neutral AST tree IR shared across language adapters.

#[repr(u32)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum NodeKind {
    Program,
    FunctionDeclaration,
    ArrowFunction,
    Method,
    CallExpression,
    MemberExpression,
    JsxElement,
    Identifier,
    Literal,
    ImportSpecifier,
    Block,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_kind_discriminants_are_stable() {
        // discriminant は baseline hash の入力に使われるため安定が必要。
        // 値が変わったら NodeKind::* の順序を変えていないか要確認。
        assert_eq!(NodeKind::Program as u32, 0);
        assert_eq!(NodeKind::FunctionDeclaration as u32, 1);
        assert_eq!(NodeKind::ArrowFunction as u32, 2);
    }

    #[test]
    fn node_kind_is_copy_and_eq() {
        let a = NodeKind::Identifier;
        let b = a;
        assert_eq!(a, b);
    }
}
