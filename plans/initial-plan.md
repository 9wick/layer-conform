# layer-conform 初期実装プラン

## 0. ゴールと非ゴール

### ゴール
- TypeScript/JavaScript プロジェクトの「同一 layer 内のコード流儀統一」を AST 類似度ベースで検査する CLI ツールを Rust で実装する
- 利用者が `golden` (規範関数) を JSON 設定で指定し、`applyTo` の glob にマッチする関数群と AST 類似度を比較する
- 一定閾値を下回った関数を「逸脱」として報告する
- `mizchi/similarity` のアーキテクチャ (Cargo workspace, oxc_parser, APTED, rayon 並列) を踏襲する

### 非ゴール (今回のスコープ外)
- Python/Go など TS/JS 以外の言語パーサー (将来拡張可能な構造だけ確保)
- 自動修正 (golden に揃える書き換え)
- LSP/エディタ統合

---

## 1. 技術スタック (確定)

| 項目 | 採用 | 備考 |
|---|---|---|
| 実装言語 | **Rust** (edition 2021) | similarity と同じ |
| ビルド | **Cargo workspace** | crates 分割 |
| TS/JS パーサー | **oxc_parser** `=0.73.0` exact pin | breaking change 多発、Cargo.lock もコミット。oxc API は `lc-ts/src/oxc_compat.rs` に閉じ込める (Anti-Corruption Layer) |
| 類似度アルゴリズム | **APTED + TSED 自前実装** | similarity から移植 |
| CLI | **clap v4** (derive) | similarity と同じ |
| 並列処理 | **rayon** | similarity と同じ。TreeNode は `Send + Sync` 要件あり (`Rc` ではなく `Box`/`Arc`) |
| ファイル走査 | **walkdir + ignore + globset** | .gitignore 尊重 |
| シリアライズ | **serde + serde_json** | 設定/JSON 出力 |
| ハッシュ | **blake3** | baseline 用。SIMD 最適化、SHA-256 より高速 |
| 設定ファイル形式 | **JSON のみ** (`.layer-conform.json`) | ユーザー指定。.ts/.js は不採用 |
| baseline 形式 | **JSON** (`.layer-conform-baseline.json`) | hashFormat バージョン付きで保存 |
| エラーハンドリング | **anyhow + thiserror** | アプリ側 anyhow、ライブラリ側 thiserror |
| 対応 OS | **Linux / macOS** のみ | Windows は対象外 (CI でもテストしない、パス区切り正規化も不要) |

---

## 2. リポジトリ構成

```
layer-conform/
├── Cargo.toml                   # workspace ルート
├── rust-toolchain.toml          # 1.80+ 想定
├── README.md
├── plans/
├── crates/
│   ├── core/                    # 言語非依存コア (純粋ロジックのみ、I/O は持たない)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── apted.rs         # 木編集距離
│   │       ├── tsed.rs          # 正規化スコア
│   │       ├── tree.rs          # TreeNode (中立 IR、NodeKind enum + id + subtree_size)
│   │       ├── fingerprint.rs   # AST fingerprint (Phase 2.5 でのみ実装)
│   │       ├── matcher.rs       # rule × file マッチング (純粋関数)
│   │       ├── deviation.rs     # 逸脱データ構造 + 差分計算 (calls/imports/signature 分解)
│   │       ├── ignore_parse.rs  # コメントトークン列から ignore directive を抽出 (純粋)
│   │       ├── language.rs      # LanguageAnalyzer trait
│   │       └── pipeline.rs      # Iterator<(PathBuf, source)> を入力に取る純関数オーケストレータ
│   ├── lc-io/                   # I/O 専用 crate (config/baseline/git/ファイル列挙)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config.rs        # `.layer-conform.json` ロード/正規化
│   │       ├── baseline.rs      # baseline JSON ロード/保存
│   │       ├── git.rs           # --changed / --since (git2 もしくは git CLI 呼び出し)
│   │       └── walker.rs        # walkdir + ignore + globset
│   ├── lc-ts/                   # TypeScript/JavaScript アダプター
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs           # LanguageAnalyzer 実装
│   │       ├── oxc_compat.rs    # oxc API を集約 (Anti-Corruption Layer。oxc bump 時はここだけ修正)
│   │       ├── parser.rs        # oxc 呼び出し (Allocator はファイル単位で生成・破棄)
│   │       ├── extract.rs       # 関数抽出 (種別ごとに分割: extract/{function_decl, arrow, method, ...})
│   │       ├── normalize.rs     # oxc AST → TreeNode (識別子の所有化)
│   │       └── signature.rs     # シグネチャ・呼び出し集合・import 抽出
│   └── cli/                     # CLI バイナリ
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── args.rs          # clap 定義
│           ├── commands/
│           │   ├── mod.rs
│           │   ├── check.rs     # default + check <file>
│           │   ├── init.rs
│           │   ├── why.rs
│           │   └── baseline.rs
│           └── reporter/
│               ├── mod.rs
│               ├── text.rs      # デフォルト出力
│               ├── summary.rs   # --summary
│               └── json.rs      # --json
└── tests/                       # ワークスペース横断 統合テスト
    └── fixtures/
        ├── repositories/        # サンプル layer
        ├── usecases/
        └── configs/
```

