# layer-conform MVP Implementation Plan (Phase 1a + 1b)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rust 製の `layer-conform` CLI バイナリの最小動作版を実装する。`layer-conform check <FILE> --golden <FILE:SYMBOL>` で 1 関数 vs 1 golden の AST 類似度を 4 軸 (shape/calls/imports/signature) で表示できるところまで。

**Architecture:** Cargo workspace で `lc-core` (純粋ロジック) / `lc-ts` (oxc 連携) / `cli` の 3 crate に分割 (lc-io は Plan #2 で追加)。`lc-core` の TreeNode に対し APTED + TSED で構文形状類似度、calls/imports の Jaccard、signature 一致度を別軸で算出し、加重平均で `overall` スコアを出す。MVP では `FunctionDeclaration` のみ抽出する。

**Tech Stack:** Rust 2021 / Cargo workspace / oxc_parser `=0.73.0` / clap v4 derive / blake3 / compact_str / smallvec / anyhow + thiserror / assert_cmd + predicates + tempfile (dev)

**Spec reference:** `docs/superpowers/specs/2026-04-26-layer-conform-design.md`

**Note on git commits:** ユーザーの CLAUDE.md により「明示指示があるまで commit 禁止」。各タスクの最後の commit step は **ユーザーが明示承認したときのみ実行** すること。実行モードを始める前に、ユーザーに「タスク区切りで commit してよいか」を確認する。

---

## File Structure

```
layer-conform/
├── Cargo.toml                       # workspace root
├── rust-toolchain.toml              # Rust 1.80+
├── rustfmt.toml
├── .gitignore
├── crates/
│   ├── core/                        # lc-core (純粋ロジック)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs               # re-exports
│   │       ├── tree.rs              # NodeKind, TreeNode, builder, hash
│   │       ├── apted.rs             # APTED edit distance
│   │       ├── tsed.rs              # TSED normalized score
│   │       ├── similarity.rs        # SimilarityScore, Jaccard
│   │       └── deviation.rs         # Differences (差分計算)
│   ├── lc-ts/                       # TS/JS adapter
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs               # parse_file エントリ
│   │       ├── oxc_compat.rs        # oxc API 集約 (Anti-Corruption Layer)
│   │       ├── extract.rs           # FunctionDeclaration 抽出
│   │       ├── normalize.rs         # oxc AST → TreeNode
│   │       └── signature.rs         # calls/imports/signature 抽出
│   └── cli/                         # CLI バイナリ
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs
│       │   ├── args.rs              # clap derive 定義
│       │   ├── reporter.rs          # text 出力
│       │   └── runner.rs            # check コマンド実行
│       └── tests/
│           └── integration.rs       # assert_cmd でフルフロー検証
```

**Note:** `lc-io` crate は Plan #2 で追加。MVP では `cli` から `lc-ts::parse_file` を直接呼ぶ。

---

## Task 1: Workspace セットアップ

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `rustfmt.toml`
- Create: `.gitignore`

- [ ] **Step 1.1: `.gitignore` を作成**

Create `/workspaces/github.com/9wick/layer-conform/.gitignore`:

```
target/
**/*.rs.bk
Cargo.lock.bak
```

`Cargo.lock` はバイナリ crate なのでコミットする (gitignore しない)。

- [ ] **Step 1.2: `rust-toolchain.toml` を作成**

Create `/workspaces/github.com/9wick/layer-conform/rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.82.0"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

- [ ] **Step 1.3: `rustfmt.toml` を作成**

Create `/workspaces/github.com/9wick/layer-conform/rustfmt.toml`:

```toml
edition = "2021"
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

- [ ] **Step 1.4: ルート `Cargo.toml` を作成**

Create `/workspaces/github.com/9wick/layer-conform/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/core",
    "crates/lc-ts",
    "crates/cli",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
rust-version = "1.82"

[workspace.dependencies]
# Internal crates
lc-core = { path = "crates/core" }
lc-ts = { path = "crates/lc-ts" }

# External (exact pin で固定)
oxc_parser = "=0.73.0"
oxc_ast = "=0.73.0"
oxc_allocator = "=0.73.0"
oxc_span = "=0.73.0"

clap = { version = "=4.5.21", features = ["derive"] }
serde = { version = "=1.0.215", features = ["derive"] }
serde_json = "=1.0.133"
anyhow = "=1.0.94"
thiserror = "=2.0.6"
compact_str = { version = "=0.8.1", features = ["serde"] }
smallvec = { version = "=1.13.2", features = ["serde", "union"] }
blake3 = "=1.5.5"

# Dev
assert_cmd = "=2.0.16"
predicates = "=3.1.3"
tempfile = "=3.14.0"

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
must_use_candidate = "allow"
```

- [ ] **Step 1.5: `cargo check` で workspace が認識されるか確認**

Run: `cargo check --workspace`
Expected: members が空なのでエラーになる (Task 2 以降で解消) — 正確には member の存在チェックで失敗する。member ディレクトリを Step 2 以降で作るので、ここでは「Cargo.toml が parse できる」だけ確認すれば良い。

Run: `cargo metadata --no-deps --format-version 1 > /dev/null`
Expected: Cargo.toml の syntax error は無いが、members が見つからずエラー。これは正常 (Task 2 で解消)。

- [ ] **Step 1.6: Commit**

```bash
git add Cargo.toml rust-toolchain.toml rustfmt.toml .gitignore
git commit -m "chore: initialize Cargo workspace with pinned dependencies"
```

---

## Task 2: `lc-core` crate scaffold

**Files:**
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs`

- [ ] **Step 2.1: `crates/core/Cargo.toml` を作成**

```toml
[package]
name = "lc-core"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
compact_str.workspace = true
smallvec.workspace = true
blake3.workspace = true
thiserror.workspace = true

[lints]
workspace = true
```

- [ ] **Step 2.2: `crates/core/src/lib.rs` を作成**

```rust
//! Pure logic core for layer-conform.
//!
//! This crate contains zero I/O. It exposes the AST IR (`tree`),
//! similarity algorithms (`apted`, `tsed`), and deviation data model.

pub mod apted;
pub mod deviation;
pub mod similarity;
pub mod tree;
pub mod tsed;
```

- [ ] **Step 2.3: 各 module ファイルを placeholder で作成**

Create `crates/core/src/tree.rs`:
```rust
// Implemented in Task 3-6.
```

Create `crates/core/src/apted.rs`:
```rust
// Implemented in Task 7.
```

Create `crates/core/src/tsed.rs`:
```rust
// Implemented in Task 8.
```

Create `crates/core/src/similarity.rs`:
```rust
// Implemented in Task 9.
```

Create `crates/core/src/deviation.rs`:
```rust
// Implemented in Task 10.
```

- [ ] **Step 2.4: `cargo build -p lc-core` で warnings なくビルドが通るか**

Run: `cargo build -p lc-core`
Expected: PASS (空の crate)

- [ ] **Step 2.5: Commit**

```bash
git add crates/core
git commit -m "feat(core): scaffold lc-core crate with module skeleton"
```

---

## Task 3: `lc-core::tree` — `NodeKind` enum

**Files:**
- Modify: `crates/core/src/tree.rs`
- Test: 同ファイル内 `#[cfg(test)] mod tests`

- [ ] **Step 3.1: 失敗するテストを書く**

`crates/core/src/tree.rs` を上書き:

```rust
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
```

- [ ] **Step 3.2: 失敗確認**

Run: `cargo test -p lc-core --lib tree::tests`
Expected: PASS (この最小定義で通る)

- [ ] **Step 3.3: Commit**

```bash
git add crates/core/src/tree.rs
git commit -m "feat(core/tree): introduce NodeKind enum with stable discriminants"
```

---

## Task 4: `lc-core::tree` — `TreeNode` 構造体と builder

**Files:**
- Modify: `crates/core/src/tree.rs`

- [ ] **Step 4.1: 失敗するテストを書く**

`crates/core/src/tree.rs` の末尾 (test mod の前) に追加:

```rust
use compact_str::CompactString;

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
}
```

`tests` mod に追加:

```rust
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
```

- [ ] **Step 4.2: テスト実行**

Run: `cargo test -p lc-core --lib tree::tests`
Expected: PASS

- [ ] **Step 4.3: Commit**

```bash
git add crates/core/src/tree.rs
git commit -m "feat(core/tree): add TreeNode with leaf/branch constructors"
```

---

## Task 5: `lc-core::tree` — `finalize` で id 採番と subtree_size 確定

**Files:**
- Modify: `crates/core/src/tree.rs`

`finalize` は構築済み TreeNode に対し、preorder traversal で連番 id を振り、bottom-up で `subtree_size` を確定させる。

- [ ] **Step 5.1: 失敗するテストを書く**

`tests` mod に追加:

```rust
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
```

- [ ] **Step 5.2: 失敗確認**

Run: `cargo test -p lc-core --lib tree::tests::finalize_assigns_preorder_ids`
Expected: FAIL — `finalize` がまだ無い

- [ ] **Step 5.3: 実装する**

`impl TreeNode` ブロック (Task 4 の `branch` の下) に追加:

```rust
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
```

- [ ] **Step 5.4: テスト通過確認**

Run: `cargo test -p lc-core --lib tree::tests`
Expected: PASS (4 tests)

- [ ] **Step 5.5: Commit**

```bash
git add crates/core/src/tree.rs
git commit -m "feat(core/tree): add TreeNode::finalize for id/subtree_size"
```

---

## Task 6: `lc-core::tree` — canonical hash (blake3)

**Files:**
- Modify: `crates/core/src/tree.rs`

baseline 用の AST hash を blake3 で算出する。serde 経由ではなく **手書き canonical writer** で `kind | value 長 | value bytes | children 数 | 子` の単純フォーマット。

- [ ] **Step 6.1: 失敗するテストを書く**

`tests` mod に追加:

```rust
    #[test]
    fn canonical_hash_is_deterministic() {
        let mut tree = TreeNode::branch(
            NodeKind::Block,
            vec![TreeNode::leaf(NodeKind::Identifier, Some("x".into()))],
        );
        tree.finalize();
        let h1 = tree.canonical_hash();
        let h2 = tree.canonical_hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn canonical_hash_differs_for_different_kinds() {
        let mut t1 = TreeNode::leaf(NodeKind::Identifier, Some("x".into()));
        t1.finalize();
        let mut t2 = TreeNode::leaf(NodeKind::Literal, Some("x".into()));
        t2.finalize();
        assert_ne!(t1.canonical_hash(), t2.canonical_hash());
    }

    #[test]
    fn canonical_hash_differs_for_different_values() {
        let mut t1 = TreeNode::leaf(NodeKind::Identifier, Some("x".into()));
        t1.finalize();
        let mut t2 = TreeNode::leaf(NodeKind::Identifier, Some("y".into()));
        t2.finalize();
        assert_ne!(t1.canonical_hash(), t2.canonical_hash());
    }

    #[test]
    fn canonical_hash_ignores_id_and_size() {
        // id と subtree_size は構築順で変わるが、ハッシュには影響しないことを確認。
        let mut t1 = TreeNode::leaf(NodeKind::Identifier, Some("x".into()));
        t1.id = 100;
        t1.subtree_size = 999;
        let h1 = t1.canonical_hash();
        t1.id = 0;
        t1.subtree_size = 1;
        let h2 = t1.canonical_hash();
        assert_eq!(h1, h2);
    }
```

- [ ] **Step 6.2: 失敗確認**

Run: `cargo test -p lc-core --lib tree::tests::canonical_hash_is_deterministic`
Expected: FAIL — `canonical_hash` がまだ無い

- [ ] **Step 6.3: 実装する**

`impl TreeNode` ブロックに追加:

```rust
    /// blake3(canonical bytes) を返す。
    /// フォーマット: kind(u32 LE) | value 長(u32 LE) | value bytes | children 数(u32 LE) | 子を再帰
    /// id / subtree_size は入力に含めない (構築順で変動するため)。
    pub fn canonical_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        Self::write_canonical(self, &mut hasher);
        *hasher.finalize().as_bytes()
    }

    fn write_canonical(node: &TreeNode, hasher: &mut blake3::Hasher) {
        hasher.update(&(node.kind as u32).to_le_bytes());
        let v = node.value.as_deref().unwrap_or("");
        let v_bytes = v.as_bytes();
        hasher.update(&(v_bytes.len() as u32).to_le_bytes());
        hasher.update(v_bytes);
        hasher.update(&(node.children.len() as u32).to_le_bytes());
        for child in &node.children {
            Self::write_canonical(child, hasher);
        }
    }
```

- [ ] **Step 6.4: テスト通過確認**

Run: `cargo test -p lc-core --lib tree::tests`
Expected: PASS (8 tests)

- [ ] **Step 6.5: Commit**

```bash
git add crates/core/src/tree.rs
git commit -m "feat(core/tree): add canonical_hash via blake3 with stable format"
```

---

## Task 7: `lc-core::apted` — APTED edit distance

**Files:**
- Modify: `crates/core/src/apted.rs`

メモ化付きの基本 APTED。ノード比較は `(kind, value)` 一致で判定し、不一致時は `rename_cost` (デフォルト `1.0`)。挿入・削除コストはどちらも `1.0`。

- [ ] **Step 7.1: 失敗するテストを書く**

`crates/core/src/apted.rs` を上書き:

```rust
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
```

- [ ] **Step 7.2: テスト実行**

Run: `cargo test -p lc-core --lib apted::tests`
Expected: PASS (4 tests)

- [ ] **Step 7.3: Commit**

```bash
git add crates/core/src/apted.rs
git commit -m "feat(core/apted): implement memoized APTED edit distance"
```

---

## Task 8: `lc-core::tsed` — 正規化スコア

**Files:**
- Modify: `crates/core/src/tsed.rs`

TSED は APTED 距離を `max(size_a, size_b)` で割って `1.0 - 正規化距離` で 0..1 のスコアに変換する。同一は 1.0、完全別物は 0.0 に近い値。

- [ ] **Step 8.1: 失敗するテストを書く**

`crates/core/src/tsed.rs` を上書き:

```rust
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
        // size 1 vs 1, distance 1 → score 0.0
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
        // distance 1, max size 3 → score ≈ 0.667
        assert!(s > 0.6 && s < 0.7, "expected ~0.667, got {s}");
    }
}
```

- [ ] **Step 8.2: テスト実行**

Run: `cargo test -p lc-core --lib tsed::tests`
Expected: PASS (3 tests)

- [ ] **Step 8.3: Commit**

```bash
git add crates/core/src/tsed.rs
git commit -m "feat(core/tsed): add size-normalized TSED score"
```

---

## Task 9: `lc-core::similarity` — Jaccard と SimilarityScore

**Files:**
- Modify: `crates/core/src/similarity.rs`

`calls` `imports` の集合 (ソート済み `Vec<CompactString>`) の Jaccard 類似度、および 4 軸の `SimilarityScore` 構造体と `overall` 算出を実装する。

- [ ] **Step 9.1: 失敗するテストを書く**

`crates/core/src/similarity.rs` を上書き:

```rust
//! 4-axis similarity score.
//!
//! Decomposes similarity into shape (TSED), calls (Jaccard), imports (Jaccard),
//! and signature (binary) so that `--explain` / `why` can show *what* differs.

use compact_str::CompactString;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimilarityScore {
    pub overall: f64,
    pub shape: f64,
    pub calls: f64,
    pub imports: f64,
    pub signature: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Weights {
    pub shape: f64,
    pub calls: f64,
    pub imports: f64,
    pub signature: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self { shape: 0.6, calls: 0.3, imports: 0.1, signature: 0.0 }
    }
}

/// Jaccard similarity over two sorted slices. Both inputs MUST be sorted.
pub fn jaccard_sorted(a: &[CompactString], b: &[CompactString]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let (mut i, mut j, mut intersect, mut union_n) = (0_usize, 0_usize, 0_usize, 0_usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Equal => {
                intersect += 1;
                union_n += 1;
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => {
                union_n += 1;
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                union_n += 1;
                j += 1;
            }
        }
    }
    union_n += a.len() - i;
    union_n += b.len() - j;
    if union_n == 0 {
        return 1.0;
    }
    intersect as f64 / union_n as f64
}

/// Build a SimilarityScore from per-axis values and weights.
pub fn aggregate(
    shape: f64,
    calls: f64,
    imports: f64,
    signature: f64,
    w: Weights,
) -> SimilarityScore {
    let total_w = w.shape + w.calls + w.imports + w.signature;
    let overall = if total_w > 0.0 {
        (shape * w.shape + calls * w.calls + imports * w.imports + signature * w.signature)
            / total_w
    } else {
        0.0
    };
    SimilarityScore { overall, shape, calls, imports, signature }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cs(items: &[&str]) -> Vec<CompactString> {
        let mut v: Vec<CompactString> = items.iter().map(|s| (*s).into()).collect();
        v.sort();
        v
    }

    #[test]
    fn jaccard_empty_inputs() {
        assert!((jaccard_sorted(&[], &[]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_identical_sets() {
        let a = cs(&["useSWR", "axios"]);
        let b = cs(&["useSWR", "axios"]);
        assert!((jaccard_sorted(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_disjoint_sets() {
        let a = cs(&["useSWR"]);
        let b = cs(&["axios"]);
        assert!(jaccard_sorted(&a, &b).abs() < 1e-9);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let a = cs(&["useSWR", "axios"]);
        let b = cs(&["useSWR", "fetch"]);
        // intersect 1, union 3 → 0.333...
        let j = jaccard_sorted(&a, &b);
        assert!((j - 1.0 / 3.0).abs() < 1e-9, "got {j}");
    }

    #[test]
    fn aggregate_uses_weights() {
        let s = aggregate(1.0, 0.0, 0.0, 0.0, Weights::default());
        // shape=0.6, calls=0.3, imports=0.1, sig=0 → total_w=1.0
        // overall = 1.0 * 0.6 / 1.0 = 0.6
        assert!((s.overall - 0.6).abs() < 1e-9);
        assert!((s.shape - 1.0).abs() < 1e-9);
    }
}
```

- [ ] **Step 9.2: テスト実行**

Run: `cargo test -p lc-core --lib similarity::tests`
Expected: PASS (5 tests)

- [ ] **Step 9.3: Commit**

```bash
git add crates/core/src/similarity.rs
git commit -m "feat(core/similarity): add Jaccard helper and 4-axis aggregate"
```

---

## Task 10: `lc-core::deviation` — Differences 計算

**Files:**
- Modify: `crates/core/src/deviation.rs`

`Differences` 構造体と、ソート済みの `calls` / `imports` から `missing` / `extra` を求めるヘルパ。

- [ ] **Step 10.1: 失敗するテストを書く**

`crates/core/src/deviation.rs` を上書き:

```rust
//! Deviation differences: what's missing/extra vs the golden.

use compact_str::CompactString;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Differences {
    pub missing_calls: Vec<CompactString>,
    pub extra_calls: Vec<CompactString>,
    pub missing_imports: Vec<CompactString>,
    pub extra_imports: Vec<CompactString>,
}

/// Both `golden` and `actual` MUST be sorted ascending.
pub fn diff_sets(golden: &[CompactString], actual: &[CompactString]) -> (Vec<CompactString>, Vec<CompactString>) {
    // missing = golden - actual, extra = actual - golden
    let (mut i, mut j) = (0_usize, 0_usize);
    let (mut missing, mut extra) = (Vec::new(), Vec::new());
    while i < golden.len() && j < actual.len() {
        match golden[i].cmp(&actual[j]) {
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => {
                missing.push(golden[i].clone());
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                extra.push(actual[j].clone());
                j += 1;
            }
        }
    }
    while i < golden.len() {
        missing.push(golden[i].clone());
        i += 1;
    }
    while j < actual.len() {
        extra.push(actual[j].clone());
        j += 1;
    }
    (missing, extra)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cs(items: &[&str]) -> Vec<CompactString> {
        let mut v: Vec<CompactString> = items.iter().map(|s| (*s).into()).collect();
        v.sort();
        v
    }

    #[test]
    fn identical_sets_have_no_diff() {
        let g = cs(&["useSWR"]);
        let a = cs(&["useSWR"]);
        let (m, e) = diff_sets(&g, &a);
        assert!(m.is_empty());
        assert!(e.is_empty());
    }

    #[test]
    fn missing_call_detected() {
        let g = cs(&["useSWR", "axios"]);
        let a = cs(&["useSWR"]);
        let (m, e) = diff_sets(&g, &a);
        assert_eq!(m, cs(&["axios"]));
        assert!(e.is_empty());
    }

    #[test]
    fn extra_call_detected() {
        let g = cs(&["useSWR"]);
        let a = cs(&["useSWR", "fetch"]);
        let (m, e) = diff_sets(&g, &a);
        assert!(m.is_empty());
        assert_eq!(e, cs(&["fetch"]));
    }

    #[test]
    fn both_missing_and_extra() {
        let g = cs(&["useSWR"]);
        let a = cs(&["fetch"]);
        let (m, e) = diff_sets(&g, &a);
        assert_eq!(m, cs(&["useSWR"]));
        assert_eq!(e, cs(&["fetch"]));
    }
}
```

- [ ] **Step 10.2: テスト実行**

Run: `cargo test -p lc-core --lib deviation::tests`
Expected: PASS (4 tests)

- [ ] **Step 10.3: 全 lc-core テストが通ることを確認**

Run: `cargo test -p lc-core`
Expected: PASS (全てのテスト 24+ 件)

- [ ] **Step 10.4: Commit**

```bash
git add crates/core/src/deviation.rs
git commit -m "feat(core/deviation): add Differences with diff_sets helper"
```

---

## Task 11: `lc-ts` crate scaffold

**Files:**
- Create: `crates/lc-ts/Cargo.toml`
- Create: `crates/lc-ts/src/lib.rs`
- Create: `crates/lc-ts/src/oxc_compat.rs`
- Create: `crates/lc-ts/src/extract.rs`
- Create: `crates/lc-ts/src/normalize.rs`
- Create: `crates/lc-ts/src/signature.rs`

- [ ] **Step 11.1: `crates/lc-ts/Cargo.toml` を作成**

```toml
[package]
name = "lc-ts"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
lc-core.workspace = true
oxc_parser.workspace = true
oxc_ast.workspace = true
oxc_allocator.workspace = true
oxc_span.workspace = true
compact_str.workspace = true
thiserror.workspace = true

[lints]
workspace = true
```

- [ ] **Step 11.2: 各 module ファイルを placeholder で作る**

Create `crates/lc-ts/src/oxc_compat.rs`:
```rust
// Implemented in Task 12.
```

Create `crates/lc-ts/src/normalize.rs`:
```rust
// Implemented in Task 13.
```

Create `crates/lc-ts/src/signature.rs`:
```rust
// Implemented in Task 14.
```

Create `crates/lc-ts/src/extract.rs`:
```rust
// Implemented in Task 15.
```

Create `crates/lc-ts/src/lib.rs`:
```rust
//! TypeScript / JavaScript adapter for layer-conform.
//!
//! Wraps oxc_parser and converts oxc AST into the neutral `lc_core::TreeNode` IR.
//! Public entrypoint is `parse_file`, defined in Task 16.

mod extract;
mod normalize;
mod oxc_compat;
mod signature;
```

- [ ] **Step 11.3: ビルド確認**

Run: `cargo build -p lc-ts`
Expected: PASS (warnings あれば許容、unused mod の warning は出る)

- [ ] **Step 11.4: Commit**

```bash
git add crates/lc-ts
git commit -m "feat(lc-ts): scaffold lc-ts crate with module skeleton"
```

---

## Task 12: `lc-ts::oxc_compat` — oxc parser 呼び出し集約

**Files:**
- Modify: `crates/lc-ts/src/oxc_compat.rs`

oxc_parser の呼び出しを 1 ファイルに閉じ込める (Anti-Corruption Layer)。Allocator はファイル単位で生成・drop する。

- [ ] **Step 12.1: 失敗するテストを書く**

`crates/lc-ts/src/oxc_compat.rs` を上書き:

```rust
//! Anti-Corruption Layer for oxc_parser API.
//! All `oxc_*` types are confined to this module.

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

/// Parse a TypeScript / JSX source. Allocator is owned by the caller and
/// must outlive the returned `Program<'a>`.
pub fn parse<'a>(allocator: &'a Allocator, source: &'a str) -> ParserReturn<'a> {
    let source_type = SourceType::default()
        .with_typescript(true)
        .with_jsx(true)
        .with_module(true);
    Parser::new(allocator, source, source_type).parse()
}

/// Convenience helper used in tests only.
#[cfg(test)]
pub fn parse_into(source: &str) -> (Allocator, String) {
    let alloc = Allocator::default();
    let ret = parse(&alloc, source);
    let body_count = format!("{} statements", ret.program.body.len());
    drop(ret);
    (alloc, body_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_source() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "");
        assert!(ret.errors.is_empty());
        assert_eq!(ret.program.body.len(), 0);
    }

    #[test]
    fn parses_function_declaration() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function foo() {}");
        assert!(ret.errors.is_empty());
        assert_eq!(ret.program.body.len(), 1);
    }

    #[test]
    fn parses_typescript_syntax() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function foo(x: number): string { return ''; }");
        assert!(ret.errors.is_empty());
    }
}
```

- [ ] **Step 12.2: テスト実行**

Run: `cargo test -p lc-ts --lib oxc_compat::tests`
Expected: PASS (3 tests)

> If oxc 0.73 の API が想定と異なる (`Parser::new` の引数順序、`SourceType` の builder メソッド名等) でテスト失敗する場合は、`crates/lc-ts/Cargo.toml` の `oxc_parser` の version とその docs.rs を確認して合わせる。これが出てきた場合は plan 全体ではなく `oxc_compat.rs` だけを修正すること (これが Anti-Corruption Layer の意義)。

- [ ] **Step 12.3: Commit**

```bash
git add crates/lc-ts/src/oxc_compat.rs
git commit -m "feat(lc-ts/oxc_compat): isolate oxc_parser invocation"
```

---

## Task 13: `lc-ts::normalize` — oxc AST → TreeNode 変換 (最小)

**Files:**
- Modify: `crates/lc-ts/src/normalize.rs`

MVP では `FunctionDeclaration` を変換する分だけ実装。識別子・リテラル・呼び出し・import の正規化ルールに従う (design §4.2)。

- [ ] **Step 13.1: 失敗するテストを書く**

`crates/lc-ts/src/normalize.rs` を上書き:

```rust
//! Convert oxc AST nodes into the neutral `lc_core::tree::TreeNode` IR.

