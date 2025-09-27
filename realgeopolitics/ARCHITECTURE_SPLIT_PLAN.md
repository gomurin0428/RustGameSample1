# realgeopolitics 責務分割提案

本メモは 2025-09-26 時点の `realgeopolitics` 配下ソースから、責務が肥大化しているコンポーネントを洗い出し、疎結合化とテスト容易性向上に向けた分割方針をまとめたものです。ここで列挙した対象は CLI / Core 双方の主要導線に位置し、機能追加時の変更衝突や副作用の多さが懸念されています。

| モジュール | ファイル | 行数 (概算) | 主な責務 | 想定されるリスク |
| --- | --- | --- | --- | --- |
| `GameState` | `core/src/game/state.rs` | ~880 行 | 初期化、時間管理、全サブシステム呼び出し、レポート生成 | テスト困難、変更の波及、状態管理の重複 |
| 産業シミュレーション群 | `core/src/game/economy/industry.rs` | ~720 行 | データモデル、カタログ読み込み、シミュレーション、メトリクス蓄積 | 設定追加時の破壊的影響、再利用性の低さ |
| スクリプトイベント基盤 | `core/src/game/event_templates.rs` | ~690 行 | テンプレート読み込み・コンパイル・評価・実行 | パーサ拡張時の影響範囲が不明瞭 |
| CLI コマンドループ | `cli/src/cli.rs` | ~300 行 | コマンド解析、実行、入出力フォーマット | 機能追加時のリグレッション、UI 混在 |
| 財政/税制ユーティリティ | `core/src/game/economy.rs`, `core/src/game/systems/fiscal.rs` | ~650 行 合計 | 財政モデル、税制、配分適用ロジック | ドメイン知識の分散、ユニットテストの粒度不足 |
## GameState (`core/src/game/state.rs`)

**現状の責務**
- 国定義のバリデーションと初期化、`Scheduler` へのタスク登録、イベントテンプレートのロード、産業カタログの構築まで一括で担当。
- 経過時間、カレンダー、時間倍率、次イベント計算などの時間制御を全て内包。
- tick 実行時に財政・外交・政策・イベント・産業の各サブシステムを直接呼び出し、レポート文言まで生成。
- CLI / Web からの API (`tick_minutes`, `apply_industry_subsidy`, `fiscal_snapshot_of` など) を単一構造体で公開。

**課題**
- `tick_minutes` と `process_*` 群が状態準備と結果反映を混在させており、部分的なサブシステムテストが困難。
- 初期化ロジックが肥大化し、将来のロード時差し替え (外部設定読み込みやセーブデータ復元) を阻害。
- `fiscal_prepared` などのフラグ管理がメソッド間で共有されており、呼び出し順序を変えると容易に破綻する。

**分割方針案**
1. `game::bootstrap` (仮称) を新設し、国初期化・タスク登録・イベントテンプレートロードをまとめたビルダー (`GameBuilder`) に移譲する。`GameState` は完成済み依存のみ受け取る。
2. `game::time` サブモジュールに `SimulationClock` (time_multiplier, calendar, scheduler の仲介) を切り出し、tick 中の時間更新はここに委譲する。
3. 財政・外交・政策処理の呼び出しを統括する `SystemsFacade` (軽量ストラテジ) を設置し、`GameState` からは「シナリオ進行の指示」と「レポート収集」だけを扱う。
4. 産業関連 (`industry_runtime` 運用と収益配分) を `IndustryEngine` に移し、国への収益配分ロジックを専用メソッドへ隔離する。



_2025-09-27 更新_: 項目 3 を実装し、SectorMetricsStore にメトリクスを集約、Reporter にレポート生成を委譲済み。IndustryRuntime::simulate_tick はストア経由で最新値を保持し、IndustryEngine::overview は HashMap の直接参照を避ける構成になった。

_2025-09-27 更新_: 項目 4 を実装し、SectorRegistry でセクター解決を一元化。IndustryEngine と GameState は registry 経由で補助金適用を行い、CLI/Web API も同経路を使用する。