### crate 名と公開名

- `lc-core` (lib): 純粋ロジック (AST/類似度/データモデル)。ファイル I/O 一切なし → LSP/GitHub Action からも再利用可
- `lc-io` (lib): ファイル I/O (設定 JSON、baseline、glob 走査、git diff)。`std::fs` `git2` 依存はここに集約
- `lc-ts` (lib): TS/JS アダプター。`oxc_*` 依存はここに集約
- `layer-conform` (bin): CLI バイナリ。`cargo install layer-conform` で入る

### 依存方向

```
cli ──→ lc-io ──→ lc-core
        └────→ lc-ts ──→ lc-core
```

`lc-core` は他 crate に依存しない (oxc, fs, git, serde_json も使わない)。`lc-core` 単体で完結したテストが書けるよう型を設計する。

---

## 3. データモデル

### 3.1 設定 (`.layer-conform.json`)

```json
{
  "$schema": "https://example.com/layer-conform.schema.json",
  "version": 1,
  "rules": [
    {
      "id": "repositories",
      "golden": { "file": "src/repositories/useUser.ts", "symbol": "useUser" },
      "applyTo": "src/repositories/**/*.ts",
      "threshold": 0.7,
      "ignore": "src/repositories/legacy/**",
      "disabled": false
    }
  ]
}
```

- ルートはオブジェクトに変更 (将来 `defaults` フィールド等を足せるよう)
- `rules` 配列が仕様書の Config 配列に相当
- **`rules[i].id` は必須**。summary・baseline・JSON 出力・将来の rule マイグレーションで配列順序非依存にするため。利用者は短い snake_case 文字列を付ける (例: `"repositories"` `"usecases"`)
- `applyTo` / `ignore` は `string | string[]` 両対応 (serde の untagged enum)
- **`golden` の型** (3 形式を許容):
  - `string`: 仕様書互換のショートハンド `"path:symbol"` (例: `"src/foo.ts:useFoo"`)。内部で `GoldenSelector` に try_from で正規化
  - `object`: `{ "file": "...", "symbol": "..." }` 構造体
  - `array`: 上記 2 形式を混ぜた配列 (multi-golden)
- **`golden.symbol` の文法** (関数特定の正式仕様):
  - `useUser`: トップレベル関数 (FunctionDeclaration / VariableDeclarator+Arrow)
  - `UserService.create`: クラスメソッド / オブジェクトメソッド (`.` 区切り、最大 1 段ネスト)
  - `default`: `export default function` / `export default () =>`
  - `default:UserService.create`: default export object 内のメソッド (将来拡張、初期は未対応)
  - シンボルが見つからなければエラー (config 起動時 fail)