use compact_str::CompactString;
use lc_core::tree::{NodeKind, TreeNode};
use oxc_ast::ast::{Expression, Statement};

const ANON_IDENT: &str = "_IDENT";
const ANON_LIT: &str = "_LIT";

/// Convert a function body (statement list) into a Block subtree.
pub fn normalize_block(body: &[Statement<'_>]) -> TreeNode {
    let children: Vec<TreeNode> = body.iter().map(normalize_statement).collect();
    let mut node = TreeNode::branch(NodeKind::Block, children);
    node.finalize();
    node
}

fn normalize_statement(stmt: &Statement<'_>) -> TreeNode {
    match stmt {
        Statement::ExpressionStatement(es) => normalize_expression(&es.expression),
        Statement::ReturnStatement(rs) => {
            let children = rs
                .argument
                .as_ref()
                .map(|e| vec![normalize_expression(e)])
                .unwrap_or_default();
            TreeNode::branch(NodeKind::Other, children)
        }
        _ => TreeNode::branch(NodeKind::Other, Vec::new()),
    }
}

fn normalize_expression(expr: &Expression<'_>) -> TreeNode {
    match expr {
        Expression::CallExpression(c) => {
            let mut children = vec![normalize_callee(&c.callee)];
            for arg in &c.arguments {
                if let Some(e) = arg.as_expression() {
                    children.push(normalize_expression(e));
                } else {
                    children.push(TreeNode::leaf(NodeKind::Other, None));
                }
            }
            TreeNode::branch(NodeKind::CallExpression, children)
        }
        Expression::Identifier(id) => {
            // 呼び出し先以外の bare identifier は匿名化。
            TreeNode::leaf(NodeKind::Identifier, Some(CompactString::new(ANON_IDENT)))
        }
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::TemplateLiteral(_) => {
            TreeNode::leaf(NodeKind::Literal, Some(CompactString::new(ANON_LIT)))
        }
        Expression::BooleanLiteral(b) => TreeNode::leaf(
            NodeKind::Literal,
            Some(CompactString::new(if b.value { "true" } else { "false" })),
        ),
        Expression::NullLiteral(_) => {
            TreeNode::leaf(NodeKind::Literal, Some(CompactString::new("null")))
        }
        _ => TreeNode::leaf(NodeKind::Other, None),
    }
}

fn normalize_callee(callee: &Expression<'_>) -> TreeNode {
    match callee {
        // 直接呼び出し: foo() → 名前を保持
        Expression::Identifier(id) => {
            TreeNode::leaf(NodeKind::Identifier, Some(CompactString::from(id.name.as_str())))
        }
        // メソッド呼び出し: a.b() → MemberExpression(a, b) として両方保持
        Expression::StaticMemberExpression(m) => {
            let object = match &m.object {
                Expression::Identifier(id) => TreeNode::leaf(
                    NodeKind::Identifier,
                    Some(CompactString::from(id.name.as_str())),
                ),
                other => normalize_expression(other),
            };
            let property = TreeNode::leaf(
                NodeKind::Identifier,
                Some(CompactString::from(m.property.name.as_str())),
            );
            TreeNode::branch(NodeKind::MemberExpression, vec![object, property])
        }
        other => normalize_expression(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oxc_compat::parse;
    use oxc_allocator::Allocator;

    #[test]
    fn empty_body_yields_block_with_zero_children() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() {}");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else {
            panic!("expected FunctionDeclaration");
        };
        let body = &f.body.as_ref().unwrap().statements;
        let tree = normalize_block(body);
        assert_eq!(tree.kind, NodeKind::Block);
        assert_eq!(tree.children.len(), 0);
        assert_eq!(tree.subtree_size, 1);
    }

    #[test]
    fn call_expression_preserves_callee_name() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { useSWR(); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else {
            panic!();
        };
        let body = &f.body.as_ref().unwrap().statements;
        let tree = normalize_block(body);
        // Block(CallExpression(Identifier("useSWR")))
        assert_eq!(tree.children.len(), 1);
        let call = &tree.children[0];
        assert_eq!(call.kind, NodeKind::CallExpression);
        assert_eq!(call.children[0].kind, NodeKind::Identifier);
        assert_eq!(call.children[0].value.as_deref(), Some("useSWR"));
    }

    #[test]
    fn member_call_preserves_both_names() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { axios.get(); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else {
            panic!();
        };
        let body = &f.body.as_ref().unwrap().statements;
        let tree = normalize_block(body);
        let call = &tree.children[0];
        // CallExpression(MemberExpression(axios, get))
        assert_eq!(call.children[0].kind, NodeKind::MemberExpression);
        assert_eq!(call.children[0].children[0].value.as_deref(), Some("axios"));
        assert_eq!(call.children[0].children[1].value.as_deref(), Some("get"));
    }

    #[test]
    fn local_identifier_is_anonymized() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { x; }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else {
            panic!();
        };
        let body = &f.body.as_ref().unwrap().statements;
        let tree = normalize_block(body);
        let id = &tree.children[0];
        assert_eq!(id.kind, NodeKind::Identifier);
        assert_eq!(id.value.as_deref(), Some(ANON_IDENT));
    }

    #[test]
    fn string_literal_is_anonymized() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { 'hello'; }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else {
            panic!();
        };
        let body = &f.body.as_ref().unwrap().statements;
        let tree = normalize_block(body);
        let lit = &tree.children[0];
        assert_eq!(lit.kind, NodeKind::Literal);
        assert_eq!(lit.value.as_deref(), Some(ANON_LIT));
    }
}
```

- [ ] **Step 13.2: テスト実行**

Run: `cargo test -p lc-ts --lib normalize::tests`
Expected: PASS (5 tests)

> oxc 0.73 で `Argument::as_expression()` や `StaticMemberExpression` の field 名が違う場合は、ここの実装だけ手直し (oxc API は ACL を貫通しないように)。

- [ ] **Step 13.3: Commit**

```bash
git add crates/lc-ts/src/normalize.rs
git commit -m "feat(lc-ts/normalize): convert oxc AST to TreeNode for MVP cases"
```

---

## Task 14: `lc-ts::signature` — calls / imports / signature 抽出

**Files:**
- Modify: `crates/lc-ts/src/signature.rs`

関数本体を walk して呼び出し関数名集合と、ファイル全体から import 名集合を抽出する。signature は引数の数だけを最小情報として保持する。

- [ ] **Step 14.1: 失敗するテストを書く**

`crates/lc-ts/src/signature.rs` を上書き:

```rust
//! Extract calls / imports / signature from oxc AST.

