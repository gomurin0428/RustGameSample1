# TROUBLESHOOTING

- `cargo` コマンドが認識されない場合は、`%USERPROFILE%\.cargo\bin\cargo.exe` を直接呼び出してください。PowerShell で別プロセスとして `_codex_env.ps1` を実行しただけでは PATH が更新されません。
- `CanvasRenderingContext2d::set_fill_style` を `&JsValue` で呼ぶと警告が出るので、`set_fill_style_str` を使う必要があります。
- アニメーションループで `Rc<RefCell<...>>` をクロージャに渡す際は、クロージャ内で再参照する用にクローンを別途確保してください。同じ `Rc` をそのまま move すると借用チェッカーに弾かれます。
- `_codex_env.ps1` はリポジトリに存在しないため、環境設定が必要な場合は手動でパスを設定するか `C:\Users\<user>\.cargo\bin\cargo.exe` のようにフルパスで `cargo` を呼び出してください。
- `trunk serve` / `trunk build` を使う場合は `trunk` と `wasm-bindgen-cli` がインストールされている必要があります。未導入なら `cargo install trunk wasm-bindgen-cli` を実行してください。
- `CanvasRenderingContext2d::save` / `restore` / `stroke` は `()` を返すため、`expect` でラップするとコンパイルエラーになります。戻り値が無い API はそのまま呼び出し、異常系が必要なら `JsValue` を返す別 API を検討してください。
- wasm 以外のターゲットで `cargo test` を動かす場合、`js_sys::Math::random` が使えないため `random_unit` などの乱数ヘルパーは `#[cfg(target_arch = "wasm32")]` と非 wasm 版の両方を定義してください。判定ロジックを追加する際も同じ分岐を忘れるとテストがクラッシュします。テストで wasm 専用のバリデーションを確認したいときは `cargo test --target wasm32-unknown-unknown` を使うと安全です。
- `realgeopolitics` の CLI 版を起動する際は `config/countries.json` が必須です。ファイルが存在しない・JSON が壊れている場合は即座にエラー終了します。国を追加する時も JSON の配列構造と各フィールド名を崩さないよう注意してください。
- ブラウザ版 (`realgeopolitics-web`) をビルドするには Rust の `wasm32-unknown-unknown` ターゲットと `trunk` コマンドが必要です。`cargo build -p realgeopolitics-web --target wasm32-unknown-unknown` で wasm のテストビルドができます。`trunk serve` を使う際は `realgeopolitics/web` ディレクトリで実行してください。
- `realgeopolitics-core` の `game` モジュールは複数ファイルに分割されています。`CountryState` を初期化する際は `CountryState::new(...)` を利用し、構造体リテラルで private フィールドを書き換えないでください（モジュール外からはアクセスできません）。
- CLI 版で `set` コマンドを使う場合は、インフラ/軍事/福祉/外交/債務返済/行政維持/研究開発の順で GDP 比率 (％) を 7 つ入力し、必要に応じて末尾に `core` または `nocore` を付けてください。合計値に上限制約はありませんが、旧仕様（割合正規化）向けのスクリプトを使い続けると引数不足で失敗するため移行時は確認してください。Web 版も NumericUpDown で同じ割合を扱い、自動正規化は行われません。
- Web 版の時間倍率セレクタは内部的に小数点第2位で丸めた値を表示します。CLI 側で 1.333 など細かい倍率を設定した直後は「カスタム」項目が追加されて 1.33 として選択されるので、厳密な値を維持したい場合は CLI から再調整してください。
- FiscalAccount の収支は tick ごとにクリアされます。テストや CLI で直前 tick の収支を確認したい場合は `tick` 実行直後に `total_revenue()` / `total_expense()` を参照してください。複数 tick の履歴が必要なら別途蓄積してください。
- `TaxPolicy` の税率を変更した場合、反映は次回の `tick` 実行時です。繰越税収 (`pending_revenue`) は自動で次 tick の即時収入に加算されるため、短時間で何度も切り替えると直前の繰越が期待より多く見える場合があります。
- CommodityMarket の価格更新は `tick` ごとに行われ、ショック発生時は価格が大きく変動します。テストで安定した結果が必要な場合は `GameState::from_definitions_with_seed` を利用し、`StdRng` のシードを固定してください。
