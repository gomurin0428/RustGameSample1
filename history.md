# history

- 2025-09-25 16:27:21 wasm-game のビルドエラーを解消。`resolve_canvas` と `canvas_2d_context` の戻り値を修正し、アニメーションループでの `Rc` の借用エラーを解消、`set_fill_style_str` を利用するよう更新。`cargo build` と `cargo test` を実行して成功を確認。