### 3.2 中立 AST (TreeNode)

```rust
#[repr(u32)] // discriminant 安定化 (baseline hash 互換性のため)
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum NodeKind {
    Program,
    FunctionDeclaration,
    ArrowFunction,
    Method,
    CallExpression,
    MemberExpression,
    JsxElement,
    Identifier,           // value に正規化文字列 (呼び出し名 or _IDENT)
    Literal,              // value に _LIT (匿名化済み) または "true"/"false"/"null"/"undefined"
    ImportSpecifier,      // value に import 名
    // ... 必要分だけ追加
}

pub struct TreeNode {
    pub kind: NodeKind,
    pub value: Option<CompactString>,     // 動的識別子 (Identifier の正規化値、ImportSpecifier 名など)
    pub children: Vec<Box<TreeNode>>,     // Send + Sync を維持 (rayon 並列のため Rc は不可)
    pub id: u32,                          // ノードに連番。APTED の memoize key 用
    pub subtree_size: u32,                // 構築時に bottom-up で確定。APTED ループ内 O(1) 参照のため
}
```

- **`kind`** はノードの静的種別 (enum の variant)、**`value`** は動的文字列 (識別子・リテラル) を分離して保持。`label: &'static str` 一本では識別子が入らず APTED 移植と矛盾する問題を解消
- **`children: Vec<Box<TreeNode>>`** で `Send + Sync` を確保し、rayon でファイル並列が成立する。子の共有が要らないので `Rc/Arc` は使わない
- **`id`** は親トラバース順の連番 (`u32` で十分)。similarity の APTED で必須の memoize key
- **`subtree_size`** は構築時に `node.subtree_size = 1 + children.iter().map(|c| c.subtree_size).sum()` で確定。similarity の `get_subtree_size()` (毎回再帰 O(n)) を避け、APTED の二重ループを O(1) に
- **正規化方針 (再確認)**:
  - 呼び出し名 (CallExpression の callee の Identifier) と import 名 (`import { X } from "Y"` の `X` `Y`) は `value` に文字列保持
  - メソッドチェーン `axios.get` は MemberExpression として `axios` と `get` を両方保持
  - その他の識別子 (ローカル変数・引数等) は `value = Some(CompactString::const_new("_IDENT"))`
  - 文字列・数値・テンプレート文字列リテラルは `value = Some("_LIT")`
  - `true / false / null / undefined` は値そのまま保持
  - JSX 要素名 (`<Button>` の `Button`) は呼び出し名と同等に保持
- 言語アダプターはこの TreeNode に変換するだけ。core は中立 IR で APTED/TSED を計算する
- 文字列は `compact_str::CompactString` を採用 (24 byte 以内は inline、heap 確保なし。識別子・hook 名は大半がここに収まる)

### 3.3 関数定義

```rust
/// 関数の種別。抽出対象を仕様化し、見逃しを防ぐ。
pub enum FunctionKind {
    FunctionDeclaration,    // function foo() {}
    VariableArrow,          // const foo = () => {}  /  const foo = function() {}
    ObjectMethod,           // { foo() {} }  /  { foo: () => {} }
    ClassMethod,            // class C { foo() {} }
    ClassPropertyArrow,     // class C { foo = () => {} }
    DefaultExportFunction,  // export default function() {}  /  export default () => {}
}

pub struct FunctionRef {
    pub file: RelPath,                  // 設定ファイル基準の相対パス (Newtype)
    pub selector: FunctionSelector,     // golden 解決と等価なセレクタ
    pub kind: FunctionKind,
    pub start_line: u32,
    pub end_line: u32,
    pub byte_range: (u32, u32),         // why サブコマンドで該当行を切り出す用
    pub tree: TreeNode,
    pub signature: Signature,
    pub calls: Vec<CompactString>,      // ソート済み (集合差分 O(n+m))
    pub imports: Vec<CompactString>,    // ソート済み
    pub ast_hash: [u8; 32],             // blake3(canonical(tree))
}

pub struct FunctionSelector {
    pub symbol: SymbolPath,             // "useUser" / "UserService.create" / "default"
}

pub struct RelPath(PathBuf);            // Newtype: 設定ファイル基準の相対パス
pub struct SymbolPath(SmallVec<[CompactString; 2]>); // 最大 2 段ネスト想定
```