use compact_str::CompactString;
use oxc_ast::ast::{Expression, Program, Statement};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Signature {
    pub param_count: u32,
}

/// Walk a function body and collect callee names.
/// Returns a sorted, deduplicated `Vec<CompactString>`.
pub fn collect_calls(body: &[Statement<'_>]) -> Vec<CompactString> {
    let mut acc = Vec::new();
    for s in body {
        walk_statement(s, &mut acc);
    }
    acc.sort();
    acc.dedup();
    acc
}

/// Collect top-level import sources from a program.
pub fn collect_imports(program: &Program<'_>) -> Vec<CompactString> {
    let mut acc = Vec::new();
    for s in &program.body {
        if let Statement::ImportDeclaration(imp) = s {
            acc.push(CompactString::from(imp.source.value.as_str()));
            for spec in imp.specifiers.iter().flatten() {
                if let Some(name) = local_name_of_specifier(spec) {
                    acc.push(name);
                }
            }
        }
    }
    acc.sort();
    acc.dedup();
    acc
}

fn local_name_of_specifier(spec: &oxc_ast::ast::ImportDeclarationSpecifier<'_>) -> Option<CompactString> {
    use oxc_ast::ast::ImportDeclarationSpecifier as S;
    match spec {
        S::ImportSpecifier(s) => Some(CompactString::from(s.imported.name().as_str())),
        S::ImportDefaultSpecifier(s) => Some(CompactString::from(s.local.name.as_str())),
        S::ImportNamespaceSpecifier(s) => Some(CompactString::from(s.local.name.as_str())),
    }
}

fn walk_statement(s: &Statement<'_>, acc: &mut Vec<CompactString>) {
    match s {
        Statement::ExpressionStatement(es) => walk_expression(&es.expression, acc),
        Statement::ReturnStatement(rs) => {
            if let Some(e) = &rs.argument {
                walk_expression(e, acc);
            }
        }
        Statement::BlockStatement(b) => {
            for s in &b.body {
                walk_statement(s, acc);
            }
        }
        _ => {}
    }
}

fn walk_expression(e: &Expression<'_>, acc: &mut Vec<CompactString>) {
    if let Expression::CallExpression(c) = e {
        if let Some(name) = callee_name(&c.callee) {
            acc.push(name);
        }
        for arg in &c.arguments {
            if let Some(inner) = arg.as_expression() {
                walk_expression(inner, acc);
            }
        }
    }
}

fn callee_name(e: &Expression<'_>) -> Option<CompactString> {
    match e {
        Expression::Identifier(id) => Some(CompactString::from(id.name.as_str())),
        Expression::StaticMemberExpression(m) => {
            // a.b → "a.b"
            if let Expression::Identifier(obj) = &m.object {
                let mut s = CompactString::from(obj.name.as_str());
                s.push('.');
                s.push_str(m.property.name.as_str());
                Some(s)
            } else {
                Some(CompactString::from(m.property.name.as_str()))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oxc_compat::parse;
    use oxc_allocator::Allocator;

    #[test]
    fn collects_simple_call() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { useSWR(); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        assert_eq!(collect_calls(body), vec![CompactString::from("useSWR")]);
    }

    #[test]
    fn collects_member_call_as_dotted() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { axios.get(); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        assert_eq!(collect_calls(body), vec![CompactString::from("axios.get")]);
    }

    #[test]
    fn collects_calls_sorted_and_deduped() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function f() { b(); a(); a(); }");
        let Statement::FunctionDeclaration(f) = &ret.program.body[0] else { panic!() };
        let body = &f.body.as_ref().unwrap().statements;
        assert_eq!(
            collect_calls(body),
            vec![CompactString::from("a"), CompactString::from("b")]
        );
    }

    #[test]
    fn collects_imports_with_specifiers() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "import { useSWR } from 'swr'; function f() {}");
        let imports = collect_imports(&ret.program);
        // sorted: "swr", "useSWR"
        assert_eq!(
            imports,
            vec![CompactString::from("swr"), CompactString::from("useSWR")]
        );
    }
}
```

- [ ] **Step 14.2: テスト実行**

Run: `cargo test -p lc-ts --lib signature::tests`
Expected: PASS (4 tests)

- [ ] **Step 14.3: Commit**

```bash
git add crates/lc-ts/src/signature.rs
git commit -m "feat(lc-ts/signature): extract calls and imports as sorted vectors"
```

---

## Task 15: `lc-ts::extract` — `FunctionDeclaration` 抽出 + `FunctionRef` 構築

**Files:**
- Modify: `crates/lc-ts/src/extract.rs`
- Modify: `crates/core/src/lib.rs` (公開 `FunctionRef` を追加)

`FunctionRef` 型を `lc-core` に置き、それを `lc-ts::extract` で構築する。

- [ ] **Step 15.1: `lc-core` に `FunctionRef` を追加**

`crates/core/src/lib.rs` を上書き:

```rust
//! Pure logic core for layer-conform.

pub mod apted;
pub mod deviation;
pub mod function_ref;
pub mod similarity;
pub mod tree;
pub mod tsed;

pub use function_ref::{FunctionKind, FunctionRef, Signature};
```

Create `crates/core/src/function_ref.rs`:

```rust
//! Per-function metadata returned by language analyzers.

use compact_str::CompactString;

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
    pub symbol: CompactString,         // "useUser" / "UserService.create"
    pub kind: FunctionKind,
    pub start_line: u32,
    pub end_line: u32,
    pub byte_range: (u32, u32),
    pub tree: TreeNode,
    pub signature: Signature,
    pub calls: Vec<CompactString>,     // sorted
    pub imports: Vec<CompactString>,   // sorted (file-level)
    pub ast_hash: [u8; 32],
}
```

- [ ] **Step 15.2: lc-core build 確認**

Run: `cargo build -p lc-core`
Expected: PASS

- [ ] **Step 15.3: 失敗するテストを書く**

`crates/lc-ts/src/extract.rs` を上書き:

```rust
//! Extract `FunctionDeclaration` from a TS/JS source. MVP: only top-level
//! function declarations are supported. Other kinds (Arrow / Method / ...)
//! are added in Plan #2.

