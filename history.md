# history

- 2025-09-25 16:27:21 wasm-game のビルドエラーを解消。`resolve_canvas` と `canvas_2d_context` の戻り値を修正し、アニメーションループでの `Rc` の借用エラーを解消、`set_fill_style_str` を利用するよう更新。`cargo build` と `cargo test` を実行して成功を確認。
- 2025-09-26 08:45:29 jinja-game を新規実装。Rust + WebAssembly で神社ゴールの 2D アクションを構築し、`Cargo.toml`・`src/lib.rs`・`index.html`・`README.md` を整備。`cargo build --target wasm32-unknown-unknown` でビルド成功を確認。
- 2025-09-26 08:53:59 jinja-game を trunk 対応に変更。`index.html` に `data-trunk` リンクを追加し、`README.md` を trunk serve 手順へ更新。`trunk build` でビルド成功を確認し、TROUBLESHOOTING.md を trunk 前提の内容に整理。
- 2025-09-26 09:29:12 orstudy-game1 を新規構築。待ち行列理論学習用のシミュレーションを Rust + WebAssembly で実装し、到着率・サービス率・窓口数などの UI コントロールとリアルタイムメトリクス表示を追加。`cargo test` と `cargo build --target wasm32-unknown-unknown` をフルパスの `cargo.exe` で実行して成功を確認し、TROUBLESHOOTING.md に非 wasm テスト時の注意点を追記。
- 2025-09-26 09:53:23 realgeopolitics を新規構築。`config/countries.json` を読み込む Rust CLI 版ジオポリティクスシミュレーターを実装し、インフラ投資・軍事演習・外交ミッションなどの行動ループとランダムイベントを追加。`C:\Users\gomur\.cargo\bin\cargo.exe fmt` / `test` / `build` を実行して成功を確認。
- 2025-09-26 10:12:49 realgeopolitics をワークスペース化し、`realgeopolitics-core` に共通ロジックを切り出して CLI (`realgeopolitics-cli`) を移行。さらに `realgeopolitics-web` と Yew/Trunk ベースの GUI を追加し、`cargo test --workspace --exclude realgeopolitics-web` と `cargo build -p realgeopolitics-web --target wasm32-unknown-unknown` の成功を確認。
- 2025-09-26 10:35:54 realgeopolitics をリアルタイム制に刷新。`realgeopolitics-core` に予算配分ベースの tick ループを追加し、CLI の `set`/`tick` コマンドと Web GUI のスライダー＋自動更新ループで即時反映するよう変更。`cargo test --workspace --exclude realgeopolitics-web` と `cargo build -p realgeopolitics-web --target wasm32-unknown-unknown` を再実行し成功を確認。
- 2025-09-26 10:43:22 GeoPolitical Simulator 化に向けた改修計画 `realgeopolitics/GEO_SIM_ROADMAP.md` を作成し、基盤強化・外交軍事・経済内政・UX までのフェーズ別ロードマップを整理。
- 2025-09-26 10:50:41 フェーズ1対応として `GameClock`・`CalendarDate`・`Scheduler` を実装し、`GameState` を多層カレンダーとタスクスケジューラ対応に改修。テスト (`cargo test --workspace --exclude realgeopolitics-web`) と wasm ビルド (`cargo build -p realgeopolitics-web --target wasm32-unknown-unknown`) を再度実行して成功を確認。
- 2025-09-26 11:04:35 スケジューラ実装に対応するユニットテストを追加し、`ScheduledTask::execute` の各タスク種別と `Scheduler` の動作を検証。再度 `cargo test --workspace --exclude realgeopolitics-web` と `cargo build -p realgeopolitics-web --target wasm32-unknown-unknown` を実行して成功を確認。
- 2025-09-26 11:13:59 スケジューラの優先度管理を拡張し、長期タスクをバケット圧縮する `Scheduler` ロジックと対応テストを追加。`cargo test --workspace --exclude realgeopolitics-web` と `cargo build -p realgeopolitics-web --target wasm32-unknown-unknown` を再実行して成功を確認。
- 2025-09-26 11:20:21 スケジューラに `ScheduleSpec` を導入し、繰り返しタスクの再登録とバケット昇格をユニットテスト付きで実装。`cargo test --workspace --exclude realgeopolitics-web` および `cargo build -p realgeopolitics-web --target wasm32-unknown-unknown` を再実行し成功を確認。
