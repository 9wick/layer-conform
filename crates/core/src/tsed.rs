//! TSED — Type Structure Edit Distance score.
//!
//! Normalizes APTED distance to a 0..1 similarity:
//!   score = max(0.0, 1.0 - distance / max(size_a, size_b))

use crate::apted::{edit_distance, AptedOptions};
use crate::tree::TreeNode;

pub fn tsed(a: &TreeNode, b: &TreeNode) -> f64 {
    tsed_with(a, b, AptedOptions::default())
}

pub fn tsed_with(a: &TreeNode, b: &TreeNode, opts: AptedOptions) -> f64 {
    let max_size = u32::max(a.subtree_size, b.subtree_size);
    if max_size == 0 {
        return 1.0;
    }
    let d = edit_distance(a, b, opts);
    let score = 1.0 - d / f64::from(max_size);
    score.max(0.0).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{NodeKind, TreeNode};

    fn finalized(mut t: TreeNode) -> TreeNode {
        t.finalize();
        t
    }

    #[test]
    fn identical_trees_score_one() {
        let a = finalized(TreeNode::branch(
            NodeKind::Block,
            vec![TreeNode::leaf(NodeKind::Identifier, Some("x".into()))],
        ));
        let b = finalized(TreeNode::branch(
            NodeKind::Block,
            vec![TreeNode::leaf(NodeKind::Identifier, Some("x".into()))],
        ));
        assert!((tsed(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn fully_different_trees_score_low() {
        let a = finalized(TreeNode::leaf(NodeKind::Identifier, Some("x".into())));
        let b = finalized(TreeNode::leaf(NodeKind::Literal, Some("y".into())));
        let s = tsed(&a, &b);
        assert!(s <= 0.01, "expected near 0, got {s}");
    }

    #[test]
    fn partial_overlap_is_in_between() {
        let a = finalized(TreeNode::branch(
            NodeKind::Block,
            vec![
                TreeNode::leaf(NodeKind::Identifier, Some("x".into())),
                TreeNode::leaf(NodeKind::Identifier, Some("y".into())),
            ],
        ));
        let b = finalized(TreeNode::branch(
            NodeKind::Block,
            vec![TreeNode::leaf(NodeKind::Identifier, Some("x".into()))],
        ));
        let s = tsed(&a, &b);
        assert!(s > 0.6 && s < 0.7, "expected ~0.667, got {s}");
    }
}
