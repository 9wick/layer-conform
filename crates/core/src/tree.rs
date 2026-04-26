//! Neutral AST tree IR shared across language adapters.

use compact_str::CompactString;

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

#[derive(Clone, Debug)]
pub struct TreeNode {
    pub kind: NodeKind,
    pub value: Option<CompactString>,
    pub children: Vec<Box<TreeNode>>,
    pub id: u32,
    pub subtree_size: u32,
}

impl TreeNode {
    /// 子なしリーフを作る。`id` と `subtree_size` は finalize で確定する。
    pub fn leaf(kind: NodeKind, value: Option<CompactString>) -> Self {
        Self { kind, value, children: Vec::new(), id: 0, subtree_size: 0 }
    }

    /// 子を持つノードを作る。`id` と `subtree_size` は finalize で確定する。
    pub fn branch(kind: NodeKind, children: Vec<TreeNode>) -> Self {
        Self {
            kind,
            value: None,
            children: children.into_iter().map(Box::new).collect(),
            id: 0,
            subtree_size: 0,
        }
    }

    /// preorder traversal で id を採番し、bottom-up で subtree_size を確定する。
    /// 構築完了後に 1 度だけ呼ぶ。
    pub fn finalize(&mut self) {
        let mut next_id: u32 = 0;
        Self::finalize_recurse(self, &mut next_id);
    }

    fn finalize_recurse(node: &mut TreeNode, next_id: &mut u32) {
        node.id = *next_id;
        *next_id += 1;
        let mut size: u32 = 1;
        for child in &mut node.children {
            Self::finalize_recurse(child, next_id);
            size += child.subtree_size;
        }
        node.subtree_size = size;
    }
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

    #[test]
    fn leaf_constructor_has_no_children() {
        let n = TreeNode::leaf(NodeKind::Identifier, Some("x".into()));
        assert_eq!(n.kind, NodeKind::Identifier);
        assert_eq!(n.value.as_deref(), Some("x"));
        assert_eq!(n.children.len(), 0);
    }

    #[test]
    fn branch_constructor_owns_children() {
        let leaf = TreeNode::leaf(NodeKind::Identifier, None);
        let branch = TreeNode::branch(NodeKind::Block, vec![leaf]);
        assert_eq!(branch.children.len(), 1);
        assert_eq!(branch.children[0].kind, NodeKind::Identifier);
    }

    #[test]
    fn finalize_assigns_preorder_ids() {
        // tree:
        //     Block (id=0, size=3)
        //     ├── Identifier (id=1, size=1)
        //     └── Identifier (id=2, size=1)
        let leaf1 = TreeNode::leaf(NodeKind::Identifier, Some("a".into()));
        let leaf2 = TreeNode::leaf(NodeKind::Identifier, Some("b".into()));
        let mut root = TreeNode::branch(NodeKind::Block, vec![leaf1, leaf2]);
        root.finalize();
        assert_eq!(root.id, 0);
        assert_eq!(root.subtree_size, 3);
        assert_eq!(root.children[0].id, 1);
        assert_eq!(root.children[0].subtree_size, 1);
        assert_eq!(root.children[1].id, 2);
        assert_eq!(root.children[1].subtree_size, 1);
    }

    #[test]
    fn finalize_handles_nested_subtrees() {
        // tree:
        //     Block (id=0, size=4)
        //     └── Block (id=1, size=3)
        //         ├── Identifier (id=2, size=1)
        //         └── Identifier (id=3, size=1)
        let leaf1 = TreeNode::leaf(NodeKind::Identifier, None);
        let leaf2 = TreeNode::leaf(NodeKind::Identifier, None);
        let inner = TreeNode::branch(NodeKind::Block, vec![leaf1, leaf2]);
        let mut root = TreeNode::branch(NodeKind::Block, vec![inner]);
        root.finalize();
        assert_eq!(root.subtree_size, 4);
        assert_eq!(root.children[0].subtree_size, 3);
    }
}
