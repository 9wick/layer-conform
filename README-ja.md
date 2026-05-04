# layer-conform

[![crates.io](https://img.shields.io/crates/v/layer-conform.svg)](https://crates.io/crates/layer-conform)
[![docs.rs](https://img.shields.io/docsrs/layer-conform)](https://docs.rs/layer-conform)
[![license: MIT](https://img.shields.io/crates/l/layer-conform.svg)](./LICENSE)

[English README](./README.md)

コードベースのアーキテクチャレイヤー内における「スタイルの逸脱」を検出するツール。同じレイヤーの他の関数と見た目が異なる関数を見つけ出します。

## 対応言語

- TypeScript / JavaScript
- Rust

## インストール

```sh
cargo install --locked layer-conform
```

## クイックスタート

スターター設定を生成:

```sh
layer-conform init
```

`.layer-conform.json` を編集し、golden（手本となる関数）と、それに揃えたいファイル群の glob を指定します。

TypeScript の場合:

```json
{
  "version": 1,
  "rules": [
    {
      "id": "repositories",
      "golden": "src/repositories/useUser.ts:useUser",
      "applyTo": "src/repositories/**/*.ts",
      "threshold": 0.7
    }
  ]
}
```

Rust の場合:

```json
{
  "version": 1,
  "rules": [
    {
      "id": "handlers",
      "golden": "src/handlers/get_user.rs:get_user",
      "applyTo": "src/handlers/**/*.rs",
      "threshold": 0.7
    }
  ]
}
```

実行:

```sh
layer-conform        # 逸脱が見つかると exit 1
```

インラインディレクティブで特定の関数をスキップ（TypeScript のみ対応）:

```ts
// layer-conform-ignore: 旧アダプター。Q3 で削除予定
function useLegacy() { ... }
```

## サブコマンド

| コマンド | 動作 |
|---|---|
| `layer-conform`（引数なし）/ `layer-conform check` | すべてのルールを、マッチするすべてのファイルに対して実行 |
| `layer-conform check --explain <FILE>` | 同上だが、指定したファイル 1 つの詳細だけを表示 |
| `layer-conform why <FILE>` | `<FILE>` に関係する rule と golden、およびそのスコアを一覧表示 |
| `layer-conform init [--force]` | スターターの `.layer-conform.json` を書き出し |

## グローバルフラグ

| フラグ | 効果 |
|---|---|
| `--threshold <N>` | すべてのルールの threshold を上書き（例: `0.5`） |
| `--no-color` | ANSI カラーを無効化 |
| `--json` | 機械可読な JSON で出力 |

## 設定

各ルールは以下のショートハンドをサポートします:

- `golden`: `"file:symbol"` | `{ "file": "...", "symbol": "..." }` | いずれかの配列
- `applyTo` / `ignore`: `string` | `string[]`
- `threshold`: 任意の `number`（デフォルト `0.7`）
- `disabled`: 任意の `boolean`

複数の golden を指定した場合、関数ごとに最も高いスコアの golden が採用されます:

```json
{ "id": "data", "golden": ["a.ts:hookA", "b.ts:hookB"], "applyTo": "src/**/*.ts" }
```

## ライセンス

MIT
