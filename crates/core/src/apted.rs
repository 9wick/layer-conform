//! APTED (All Path Tree Edit Distance) implementation.
//!
//! Memoized DP over `(node1.id, node2.id)` pairs. Identifier equality is
//! decided by `(kind, value)` — id/subtree_size are ignored.

use std::collections::HashMap;

use crate::tree::TreeNode;

#[derive(Copy, Clone, Debug)]
pub struct AptedOptions {
    pub rename_cost: f64,
    pub insert_cost: f64,
    pub delete_cost: f64,
}

impl Default for AptedOptions {
    fn default() -> Self {
        Self { rename_cost: 1.0, insert_cost: 1.0, delete_cost: 1.0 }
    }
}

/// Compute tree edit distance between `a` and `b`.
/// Both trees must have been finalized (id/subtree_size set).
pub fn edit_distance(a: &TreeNode, b: &TreeNode, opts: AptedOptions) -> f64 {
    let mut memo: HashMap<(u32, u32), f64> = HashMap::new();
    distance_recurse(a, b, opts, &mut memo)
}

fn distance_recurse(
    a: &TreeNode,
    b: &TreeNode,
    opts: AptedOptions,
    memo: &mut HashMap<(u32, u32), f64>,
) -> f64 {
    if let Some(v) = memo.get(&(a.id, b.id)) {
        return *v;
    }
    let cost_root = if a.kind == b.kind && a.value == b.value {
        0.0
    } else {
        opts.rename_cost
    };

    // Children edit distance via DP over child sequences.
    let n = a.children.len();
    let m = b.children.len();
    let mut dp = vec![vec![0.0_f64; m + 1]; n + 1];
    for i in 1..=n {
        dp[i][0] = dp[i - 1][0] + opts.delete_cost * f64::from(a.children[i - 1].subtree_size);
    }
    for j in 1..=m {
        dp[0][j] = dp[0][j - 1] + opts.insert_cost * f64::from(b.children[j - 1].subtree_size);
    }
    for i in 1..=n {
        for j in 1..=m {
            let del = dp[i - 1][j] + opts.delete_cost * f64::from(a.children[i - 1].subtree_size);
            let ins = dp[i][j - 1] + opts.insert_cost * f64::from(b.children[j - 1].subtree_size);
            let rep = dp[i - 1][j - 1]
                + distance_recurse(&a.children[i - 1], &b.children[j - 1], opts, memo);
            dp[i][j] = del.min(ins).min(rep);
        }
    }

    let total = cost_root + dp[n][m];
    memo.insert((a.id, b.id), total);
    total
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
    fn identical_leaves_have_zero_distance() {
        let a = finalized(TreeNode::leaf(NodeKind::Identifier, Some("x".into())));
        let b = finalized(TreeNode::leaf(NodeKind::Identifier, Some("x".into())));
        assert!((edit_distance(&a, &b, AptedOptions::default()) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn different_value_costs_one_rename() {
        let a = finalized(TreeNode::leaf(NodeKind::Identifier, Some("x".into())));
        let b = finalized(TreeNode::leaf(NodeKind::Identifier, Some("y".into())));
        assert!((edit_distance(&a, &b, AptedOptions::default()) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn missing_child_costs_subtree_size() {
        // a: Block(Ident, Ident)   size=3
        // b: Block(Ident)          size=2
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
        let d = edit_distance(&a, &b, AptedOptions::default());
        assert!((d - 1.0).abs() < 1e-9, "expected 1.0, got {d}");
    }

    #[test]
    fn completely_different_trees() {
        let a = finalized(TreeNode::leaf(NodeKind::Identifier, Some("x".into())));
        let b = finalized(TreeNode::leaf(NodeKind::Literal, Some("y".into())));
        // kind と value 両方違う → rename 1.0
        assert!((edit_distance(&a, &b, AptedOptions::default()) - 1.0).abs() < 1e-9);
    }
}