- **`FunctionKind` を 6 種別に細分**: `Function | Arrow | Method` の 3 区分では React/Node の実コードで取りこぼしが多い。仕様化することで「対象/非対象」がレビュー可能になる
- **抽出対象 (Phase 1b で extract.rs に実装)**: 上記 6 種別。各種別は `extract/{function_decl, arrow, method, class_method, class_property_arrow, default_export}.rs` に分割実装
- **抽出非対象 (明示的に除外、`why` でユーザーに見える化)**:
  - 関数式 (`const x = function() {}` の右辺の bare function expression) は VariableArrow と同等に扱う
  - 即時実行関数 (IIFE)、コールバック内のインライン関数は対象外
  - getter/setter は ClassMethod として扱う
- **Newtype** (`RelPath` `SymbolPath`) を導入。`String` で取り回す事故 (file と name の入れ違い) を型で防ぐ
- **`calls` `imports`**: `BTreeSet<String>` から `Vec<CompactString>` (構築時にソート) に変更。要素数 10〜30 では BTree より高速、CompactString で heap allocation 削減

### 3.4 逸脱

```rust
pub struct Deviation {
    pub rule_id: RuleId,                // どのルールでの逸脱か (rules[i].id)
    pub function: FunctionRef,
    pub matched_golden: FunctionRef,    // 最も類似度が高かった golden
    pub all_golden_scores: Vec<(GoldenId, f64)>, // 全 golden への類似度 (上位 N 表示用)
    pub similarity: SimilarityScore,    // 分解されたスコア
    pub differences: Differences,
}

pub struct SimilarityScore {
    pub overall: f64,                   // 0..1 の合算スコア (threshold 比較に使用)
    pub shape: f64,                     // TSED に基づく構文形状の類似度
    pub calls: f64,                     // calls 集合の Jaccard 類似度
    pub imports: f64,                   // imports 集合の Jaccard 類似度
    pub signature: f64,                 // 引数数/戻り値形の一致度 (0 or 1 の段階値)
}

pub struct Differences {
    pub missing_calls: Vec<CompactString>,
    pub extra_calls: Vec<CompactString>,
    pub missing_imports: Vec<CompactString>,
    pub extra_imports: Vec<CompactString>,
    pub signature_diff: Option<SignatureDiff>,
}
```

- **類似度を 4 軸に分解**: `overall` だけでなく shape/calls/imports/signature を持たせる。理由:
  1. `why` で「なぜ逸脱なのか」を説明できる (TSED は近いが calls 集合が違う、等)
  2. 将来 rule ごとに重みを変える (`weights: { shape: 0.5, calls: 0.4, imports: 0.1 }`) 拡張に開かれる
  3. レビュアー指摘「TSED 単体では『流儀統一』を測れない」への対応
- `overall` の計算は Phase 1b では `0.6 * shape + 0.3 * calls + 0.1 * imports` の固定重み。Phase 2 でフィクスチャ駆動でチューニング

### 3.5 ベースライン (`.layer-conform-baseline.json`)

```json
{
  "version": 1,
  "hashFormat": "blake3-canonical-v1",
  "deviations": [
    {
      "ruleId": "repositories",
      "file": "src/repositories/useProduct.ts",
      "symbol": "useProduct",
      "astHash": "a3f5...",
      "addedAt": "2026-04-26"
    }
  ]
}
```