use lc_core::{
    tree::{NodeKind, TreeNode},
    FunctionKind, FunctionRef, Signature,
};
use oxc_ast::ast::Statement;

use crate::{normalize, signature};

pub fn extract_function_declarations(source: &str) -> Vec<FunctionRef> {
    let alloc = oxc_allocator::Allocator::default();
    let ret = crate::oxc_compat::parse(&alloc, source);
    let program = &ret.program;
    let imports = signature::collect_imports(program);

    let mut out = Vec::new();
    for stmt in &program.body {
        if let Statement::FunctionDeclaration(decl) = stmt {
            let Some(name) = decl.id.as_ref().map(|i| i.name.as_str()) else { continue };
            let body = decl.body.as_ref().map(|b| b.statements.as_slice()).unwrap_or(&[]);
            let mut tree = normalize::normalize_block(body);
            tree.finalize();
            let ast_hash = tree.canonical_hash();
            let calls = signature::collect_calls(body);
            let span = decl.span;
            out.push(FunctionRef {
                symbol: name.into(),
                kind: FunctionKind::FunctionDeclaration,
                start_line: 0,
                end_line: 0,
                byte_range: (span.start, span.end),
                tree,
                signature: Signature { param_count: decl.params.items.len() as u32 },
                calls,
                imports: imports.clone(),
                ast_hash,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_no_function_when_none_present() {
        let v = extract_function_declarations("const x = 1;");
        assert!(v.is_empty());
    }

    #[test]
    fn extracts_single_function_declaration() {
        let v = extract_function_declarations("function useUser() { return useSWR('/u'); }");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].symbol.as_str(), "useUser");
        assert_eq!(v[0].kind, FunctionKind::FunctionDeclaration);
        assert_eq!(v[0].calls, vec![compact_str::CompactString::from("useSWR")]);
    }

    #[test]
    fn skips_arrow_functions_in_mvp() {
        // Plan #2 で対応。MVP では拾わない。
        let v = extract_function_declarations("const useUser = () => useSWR('/u');");
        assert!(v.is_empty());
    }

    #[test]
    fn captures_param_count() {
        let v = extract_function_declarations("function f(a, b, c) {}");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].signature.param_count, 3);
    }

    #[test]
    fn captures_imports_at_file_level() {
        let src = "import { useSWR } from 'swr';\nfunction f() {}";
        let v = extract_function_declarations(src);
        assert_eq!(v.len(), 1);
        assert!(v[0].imports.iter().any(|s| s == "swr"));
        assert!(v[0].imports.iter().any(|s| s == "useSWR"));
    }
}
```

- [ ] **Step 15.4: テスト実行**

Run: `cargo test -p lc-ts --lib extract::tests`
Expected: PASS (5 tests)

- [ ] **Step 15.5: Commit**

```bash
git add crates/core/src/lib.rs crates/core/src/function_ref.rs crates/lc-ts/src/extract.rs
git commit -m "feat(lc-ts/extract): extract FunctionDeclaration into FunctionRef"
```

---

## Task 16: `lc-ts::lib` — `parse_file` 公開エントリ

**Files:**
- Modify: `crates/lc-ts/src/lib.rs`

CLI から呼ぶ単一のエントリポイントを公開する。

- [ ] **Step 16.1: `lc-ts/src/lib.rs` を更新**

```rust
//! TypeScript / JavaScript adapter for layer-conform.
//!
//! Wraps oxc_parser and converts oxc AST into the neutral `lc_core::TreeNode` IR.