**移行ステップ案**
- 先に `GameState::from_definitions_*` の戻り値を `GameBuilder` に置き換える形でリファクタリングを開始し、挙動比較テストを追加。
- `tick_minutes` の内部ロジックを、時間更新 → 財政準備 → システム呼び出し → レポート合成 の 4 ブロックに整理してから、それぞれを専用型に抽出。
- CLI/Web の利用側は `GameState` の公開 API を維持しつつ内部委譲に切り替えるため、機能単位の統合テストを先に用意しておく。
## 産業シミュレーション (`core/src/game/economy/industry.rs`)

**現状の責務**
- 産業カテゴリ列挙からセクター定義 (`SectorDefinition`)、依存関係 (`SectorDependency`) までのデータモデルを 1 ファイルに内包。
- 組み込み YAML/JSON のロードとファイルシステムからの読込の双方を `IndustryCatalog` が保持。
- `IndustryRuntime` がシミュレーション・メトリクス蓄積・補助金適用・エネルギーコスト算出などを一括で担当。
- テスト用の補助メソッド (`set_modifier_for_test`) や CLI 向けトークン解決まで入り混じっている。

**課題**
- カタログ拡張や設定ファイル追加時にシミュレーションロジックへも影響し、最小変更で済ませづらい。
- `simulate_tick` にはカテゴリ横断の副作用 (エネルギーコスト指数の算出、レポート生成) が含まれ、ユニットテストで部分検証できない。
- `SectorId` をキーにした `HashMap` 群が散在し、ランダムイベント等から参照する際に API が冗長。

**分割方針案**
1. `core/src/game/economy/industry/` 配下に `mod.rs` を作り、`model.rs` (定義系) と `catalog.rs` (読み込み) を分離する。
2. `runtime.rs` へシミュレーション本体を移し、補助金・修正値の適用は `effects.rs` へ独立させる。`IndustryRuntime` はこれらを合成するオーケストレータへ縮小。
3. 産業別メトリクス (output/revenue/cost) を `SectorMetricsStore` (新規) に集約し、レポート生成は `Reporter` で責務分割する。
4. CLI/API から利用するセクター解決は `SectorRegistry` にまとめ、`GameState` からは registry 経由で操作する。

**移行ステップ案**
- 先に `IndustryCatalog` の API を `IndustryCatalog::iter_definitions` など中間 API へ差し替え、`IndustryRuntime` が内部データ構造を知らなくても動く作りにする。
- `simulate_tick` を「入力集計」「依存関係反映」「結果反映」の 3 フェーズに分解し、フェーズごとにユニットテストを追加したうえでファイル分割。
- 補助金やテスト用メソッドは `cfg(test)` 付きトレイトを導入して露出を絞り、プロダクションコードと分離する。
## スクリプトイベント基盤 (`core/src/game/event_templates.rs`)

**現状の責務**
- イベント定義ソースの読み込み (組み込み YAML/JSON)・逆シリアライズ・エラーハンドリングを単一ファイルで実装。
- 条件式パーサ (`ConditionExpr`) とトークナイザ、比較演算子の評価を同じファイルに保持。
- `ScriptedEventState` がクールダウン管理とエフェクト適用を兼務し、国ごとの履歴を直接操作。

**課題**
- 条件式の構文拡張や UI からのテスト生成を導入しようとすると、ファイル全体でビルド時間とレビューコストが増大。
- データ構造と実行時ロジックの境界が曖昧で、シナリオごとのテストダブルを作りづらい。
- イベント ID 解決や説明文アクセスなど UI 用の API が `GameState` から `ScriptedEventState` への直接参照になっており、非公開データを露出させがち。

**分割方針案**
1. （完了 2025-09-27）`event_templates/loader.rs` を新設し、IO とデシリアライズを担当させる。`CompiledEventTemplate` 生成は `compiler.rs` に分離。
2. （完了 2025-09-27）条件式パーサは `condition/` ディレクトリに切り出し、小さなトレイト (`ConditionEvaluator`) を導入してユニットテストを個別化。
3. 実行時状態 (`ScriptedEventInstance`) とテンプレート (`CompiledEventTemplate`) を分け、`GameState` からはインターフェース (`ScriptedEventEngine`) のみ参照する。
4. レポート文言生成をフォーマッタに委譲し、イベント効果適用自体はピュアなロジックとしてテスト可能にする。