- **突合キー (確定)**: `(ruleId, file, symbol, astHash)` の 4 タプル一致
  - `ruleId` 一致: 違うルールでの逸脱は別物として扱う (rule の意図は ID で識別)
  - `file + symbol` 一致: 別場所にコピーされた逸脱を抑制しない (見逃し防止)
  - `astHash` 一致: 関数の実装が変わったら baseline から外れる
  - rename/move 耐性は別機能 `--baseline-mode moved` で対応 (Phase 3.5)。デフォルトは `strict` (上記 4 タプル一致)
- **`astHash` の入力**: 正規化後の TreeNode を **canonical な手書きシリアライザ** で直列化し blake3 にかける
  - `serde_json` 経由の直列化はバージョン依存で baseline を壊しうるため使わない
  - canonical writer は `kind (u32 LE) | value 長 (u32 LE) | value bytes | children 数 (u32 LE) | 子を再帰` の単純フォーマット
  - `hashFormat` を JSON に明記。互換破壊時はバージョンを上げる
- ハッシュアルゴリズム: **blake3** (SHA-256 比で速い、Rust エコシステムでメジャー)

---

## 4. パイプライン (デフォルト `layer-conform` 実行時)

```
1. 設定ロード (.layer-conform.json)
2. baseline ロード (任意)
3. ファイル列挙 (applyTo の glob ∪ → walkdir + ignore で展開)
   - --changed / --since の場合は git diff --name-only と AND
4. 言語判定 → LanguageAnalyzer 選択 (拡張子)
5. 並列パース・関数抽出 (rayon par_iter)
   - 各ファイル → Vec<FunctionRef>
6. golden 関数を解決 (file:functionName で探す)
7. ルール × 関数 のマッチング:
   - applyTo にマッチ かつ ignore にマッチしない
   - golden 自身は除外
   - インラインコメント (layer-conform-ignore: <理由>) で除外
8. 各関数 vs 各 golden の類似度計算
   - fingerprint 距離で事前フィルタ (キャッシュ用 cheap)
   - APTED + TSED で本計算
   - 多 golden 時は max
9. threshold 未満を Deviation として収集
10. baseline でフィルタ (--no-baseline で無効化)
11. レポーター呼び出し (text / json / summary)
12. 終了コード決定 (--warn-only で常に 0)
```

---

## 5. CLI 仕様 (clap)

```
layer-conform [OPTIONS] [PATHS]...
layer-conform init
layer-conform check <FILE>
layer-conform why <FILE>
layer-conform baseline
```

### グローバルフラグ

- `--config <PATH>`: 設定ファイルパス (デフォルト: カレントディレクトリ直下の `.layer-conform.json` のみ。親ディレクトリの探索は行わない。存在しなければ `--auto` 指定がない限りエラー)
- `--auto`: 設定ファイルを使わず自動推定モード
- `--changed`: git で変更されたファイルのみ
- `--since <REF>`: 指定 ref からの変更ファイルのみ
- `--threshold <N>`: 全エントリの threshold を上書き
- `--json`: JSON 出力
- `--summary`: layer サマリ出力
- `--explain <FILE>`: 1 ファイルだけ詳細表示
- `--warn-only`: 逸脱があっても exit 0
- `--no-baseline`: baseline を無視
- `--no-color`: ANSI カラー無効化

### `--summary` のグループ化

設定エントリ単位で集計する。各 `rules[i]` ごとに「対象関数数 / conform 数 / 逸脱数 / 逸脱率」を 1 行表示する。layer 名は `rules[i].name`(将来追加) または applyTo パターンの先頭を使う。

### 終了コード

- 0: 逸脱なし、または `--warn-only`
- 1: 逸脱あり (1 件でも fail)
- 2: ツール側エラー (設定不正等)
- baseline でフィルタ後に 1 件でも逸脱が残れば exit 1。`--fail-on` のような閾値オプションは導入しない (シンプル維持)

---

## 6. インラインコメントによる除外