mod extract;
mod normalize;
mod oxc_compat;
mod signature;

use lc_core::FunctionRef;

/// Parse a TS/JS source string and return all extractable functions.
/// MVP supports only `FunctionDeclaration`.
pub fn parse_file(source: &str) -> Vec<FunctionRef> {
    extract::extract_function_declarations(source)
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
```

- [ ] **Step 16.2: テスト実行**

Run: `cargo test -p lc-ts`
Expected: PASS (全テスト)

- [ ] **Step 16.3: Commit**

```bash
git add crates/lc-ts/src/lib.rs
git commit -m "feat(lc-ts): expose parse_file as the single public entry"
```

---

## Task 17: `cli` crate scaffold + clap derive

**Files:**
- Create: `crates/cli/Cargo.toml`
- Create: `crates/cli/src/main.rs`
- Create: `crates/cli/src/args.rs`
- Create: `crates/cli/src/reporter.rs`
- Create: `crates/cli/src/runner.rs`

- [ ] **Step 17.1: `crates/cli/Cargo.toml` を作成**

```toml
[package]
name = "layer-conform"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[[bin]]
name = "layer-conform"
path = "src/main.rs"

[dependencies]
lc-core.workspace = true
lc-ts.workspace = true
clap.workspace = true
anyhow.workspace = true
compact_str.workspace = true

[dev-dependencies]
assert_cmd.workspace = true
predicates.workspace = true
tempfile.workspace = true

[lints]
workspace = true
```

- [ ] **Step 17.2: `crates/cli/src/args.rs` を作成**

```rust
//! CLI argument parsing.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "layer-conform", version, about = "Detect layer style deviations")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Compare a single function against a single golden (MVP).
    Check {
        /// Source file containing the function to check.
        #[arg(long)]
        file: PathBuf,
        /// Symbol name within `--file` to check.
        #[arg(long)]
        symbol: String,
        /// Golden in the form "<path>:<symbol>".
        #[arg(long)]
        golden: String,
    },
}
```

- [ ] **Step 17.3: `crates/cli/src/reporter.rs` を作成**

```rust
//! Text reporter for MVP single-pair comparison output.

