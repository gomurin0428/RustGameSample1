# Real Geopolitics Simulator

`realgeopolitics` はコアロジックを共有しつつ、CLI 版 (`realgeopolitics-cli`) と Web GUI 版 (`realgeopolitics-web`) を持つワークスペース構成です。各国の予算配分をリアルタイムに調整し、インフラ・軍事・福祉・外交への投資比率が即座に指標へ反映されます。

## ワークスペース構成

| クレート | 内容 |
| --- | --- |
| `core` | ゲームロジックとデータモデル (`GameState`, `BudgetAllocation` など) |
| `cli` | ターミナルから操作する CLI インターフェース |
| `web` | Yew + Trunk を用いたブラウザ向けフロントエンド |

## CLI 版の実行

1. ルート (`realgeopolitics`) で `config/countries.json` が存在することを確認します。国データを追加したい場合は同ファイルにエントリを追記してください。
2. PowerShell 等で以下を実行します。
   ```powershell
   & "C:\Users\gomur\.cargo\bin\cargo.exe" run -p realgeopolitics-cli
   ```
3. プロンプトに `overview`, `inspect 1`, `set 1 40 30 20 10`, `tick 30` などのコマンドを入力して操作します。`set` は百分率で配分を更新し、`tick` は指定分だけシミュレーションを進めます。

## Web 版の起動

1. Rust の `wasm32-unknown-unknown` ターゲットと `trunk` が導入されている必要があります。
2. `realgeopolitics/web` ディレクトリで以下を実行します。
   ```powershell
   trunk serve --open
   ```
   もしくはビルドのみの場合は:
   ```powershell
   trunk build --release
   ```
3. ブラウザの GUI から各国のスライダーを操作して配分を変更すると、即座にメトリクスが更新されます。画面下部のイベントログで最新の出来事を確認できます。

## テスト

コアロジックはユニットテストで検証しています。

```powershell
& "C:\Users\gomur\.cargo\bin\cargo.exe" test --workspace --exclude realgeopolitics-web
```

Web 版の wasm ビルドを検証したい場合は:

```powershell
& "C:\Users\gomur\.cargo\bin\cargo.exe" build -p realgeopolitics-web --target wasm32-unknown-unknown
```
## カバレッジレポート

1. ルートで `coverage.ps1` を実行します。
   ```powershell
   .\coverage.ps1
   ```
2. `coverage/html/index.html` をブラウザで開くと HTML レポートを、`coverage/lcov.info` で LCOV 形式のレポートを確認できます。CI 連携では `coverage/lcov.info` をアップロードしてください。

補足: `cargo coverage` で HTML レポートのみを再生成し、`cargo llvm-cov report --lcov --output-path coverage/lcov.info` で LCOV を単独更新することも可能です。



