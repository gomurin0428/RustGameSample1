# history

- 2025-09-25 16:27:21 wasm-game のビルドエラーを解消。`resolve_canvas` と `canvas_2d_context` の戻り値を修正し、アニメーションループでの `Rc` の借用エラーを解消、`set_fill_style_str` を利用するよう更新。`cargo build` と `cargo test` を実行して成功を確認。
- 2025-09-26 08:45:29 jinja-game を新規実装。Rust + WebAssembly で神社ゴールの 2D アクションを構築し、`Cargo.toml`・`src/lib.rs`・`index.html`・`README.md` を整備。`cargo build --target wasm32-unknown-unknown` でビルド成功を確認。
- 2025-09-26 08:53:59 jinja-game を trunk 対応に変更。`index.html` に `data-trunk` リンクを追加し、`README.md` を trunk serve 手順へ更新。`trunk build` でビルド成功を確認し、TROUBLESHOOTING.md を trunk 前提の内容に整理。
- 2025-09-26 09:29:12 orstudy-game1 を新規構築。待ち行列理論学習用のシミュレーションを Rust + WebAssembly で実装し、到着率・サービス率・窓口数などの UI コントロールとリアルタイムメトリクス表示を追加。`cargo test` と `cargo build --target wasm32-unknown-unknown` をフルパスの `cargo.exe` で実行して成功を確認し、TROUBLESHOOTING.md に非 wasm テスト時の注意点を追記。