use lc_core::similarity::SimilarityScore;

pub struct Report<'a> {
    pub file: &'a str,
    pub symbol: &'a str,
    pub golden_file: &'a str,
    pub golden_symbol: &'a str,
    pub score: SimilarityScore,
    pub missing_calls: &'a [compact_str::CompactString],
    pub extra_calls: &'a [compact_str::CompactString],
}

impl<'a> Report<'a> {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "{}:{} vs {}:{}\n",
            self.file, self.symbol, self.golden_file, self.golden_symbol
        ));
        out.push_str(&format!(
            "  overall={:.3}  shape={:.3}  calls={:.3}  imports={:.3}  signature={:.3}\n",
            self.score.overall,
            self.score.shape,
            self.score.calls,
            self.score.imports,
            self.score.signature,
        ));
        if !self.missing_calls.is_empty() {
            out.push_str(&format!("  missing calls: {:?}\n", self.missing_calls));
        }
        if !self.extra_calls.is_empty() {
            out.push_str(&format!("  extra calls:   {:?}\n", self.extra_calls));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compact_str::CompactString;

    #[test]
    fn renders_header_and_scores() {
        let r = Report {
            file: "a.ts",
            symbol: "useFoo",
            golden_file: "b.ts",
            golden_symbol: "useBar",
            score: SimilarityScore { overall: 0.5, shape: 0.6, calls: 0.4, imports: 1.0, signature: 0.0 },
            missing_calls: &[CompactString::from("useSWR")],
            extra_calls: &[CompactString::from("fetch")],
        };
        let s = r.render();
        assert!(s.contains("a.ts:useFoo"));
        assert!(s.contains("b.ts:useBar"));
        assert!(s.contains("overall=0.500"));
        assert!(s.contains("missing calls"));
        assert!(s.contains("extra calls"));
    }
}
```

- [ ] **Step 17.4: `crates/cli/src/runner.rs` を placeholder で作成**

```rust
// Implemented in Task 18.
```

- [ ] **Step 17.5: `crates/cli/src/main.rs` を作成**

```rust
//! layer-conform CLI entrypoint.

