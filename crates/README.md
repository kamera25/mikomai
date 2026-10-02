# Mikomai workspace crates

依存方向はリポジトリ直下の `mikomai-core/`（domain/application/port）を中心に、`crates/mikomai-adapters`、`crates/mikomai-cli`、`crates/mikomai-ffi` が外向きに依存する形に固定する。coreからOS固有UI、DB、LLM、外部プロセスの起動方法を参照しない。

LLM は次の境界を使う。既存の core 配置はビルド・スクリプト互換性のため維持する。

- `mikomai-llm`: core の `InferencePort` / streaming / Vision を再公開する共通契約。core はこの crate や具体的な backend に依存しない。
- `mikomai-llm-llamacpp`: 従来の GGUF 読み込み、推論パラメータ、キャンセル、ストリーミング、Vision、ネイティブログ制御を所有する。`mikomai_adapters::local_llama` は互換用の再公開。
- `mikomai-llm-apple`: macOS の target-specific dependency。非 macOS の `--workspace` ビルドにも入らないよう workspace member から除外し、実装も `cfg(target_os = "macos")` で保護する。Swift / C FFI / Apple SDK へのビルド依存はない。

新しい backend も `InferencePort` を実装し、既存の core の呼び出し箇所に渡せる。
`capabilities()` は、この連携で提供するテキスト生成・構造化出力・tool calling とトークン制限を返す。
構造化出力/tool calling は現在両 backend とも専用 API を提供していないため false。自由文から JSON を抽出する従来の Agent 動作は変更しない。
`availability()` はコンパイル対象とは独立した実行時の状態。従来の callback adapter は Unknown、llama.cpp は GGUF のロード状況、Apple は `fm available --model system` を確認する。

Apple の利用例（macOS のアプリケーション側で選択する。既存 CLI/FFI の既定は llama.cpp のまま）:

```rust,ignore
use mikomai_llm::InferencePort;
use mikomai_adapters::apple::AppleInference;

let backend = AppleInference::default();
let capabilities = backend.capabilities();
let availability = backend.availability();
let answer = backend.complete("短く挨拶してください").await?;
```

初期実装は Apple の `/usr/bin/fm`（`available`, `count-tokens --quiet`, `respond --no-stream`）を使用する。
CLI が存在しない macOS やモデル準備未完了時は実行時に unavailable を返す。インストールや利用規約への同意は行わない。
stdin でプロンプトを渡すので、シェル展開や先頭の `--` によるオプション解釈は起きない。
各リクエストは独立したセッションで、暗黙の会話履歴・core の長い system prompt は追加しない。
Apple 側で保守的な 4096 トークンの予算を使い、応答用に 1024 を確保するため入力は 3072 以下に制限する。
計数失敗や超過はエラーとし、入力を黙って切り捨てない。fm が出力トークン上限を指定できないため `max_output_tokens` は None。
これは生成長の保証ではなく応答用の余裕であり、モデルの context 超過等の実行時エラーもそのまま呼び出し側に返す。
`AppleTransport` を差し替えることで core を変更せずにネイティブ連携へ移行できる。

検証: `cargo check --workspace`、`cargo test -p mikomai-core -p mikomai-llm -p mikomai-llm-llamacpp`。
macOS では `cargo test --manifest-path crates/mikomai-llm-apple/Cargo.toml --target-dir target` と
`cargo run --manifest-path crates/mikomai-llm-apple/Cargo.toml --target-dir target --example smoke` で Apple を個別確認できる。
