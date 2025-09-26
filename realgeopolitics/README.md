# Real Geopolitics Simulator

`realgeopolitics` はコアロジックを共有しつつ、CLI 版 (`realgeopolitics-cli`) と Web GUI 版 (`realgeopolitics-web`) を持つワークスペース構成です。各国の主要指標を管理しながら、インフラ投資・軍事演習・社会福祉・外交ミッションといった政策をターン制で実行できます。

## ワークスペース構成

| クレート | 内容 |
| --- | --- |
| `core` | ゲームロジックとデータモデル (`GameState`, `Action` など) |
| `cli` | ターミナルから操作する CLI インターフェース |
| `web` | Yew + Trunk を用いたブラウザ向けフロントエンド |

## CLI 版の実行

1. ルート (`realgeopolitics`) で `config/countries.json` が存在することを確認します。国データを追加したい場合は同ファイルにエントリを追記してください。
2. PowerShell 等で以下を実行します。
   ```powershell
   & "C:\Users\gomur\.cargo\bin\cargo.exe" run -p realgeopolitics-cli
   ```
3. プロンプトに `overview`, `inspect 1`, `plan 1 infrastructure`, `end` などのコマンドを入力して操作します。

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
3. ブラウザの GUI から各国の指標を確認しながら行動を設定し、`ターンを進める` ボタンで結果を確認できます。

## テスト

コアロジックはユニットテストで検証しています。

```powershell
& "C:\Users\gomur\.cargo\bin\cargo.exe" test --workspace --exclude realgeopolitics-web
```

Web 版の wasm ビルドを検証したい場合は:

```powershell
& "C:\Users\gomur\.cargo\bin\cargo.exe" build -p realgeopolitics-web --target wasm32-unknown-unknown
```
