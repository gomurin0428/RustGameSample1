# Rust + WebAssembly Mini Game

Rust と WebAssembly で動作する、トランク (`trunk`) ベースのシンプルなゲームサンプルです。矢印キーまたは WASD でプレイヤーのスクエアを操作し、オレンジ色のコインを取りに行きます。

## 必要ツール

- Rust 1.76 以降を推奨
- `wasm32-unknown-unknown` ターゲット
- `trunk` と `wasm-bindgen-cli`

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk wasm-bindgen-cli
```

## 実行方法

1. このフォルダに移動します。
   ```bash
   cd wasm-game
   ```
2. 開発サーバーを起動します。
   ```bash
   trunk serve --open
   ```
   ブラウザーが自動的に開き、`http://127.0.0.1:8080` でゲームが動作します。

## ビルド方法

本番ビルドは次で生成できます。

```bash
trunk build --release
```

`dist/` ディレクトリに最適化された JS/WASM ファイルが出力されます。

## プロジェクト構成

- `src/lib.rs`: ゲームロジックとブラウザー連携コード
- `index.html`: Trunk が処理するシンプルなホストページ
- `Cargo.toml`: `wasm-bindgen` と `web-sys` を利用する WebAssembly 向け設定

## カスタマイズのヒント

- `Game::update` でスピードや当たり判定を変更できます。
- `Game::draw` の描画コードを編集することで、色や表現を自由にカスタマイズできます。
- 入力デバイスを追加したい場合は、`register_input_listeners` にイベントハンドラーを足してください。
