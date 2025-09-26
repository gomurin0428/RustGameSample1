# TROUBLESHOOTING

- `cargo` コマンドが認識されない場合は、`%USERPROFILE%\\.cargo\\bin\\cargo.exe` を直接呼び出してください。PowerShell で別プロセスとして `_codex_env.ps1` を実行しただけでは PATH が更新されません。
- `CanvasRenderingContext2d::set_fill_style` を `&JsValue` で呼ぶと警告が出るので、`set_fill_style_str` を使う必要があります。
- アニメーションループで `Rc<RefCell<...>>` をクロージャに渡す際は、クロージャ内で再参照する用にクローンを別途確保してください。同じ `Rc` をそのまま move すると借用チェッカーに弾かれます。
- `_codex_env.ps1` はリポジトリに存在しないため、環境設定が必要な場合は手動でパスを設定するか `C:\Users\<user>\.cargo\bin\cargo.exe` のようにフルパスで `cargo` を呼び出してください。
- `trunk serve` / `trunk build` を使う場合は `trunk` と `wasm-bindgen-cli` がインストールされている必要があります。未導入なら `cargo install trunk wasm-bindgen-cli` を実行してください。
- `CanvasRenderingContext2d::save` / `restore` / `stroke` は `()` を返すため、`expect` でラップするとコンパイルエラーになります。戻り値が無い API はそのまま呼び出し、異常系が必要なら `JsValue` を返す別 API を検討してください。
- wasm 以外のターゲットで `cargo test` を動かす場合、`js_sys::Math::random` が使えないため `random_unit` などの乱数ヘルパーは `#[cfg(target_arch = "wasm32")]` と非 wasm 版の両方を定義してください。判定ロジックを追加する際も同じ分岐を忘れるとテストがクラッシュします。テストで wasm 専用のバリデーションを確認したいときは `cargo test --target wasm32-unknown-unknown` を使うと安全です。