**移行ステップ案**
- 現行の `load_event_templates` を `ScriptedEventEngine::from_builtin()` に置き換える薄いアダプタを先に用意し、既存呼び出しを移行。
- 条件式評価をモジュール化する際に、既存の `GameState::process_scripted_event` のユニットテストをベースラインとして確保する。
- CLI/Web が利用している「説明テキスト参照 API」は新しい `Engine` に委譲し、旧構造体を段階的に非公開化する。
## CLI コマンドループ (`realgeopolitics/cli/src/cli.rs`)

**現状の責務**
- 入力ループ、コマンド文字列の分解、エラー表示、整形出力までを一つのファイルにまとめている。
- 各コマンドの引数検証 (`set`, `industry`, `speed` など) が個別にベタ書きされ、`GameState` API へのアクセスと UI 表示が密結合。
- 書式変換ロジックが共有されておらず、表示変更時に複数箇所の修正が必要。

**課題**
- 新コマンド追加時に `dispatch_command` が肥大化し、テスト観点の抜け漏れが発生しがち。
- CLI 専用ユニットテストを作る際に、現在は `run` を丸ごと動かすしかなく、入出力をモックしづらい。

**分割方針案**
1. `commands/mod.rs` を新設し、各コマンドを `trait Command { fn name() -> &'static str; fn execute(&mut Context, Args) -> Result<()>; }` 形式で実装する。
2. 入出力は `CliIo` (読取/書込の抽象) に包み、`run` は IO とコマンドレジストリを束ねるシンプルなループへ縮小。
3. フォーマット系 (国概要、財政詳細) は `formatters.rs` に隔離し、Web 側とロジックを共有できるよう構造体ベースにする。
4. 引数パースは `structopt` 等への移行も視野に入れつつ、短期的にはユーティリティ関数群を `parser.rs` へ抽出する。

**移行ステップ案**
- まず `overview` コマンドのみ新フレームワークに移し、同じ出力になることを Golden テストで保証。
- 次いで `set`・`industry` のような副作用ありコマンドを移行し、引数検証ユニットテストを追加する。
- すべてのコマンド移行後に旧 `dispatch_command` を削除し、`run` の while ループを `CommandRegistry::dispatch` に切り替える。
## 財政・税制ロジックの整理 (`economy.rs` / `systems/fiscal.rs`)

**現状の責務**
- `economy.rs` に信用格付け、財政口座、税制、トレンド履歴が同居しており、ドメインごとの境界が曖昧。
- `systems/fiscal.rs` の `apply_budget_effects` が各配分カテゴリと外交連携、資源収益まで扱うモノリシックな処理。

**分割方針案**
- `economy/` 配下に `fiscal_account.rs`、`tax_policy.rs`、`credit_rating.rs` を作成し、型ごとの役割を明確化する。
- `apply_budget_effects` をカテゴリ単位の関数 (例: `apply_infrastructure_spending`) へ分割し、`GameState` 経由でも個別呼び出しが可能な形にする。
- ドメインごとのテストモジュールを作り、支出カテゴリ追加時に既存テストが指標として機能するよう整備する。

**推奨テスト戦略**
- `GameState` 分割に合わせて、`tick_minutes` の統合テストを「初期キャッシュ」「時間更新」「レポート件数」など観測値ベースで押さえる。
- 産業・イベント・財政それぞれにモックデータを用いたユニットテストを追加し、ファイル分割後の挙動差分を検出しやすくする。
## 次のアクション候補
- `GameState` ビルダー導入を最優先で検討し、`from_definitions_*` の依存整理から着手する。
- 産業シミュレーションのフェーズ分割に合わせて、`IndustryTickOutcome` をデータクラス化し、シリアライズしやすい構造へ整える。
- イベントテンプレートの条件式パーサを独立ライブラリ化する前段として、新モジュール構成を試作し、既存テンプレートでゴールデンテストを作成する。
- CLI コマンドの抽象化で必要になる API (例: 産業一覧取得) を洗い出し、`GameState` の公開メソッドに不足がないか確認する。