mod args;
mod reporter;
mod runner;

use clap::Parser;

use crate::args::{Cli, Cmd};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Check { file, symbol, golden } => runner::run_check(file, &symbol, &golden),
    }
}
```

- [ ] **Step 17.6: ビルド確認**

Run: `cargo build -p layer-conform`
Expected: FAIL — `runner::run_check` 未実装。これは Task 18 で実装するので OK。`cargo check -p layer-conform` も通らない。

> このタスクではビルドを通さなくて良い (Task 18 で run_check を実装するため)。次タスクに進む。

- [ ] **Step 17.7: 部分 commit (build 失敗状態だが OK)**

```bash
git add crates/cli
git commit -m "feat(cli): scaffold CLI crate with clap-based check subcommand"
```

> **build が通らないので commit したくない場合**: Task 18 まで進めてから 1 度に commit してもよい。

---

## Task 18: `cli::runner::run_check` — 1 ペア比較ロジック

**Files:**
- Modify: `crates/cli/src/runner.rs`

`run_check` は `--file` `--symbol` と `--golden <path:symbol>` を受けて、両方をパース→ FunctionRef を取得→ 4 軸で類似度を計算→ レポーターで出力する。

- [ ] **Step 18.1: 失敗するテストを書く (cli ユニット)**

`crates/cli/src/runner.rs` を上書き:

```rust
//! Implements `layer-conform check` for a single (file, symbol) vs golden pair.

use std::{fs, path::PathBuf};

use anyhow::{anyhow, Context};
use lc_core::{
    deviation::diff_sets,
    similarity::{aggregate, jaccard_sorted, Weights},
    tsed,
    FunctionRef,
};

use crate::reporter::Report;

pub fn run_check(file: PathBuf, symbol: &str, golden_spec: &str) -> anyhow::Result<()> {
    let (g_path, g_symbol) = parse_golden_spec(golden_spec)?;

    let actual = load_function(&file, symbol)?;
    let golden = load_function(&PathBuf::from(&g_path), &g_symbol)?;

    let shape = tsed::tsed(&actual.tree, &golden.tree);
    let calls = jaccard_sorted(&actual.calls, &golden.calls);
    let imports = jaccard_sorted(&actual.imports, &golden.imports);
    let signature = if actual.signature == golden.signature { 1.0 } else { 0.0 };
    let score = aggregate(shape, calls, imports, signature, Weights::default());

    let (missing_calls, extra_calls) = diff_sets(&golden.calls, &actual.calls);

    let report = Report {
        file: file.to_str().unwrap_or("<file>"),
        symbol,
        golden_file: &g_path,
        golden_symbol: &g_symbol,
        score,
        missing_calls: &missing_calls,
        extra_calls: &extra_calls,
    };
    print!("{}", report.render());
    Ok(())
}

fn parse_golden_spec(spec: &str) -> anyhow::Result<(String, String)> {
    let (path, name) = spec
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("--golden must be \"<path>:<symbol>\", got {spec}"))?;
    if path.is_empty() || name.is_empty() {
        return Err(anyhow!("--golden has empty part: {spec}"));
    }
    Ok((path.to_string(), name.to_string()))
}

fn load_function(path: &PathBuf, symbol: &str) -> anyhow::Result<FunctionRef> {
    let src = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let funcs = lc_ts::parse_file(&src);
    funcs
        .into_iter()
        .find(|f| f.symbol == symbol)
        .ok_or_else(|| anyhow!("symbol `{symbol}` not found in {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_golden_spec_splits_on_last_colon() {
        let (p, s) = parse_golden_spec("src/foo.ts:useFoo").unwrap();
        assert_eq!(p, "src/foo.ts");
        assert_eq!(s, "useFoo");
    }

    #[test]
    fn parse_golden_spec_rejects_missing_colon() {
        assert!(parse_golden_spec("foo").is_err());
    }

    #[test]
    fn parse_golden_spec_rejects_empty_part() {
        assert!(parse_golden_spec(":foo").is_err());
        assert!(parse_golden_spec("foo:").is_err());
    }
}
```

- [ ] **Step 18.2: テスト実行**

Run: `cargo test -p layer-conform --lib runner::tests`
Expected: PASS (3 tests)

- [ ] **Step 18.3: ビルド全体確認**

Run: `cargo build --workspace`
Expected: PASS

- [ ] **Step 18.4: Commit**

```bash
git add crates/cli/src/runner.rs
git commit -m "feat(cli/runner): implement check command with 4-axis comparison"
```

---

## Task 19: 統合テスト — conform ケース

**Files:**
- Create: `crates/cli/tests/integration.rs`

CLI バイナリを `assert_cmd` で起動し、自身と完全一致するゴールデンを比較すると `overall=1.000` が出ることを検証する。

- [ ] **Step 19.1: 統合テスト 1 ケース目を書く**

Create `crates/cli/tests/integration.rs`:

```rust
//! End-to-end integration tests for the `layer-conform` binary.

use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

fn write(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).unwrap();
}

