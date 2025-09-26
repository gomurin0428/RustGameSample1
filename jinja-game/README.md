# Jinja Quest

Rust と WebAssembly で実装した横スクロール風 2D アクションです。マリオ風の操作感で鳥居のある神社に辿り着くとクリアになります。

## 必要ツール

- Rust 1.76 以降を推奨
- `wasm32-unknown-unknown` ターゲット
- `trunk` と `wasm-bindgen-cli`

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk wasm-bindgen-cli
```

## 起動手順

1. このフォルダに移動します。
   ```bash
   cd jinja-game
   ```
2. 開発サーバーを起動します。
   ```bash
   trunk serve --open
   ```
   ブラウザーが自動で開き、`http://127.0.0.1:8080` でゲームを遊べます。

## ビルド

本番向けビルドは以下で生成できます。

```bash
trunk build --release
```

`dist/` ディレクトリに最適化された JS/WASM が出力されます。

## 操作方法

- 左右移動: 矢印キーまたは A / D
- ジャンプ: スペース / W / ↑ / Z
- クリア条件: 右端の神社に到達

## 実装メモ

- レベルは `src/lib.rs` の `LEVEL_MAP` で定義しています。`#` がブロック、`P` が初期位置、`S` が神社です。
- タイルベースの衝突判定で物理挙動を実装しており、想定外の地形があるとパニックを発生させて異常を検出します。
- 描画は `CanvasRenderingContext2d` への直接描画で、背景スクロールとカメラ追従は `Game::update_camera` が担っています。