仕様書 6.1 の `layer-conform-ignore: <理由>` を実装する。

- **検出位置 (確定)**: 関数定義の**直前 1〜3 行**、または直前の **JSDoc ブロック (`/** ... */`) 内**
  - 直前 1〜3 行のラインコメント `// layer-conform-ignore: <reason>`
  - 直前のブロックコメント `/* layer-conform-ignore: <reason> */`
  - 直前の JSDoc コメント `/** ... @layer-conform-ignore: <reason> ... */` または素の `/** ... layer-conform-ignore: <reason> ... */`
  - **デコレーターと関数の間に挟まれている場合も許容** (デコレーター行をまたいで関数直前と見なす)
  - 直後・関数内のコメントは無視
- 理由なし (`// layer-conform-ignore`) は警告 (stderr) を出すが除外はする (運用しやすさ重視)
- 検出は oxc が返す trivia (コメントスパン) を利用

---

## 7. 多言語対応の拡張ポイント

```rust
pub trait LanguageAnalyzer: Send + Sync {
    fn extensions(&self) -> &'static [&'static str];

    /// ファイルをパースして関数を全て抽出する。
    /// path は表示用。FunctionRef.file は呼び出し側で設定する。
    /// source の所有権は内部で必要なら to_owned する (TreeNode は owned で返す)。
    fn parse_file(
        &self,
        ctx: &ParseContext,
        path: &Path,
        source: &str,
    ) -> Result<Vec<FunctionRef>, ParseError>;
}

pub struct ParseContext {
    pub jsx: bool,
    pub strict: bool,
    // 将来オプションを増やすときに trait シグネチャを壊さないため struct で受ける
}
```

- **oxc Allocator のライフタイム戦略 (確定)**:
  - Allocator はファイル単位で `parse_file` 内に閉じ込め、`TreeNode` 構築完了後に drop
  - oxc AST 由来の `&str` は normalize.rs 内で `CompactString::from()` で **必ず owned 化**
  - `TreeNode` には oxc 由来の参照を一切残さない (`'static` 相当)
  - 並列モデル: `parse_file` 自体は `&self` で `Send + Sync`、ファイル単位で rayon の `par_iter` を回す
- core は `Vec<Box<dyn LanguageAnalyzer>>` を持つレジストリ経由で動作
- 今回は `lc-ts` のみ実装
- 将来の `lc-py`, `lc-go` は別 crate として workspace に追加するだけ

---

## 8. テスト戦略

### 8.1 ユニットテスト

- `apted.rs`: 既知の木ペアで距離値を assert (similarity の test ケース流用)
- `tsed.rs`: 同一関数 → 1.0、完全別関数 → 0 に近い、を assert
- `fingerprint.rs`: 同一関数の fingerprint 一致、距離関数の不等式
- `extract.rs`: TS のサンプルから FunctionDeclaration/Arrow/Method を期待数だけ抽出
- `ignore.rs`: コメント解析の正常/異常系
- `config.rs`: JSON ロードの未知フィールド許容、文字列/配列の両対応
- `baseline.rs`: hash 一致時にスキップ、不一致で再検出

### 8.2 統合テスト (`tests/`)

- `assert_cmd` + `tempfile` で一時ディレクトリにフィクスチャを展開し CLI 実行
- ケース:
  - 設定 + 全関数 conform → exit 0、stdout 「No deviations」
  - 1 関数のみ逸脱 → exit 1、stdout に該当ファイル名
  - `--json` → 仕様書 7.3 の形を満たす JSON を出力 (snapshot)
  - `--summary` → layer 単位の集計を表示
  - `--changed` → git fixture と組み合わせ
  - `baseline` 作成 → 同入力で次回 exit 0
- スナップショット: `insta` を導入して text/json 出力を固定化

### 8.3 ベンチマーク (任意)

- `criterion` で 100 関数 × 5 layer のデータセット
- 並列 vs 直列、fingerprint 事前フィルタ on/off