#[test]
fn check_reports_overall_one_for_identical_function() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.ts");
    write(
        &file,
        "import { useSWR } from 'swr';\nfunction useFoo() { return useSWR('/x'); }\n",
    );

    Command::cargo_bin("layer-conform")
        .unwrap()
        .args([
            "check",
            "--file",
            file.to_str().unwrap(),
            "--symbol",
            "useFoo",
            "--golden",
        ])
        .arg(format!("{}:useFoo", file.display()))
        .assert()
        .success()
        .stdout(contains("overall=1.000"));
}
```

- [ ] **Step 19.2: テスト実行**

Run: `cargo test -p layer-conform --test integration`
Expected: PASS

- [ ] **Step 19.3: Commit**

```bash
git add crates/cli/tests/integration.rs
git commit -m "test(cli): integration test for conform case"
```

---

## Task 20: 統合テスト — deviation ケース

**Files:**
- Modify: `crates/cli/tests/integration.rs`

異なる流儀の関数 (片方は `useSWR`、もう片方は `fetch`) を比較し、`missing calls` と `extra calls` が正しく出ることを検証する。

- [ ] **Step 20.1: 2 ケース目を追加**

`crates/cli/tests/integration.rs` の末尾に追加:

```rust
#[test]
fn check_reports_missing_and_extra_calls_for_divergent_styles() {
    let dir = tempdir().unwrap();
    let golden = dir.path().join("golden.ts");
    let actual = dir.path().join("actual.ts");
    write(
        &golden,
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(
        &actual,
        "function useActual() { return fetch('/x'); }\n",
    );

    Command::cargo_bin("layer-conform")
        .unwrap()
        .args([
            "check",
            "--file",
            actual.to_str().unwrap(),
            "--symbol",
            "useActual",
            "--golden",
        ])
        .arg(format!("{}:useGolden", golden.display()))
        .assert()
        .success()
        .stdout(contains("missing calls"))
        .stdout(contains("useSWR"))
        .stdout(contains("extra calls"))
        .stdout(contains("fetch"));
}

#[test]
fn check_fails_when_symbol_not_found() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.ts");
    write(&file, "function existing() {}\n");

    Command::cargo_bin("layer-conform")
        .unwrap()
        .args([
            "check",
            "--file",
            file.to_str().unwrap(),
            "--symbol",
            "missing_symbol",
            "--golden",
        ])
        .arg(format!("{}:existing", file.display()))
        .assert()
        .failure();
}
```

- [ ] **Step 20.2: テスト実行**

Run: `cargo test -p layer-conform --test integration`
Expected: PASS (3 tests)

- [ ] **Step 20.3: 全 workspace test 実行**

Run: `cargo test --workspace`
Expected: PASS (全テスト)

- [ ] **Step 20.4: clippy 確認**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

> warnings が出る場合: 内容を見て、Plan に書かれている設計の本質に関わるものは設計を修正、関わらないものは局所的に `#[allow(clippy::xxx)]` を付ける。`pedantic` 周りで多めに warnings が出ることが想定されるが、`workspace.lints` で `pedantic = "warn"` にしているので fail はしない。`-D warnings` を `-W warnings` に緩めても良い。

- [ ] **Step 20.5: rustfmt 確認**

Run: `cargo fmt --all -- --check`
Expected: PASS

- [ ] **Step 20.6: Commit**

```bash
git add crates/cli/tests/integration.rs
git commit -m "test(cli): integration tests for deviation and missing-symbol cases"
```

---

## Task 21: ドキュメント整備 (README)

**Files:**
- Create: `README.md`

- [ ] **Step 21.1: 最小 README を書く**

Create `/workspaces/github.com/9wick/layer-conform/README.md`:

````markdown
# layer-conform

Detect "style deviations" within a layer of a TypeScript/JavaScript project — i.e. find functions that look different from the rest of their layer.

This MVP only supports a single-pair comparison via CLI flags.

## Build

```sh
cargo build --release
```

## Usage (MVP)

```sh
layer-conform check \
  --file src/repositories/useProduct.ts \
  --symbol useProduct \
  --golden src/repositories/useUser.ts:useUser
```

Output:

```
src/repositories/useProduct.ts:useProduct vs src/repositories/useUser.ts:useUser
  overall=0.412  shape=0.380  calls=0.000  imports=0.500  signature=1.000
  missing calls: ["useSWR"]
  extra calls:   ["fetch", "useEffect", "useState"]
```

## Status

- ✅ Phase 1a: lc-core (APTED + TSED + 4-axis similarity)
- ✅ Phase 1b: lc-ts (FunctionDeclaration only) + CLI 1-pair compare
- ⏳ Phase 2: config-driven, full extraction (Arrow/Method/etc.), `--explain` / `why`, `init`
- ⏳ Phase 3: baseline, `--changed`, `--summary`
- ⏳ Phase 4: `init --auto`, multi-language, distribution

See `docs/superpowers/specs/2026-04-26-layer-conform-design.md`.
````

- [ ] **Step 21.2: Commit**

```bash
git add README.md
git commit -m "docs: add README with MVP usage"
```

---

## Self-Review Checklist (実行前に確認)

実装着手前にこのリストを 1 周する。

### Spec coverage

design doc (`docs/superpowers/specs/2026-04-26-layer-conform-design.md`) のうち本 plan に含まれるもの:

- §2 技術スタック: Cargo workspace / oxc 0.73 pin / clap / blake3 → Task 1, 11
- §3 リポジトリ構成: 3 crate (lc-io は除く) → Task 1, 2, 11, 17
- §4.2 TreeNode (NodeKind / id / subtree_size / canonical_hash) → Task 3, 4, 5, 6
- §4.3 FunctionRef (FunctionDeclaration のみ MVP) → Task 15
- §4.4 SimilarityScore (4 軸) と Differences → Task 9, 10
- §5 パイプライン (1 ペア比較版): parse → 4 軸算出 → text 出力 → Task 18
- §6 CLI: `check` サブコマンドの最小形 → Task 17, 18
- §9.1 ユニットテスト → 各 Task に組込
- §9.2 統合テスト (conform / deviation) → Task 19, 20

本 plan に**含まれない** design 項目 (Plan #2 以降):

- 4.1 設定 JSON、4.5 baseline、§6 ignore コメント、§7 多言語 trait、§5 パイプライン (golden 解決・matcher・ignore)、`why`、`init`、`--summary`、`--json`、git 連携、`init --auto`

### Placeholder scan

- 各 Task に `TODO`/`TBD`/`fill in details` などの曖昧な指示が無いか — 無い (確認済)
- code 変更ステップは全て完全なコードブロックを含むか — Yes
- 「Task N と同じ」という参照のみ箇所が無いか — 無い

### Type consistency

- `TreeNode { kind, value, children, id, subtree_size }` を Task 4-6 と Task 13, 15 で一貫使用 — Yes
- `FunctionRef { symbol, kind, ..., calls, imports, ast_hash }` を Task 15, 16, 18 で一貫使用 — Yes
- `SimilarityScore { overall, shape, calls, imports, signature }` を Task 9, 17, 18 で一貫使用 — Yes
- `aggregate(shape, calls, imports, signature, Weights)` のシグネチャ Task 9 と Task 18 で一致 — Yes

### oxc 0.73 互換性

oxc API は breaking change が多い。Task 12, 13, 14 でテストが落ちる場合は **`oxc_compat.rs` と `normalize.rs` の oxc 呼び出し部分のみ** を修正し、TreeNode / FunctionRef のシグネチャ等は変えない (これが ACL の意義)。

具体的に揺れやすい点:
- `Parser::new()` の引数順
- `SourceType` のビルダーメソッド (`with_typescript` `with_jsx` `with_module`)
- `Argument::as_expression()` の戻り値型
- `StaticMemberExpression` の field 名 (`object` / `property`)
- `ImportDeclarationSpecifier` の variant 名

oxc 0.73.0 の docs.rs を一度開いてシグネチャを確認するのが確実。

---

## Execution

実装着手時は以下の方法を選ぶ:

1. **Subagent-Driven (推奨)** — タスクごとに新しい subagent を起動、レビューを挟む
2. **Inline Execution** — このセッション内で順次実行、checkpoint レビュー

選んだ方を起動するときに、`commit step は CLAUDE.md の規定で明示承認制` であることを subagent に伝えること。
