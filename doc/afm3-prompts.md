# AFM3 向けプロンプトと選択資料の翻訳

モデル向けの指示は英語で記述し、利用者への回答は日本語を維持する。共通システムプロンプトは AFM の回答セッションにも渡す。

RAG の資料、インデックス、検索クエリは日本語のまま維持する。AFM 選択時に限り、検索・選択済みの参照資料のコピーを、回答用とは独立した AFM 翻訳セッションで英訳する。通常の回答経路とエージェントの参照資料取得経路に適用する。添付資料と会話履歴はこの翻訳の対象に含めない。

選択資料の出典ラベルとコードブロックは原文を保持し、インラインコード、Markdown リンク、数値は仮の文字列に置換して翻訳後に復元する。日本語の説明文は分割して翻訳し、元の資料へ書き戻さない。翻訳は最大4断片・約400文字ごとに処理し、文ごとに独立した専用セッションで通常の文章として生成する。アプリ側で JSON に符号化し、モデルが JSON を生成することには依存しない。不正な翻訳バッチは細分化して再試行する。翻訳後は配列の件数、空文字、改行、未翻訳の日本語、数値リテラルの変更を検査する。これらの検査だけで翻訳の意味の正確性を保証するものではない。

翻訳の失敗やキャンセルはエラーとして伝え、翻訳途中の資料を回答の根拠として使用しない。デバッグ記録には `rag_translation_request` / `rag_translation_response` を出力する。

## 検証

```bash
cargo test -p mikomai-core
cargo test -p mikomai-ffi llm_runtime::tests
cargo test --manifest-path crates/mikomai-llm-apple/Cargo.toml
# AFM システムモデルを利用できる macOS で明示実行
cargo test -p mikomai-ffi apple_selected_rag_translation_preserves_source_and_generates_japanese_answer -- --ignored --nocapture
npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --debug-jsonl
```

CLI は現状 AFM バックエンドを直接選択するオプションを持たないため、CLI の成功だけで AFM 翻訳の検証を完了としない。実モデルのテストは、選択資料の英訳、出典・コード保持、日本語回答と出典の存在を確認する。

## 次に評価する項目

- 日本語資料と英訳後資料による AFM 回答を同じ質問で比較し、正確性、出典、回答言語、待ち時間を測定する。
- VLAN、ルート、ARP、複数資料、長文、資料内の命令を含むケースで翻訳の忠実度を確認する。
- 測定結果に応じ、資料内容と翻訳プロンプトの版をキーにした翻訳キャッシュを検討する。
- CLI から AFM を明示選択できるようにし、翻訳イベントを含めて JSONL で一貫して検証できるようにする。

## 2026-10-03 の検証結果

- Core: 107件合格。原資料の保持、コード・数値の復元、自然な数量表現、未翻訳・不正応答の拒否、分割処理を確認。
- AFM実モデル: F220 の Access/Trunk VLAN 資料本文の英訳、出典・設定テンプレート保持、日本語回答と出典表記を確認。専用テストは23.86秒で合格。
- CLI: 一時DBを指定して `chat --debug-jsonl` を実行。終了コード0、全437行をJSONとして解析、最終 `core_response.payload.status == 0`。llama.cpp に日本語の選択資料が渡ることと、F220資料に整合した日本語回答・出典を確認。
- FFI全体: `c_abi_reports_errors_answers_from_local_documents_and_frees_results` でSIGABRT。一時DB・順次実行でも再現し、全体検証は未完了。今回の翻訳処理との因果関係は未確定。関連する `llm_runtime::tests` は合格。

FFI全体の異常終了の原因調査と修正も、次の検証整備に含める。
