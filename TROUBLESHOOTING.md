# TROUBLESHOOTING

- `cargo` コマンドが認識されない場合は、`%USERPROFILE%\\.cargo\\bin\\cargo.exe` を直接呼び出してください。PowerShell で別プロセスとして `_codex_env.ps1` を実行しただけでは PATH が更新されません。
- `CanvasRenderingContext2d::set_fill_style` を `&JsValue` で呼ぶと警告が出るので、`set_fill_style_str` を使う必要があります。
- アニメーションループで `Rc<RefCell<...>>` をクロージャに渡す際は、クロージャ内で再参照する用にクローンを別途確保してください。同じ `Rc` をそのまま move すると借用チェッカーに弾かれます。