---

## 9. 実装フェーズ分割

### Phase 1a — core 単体 (優先度: 必須 / 着手の最初)

1. workspace, 4 crates (lc-core / lc-io / lc-ts / cli), rust-toolchain.toml, workspace lints (`[workspace.lints]`), rustfmt.toml
2. `lc-core::tree` (NodeKind enum, TreeNode, id 採番、subtree_size 計算)
3. `lc-core::apted` (similarity から移植、`HashMap<(u32,u32), f64>` を memoize key に)
4. `lc-core::tsed` (sized-aware 正規化、ペナルティ係数は constant で開始)
5. `lc-core::deviation` (SimilarityScore 4 軸計算、Differences 構築)
6. **`cargo test -p lc-core` だけで完結する単体テストを通す** (CLI なし、ファイル I/O なし)。similarity の test fixture を借用

### Phase 1b — TS パース + 1 ペア比較 CLI (優先度: 必須 / MVP の境界)

7. `lc-ts::oxc_compat` (oxc 0.73 API を集約、bump 時はここだけ修正)
8. `lc-ts::parser` (Allocator をファイル単位で閉じ込め)
9. `lc-ts::extract`: FunctionDeclaration のみ実装。Arrow/Method/ClassMethod は Phase 2 へ
10. `lc-ts::normalize` (oxc AST → TreeNode、識別子の owned 化)
11. CLI: `layer-conform check <FILE> --golden <FILE:SYMBOL>` 形式で **1 ペア比較のみ** 動かす。設定ファイル不要
12. text reporter (4 軸スコアを表示)
13. 統合テスト 2 ケース (conform / deviation)

> **ここで TSED 重み付けのフィードバックループを早く回す**。フィクスチャを増やしながら shape/calls/imports/signature の重みを調整する。

### Phase 2 — 設定駆動と関数抽出網羅 (優先度: 高)

14. `lc-io::config` (`.layer-conform.json` ロード、`golden` の string/object/array 三形態を `try_from` で正規化)
15. `lc-core::matcher` (rule × file マッチング)
16. multi-golden 対応 (全 golden への類似度を保存、max を採用)
17. `lc-ts::extract`: VariableArrow / ObjectMethod / ClassMethod / ClassPropertyArrow / DefaultExportFunction を順次追加
18. `lc-core::ignore_parse` + `lc-ts` のコメント trivia 連携 (デコレーター/JSDoc 跨ぎ対応)
19. `--threshold`, `--no-color`, `--json` (snapshot test)
20. `--explain <file>` / `why <file>` (Phase 3 から前倒し): 4 軸スコア + missing/extra calls 表示
21. `init` サブコマンド (既存ファイルでエラー、`--force` で上書き)

> fingerprint 事前フィルタは MVP ではスキップ。関数 1000 件超で APTED が遅い場合のみ Phase 2.5 で後追い実装。

### Phase 3 — 運用機能 (優先度: 中)

22. `lc-io::baseline` + `baseline` サブコマンド (`(ruleId, file, symbol, astHash)` 4 タプル突合)
23. `lc-io::git` + `--changed`, `--since <ref>`
24. `--summary` レポーター (rule_id 単位で集計)
25. `--warn-only`, `--no-baseline`
26. 大量警告対策: `--limit <N>`, `--sort similarity|file|rule`, `--rule <ID>`, `--min-similarity <N>`

### Phase 4 — 自動推定 / 多言語の足場 / 配布 (優先度: 低)

27. `init --auto` (実行モードではなく config 生成支援に格下げ): layer 候補抽出 + クラスタリングで golden 候補を提案
28. `LanguageAnalyzer` trait の最終形 (`lc-py` を足せる形に)
29. ベンチマーク (`criterion`)
30. **配布**: GitHub Releases prebuilt binary (Linux x86_64 / macOS aarch64+x86_64)、Homebrew tap、`cargo install` から外部経路を確保

---

## 10. 残課題 (実装中・後続フェーズで詰める)

### Phase 進行と並行して解決

1. **`--auto` モードの layer 識別アルゴリズム**: ディレクトリ末端パスでのグループ化を起点に、特徴ベクトル (calls/imports 集合) のクラスタリング方式を Phase 4 で具体化
2. **fingerprint の方式**: MVP ではスキップ。プロファイリング後、必要なら similarity と同じ Bloom 128bit + node 種別カウントを Phase 2.5 で移植
3. **TSED と 4 軸スコアの重み**: Phase 1b で固定値スタート、Phase 2 でフィクスチャ駆動でチューニング

### Plan 反映済み (Critical/High 取り込み完了)

- TreeNode 再設計 (NodeKind enum + Box + id + subtree_size)
- `golden` の正式設計 (string/object/array の 3 形態 + symbol path 文法)
- `FunctionKind` の 6 種別細分化と抽出対象/非対象の明文化
- baseline 突合キー拡張 (`ruleId + file + symbol + astHash`) + blake3 + canonical writer
- oxc Allocator のライフタイム戦略 (ファイル単位で閉じ込め、TreeNode は owned)
- oxc `=0.73.0` exact pin + `oxc_compat.rs` Anti-Corruption Layer
- `lc-core` から I/O 切り出し (`lc-io` 新設)
- 類似度を 4 軸 (shape/calls/imports/signature) に分解、`why`/`--explain` を Phase 2 に前倒し
- `rules[].id` 必須化
- Phase 1 を 1a/1b に分割

### 実装中に判断 (Medium 以下、Plan 取り込み見送り)

- clap subcommand と top-level positional の重複解消 (実装時に整える)
- `--config` 探索戦略 (カレント直下のみ vs 親探索) — カレント直下のみで MVP、要望次第で拡張
- workspace lints の細部 (`pedantic` の allow リスト) — 実装中に必要分だけ追加
- BTreeSet → SmallVec/lasso の最適化 — Phase 2.5 ベンチで判断
- monorepo baseline 共有戦略 — 要望が出てから対応
- `--baseline-mode moved` (rename/move 検知) — 利用者要望次第で Phase 3.5
- LanguageAnalyzer trait の責務拡張 (find_ignore_directives 等) — 多言語対応着手時に判断
- golden 品質管理 (オーナー/レビュー済みメタデータ) — 要望次第

---

## 11. 参考実装ファイル (similarity-ts から移植)

| similarity 側 | layer-conform 側 | 注意点 |
|---|---|---|
| `crates/core/src/apted.rs` (88行) | `crates/core/src/apted.rs` | TreeNode の型が違うので memoize key を `(u32, u32)` に変更 |
| `crates/core/src/tsed.rs` (181行) | `crates/core/src/tsed.rs` | サイズペナルティ係数を Phase 2 でチューニング |
| `crates/core/src/tree.rs` | `crates/core/src/tree.rs` | NodeKind enum + Box + id + subtree_size に再設計 (3.2) |
| `crates/core/src/ast_fingerprint.rs` (629行) | `crates/core/src/fingerprint.rs` | Phase 2.5 でのみ実装 |
| `crates/core/src/function_extractor.rs` (835行) | `crates/lc-ts/src/extract/*.rs` | **6 種別に分割実装** (3.3)、型抽出は不要 |
| `crates/core/src/parser.rs` | `crates/lc-ts/src/parser.rs` | Allocator をファイル単位で閉じ込め、文字列 owned 化 |
| `crates/similarity-ts/src/main.rs` | `crates/cli/src/main.rs` | サブコマンド構造を再設計 |

similarity の TSED は「サイズが違うほど低スコア」のペナルティを掛けるが、layer-conform は **流儀の同一性** を測りたいので、過度なサイズペナルティは寧ろ false negative の原因。Phase 2 でチューニングする。
