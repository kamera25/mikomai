---
name: cli-verification
description: >-
  Verify Mikomai development, new features, responses, and bug fixes with mandatory mikomai-cli chat --debug-jsonl execution,
  change-specific acceptance criteria, and checks for behavior outside CLI coverage.
---

# mikomai-cli Chat Verification

コード修正・追加・バグ修正・設定変更の完了前、および新機能・応答確認時には、必ず `mikomai-cli chat --debug-jsonl` を実行します。通常テキスト出力や `--json` だけの実行で代用してはいけません。CLI の正常終了と変更した機能の正しさを別々に判定してください。

## 手順

1. 差分と実行経路を確認し、変更箇所を通る入力、期待結果、出てはいけない結果を実行前に決める。
2. CLI で確認できる範囲を特定し、範囲外の UI・LLM・MCP などには変更に応じた検証を選ぶ。
3. 最終変更後に基本質問と変更に対応した質問を `chat --debug-jsonl` で実行する。基本質問で変更箇所も確認できる場合は兼用する。通常出力・`--json` 自体の確認が必要な場合は追加実行する。
4. stdout の全行をJSONとして解析し、入力・内部処理・最終応答を確認する。終了コード・出力構造と、回答内容・実行された処理の証拠を分けて確認する。
5. 合格・不合格・未検証を検証項目ごとに報告し、未検証の機能まで成功と扱わない。

詳細な判定基準、実装上の制約、記録方法は [共通検証ガイド](../../skills.md) を参照してください。

## 基本コマンド

プロジェクトルートで実行します。

```bash
npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --debug-jsonl > /tmp/mikomai-verification.jsonl 2> /tmp/mikomai-verification.stderr

# 通常JSON出力自体の確認は、必須のJSONL検証とは別に実行する
npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --json
```

JSONLでは `cli_request`、内部の `llm_request` / `llm_response` または `agent_event`、最後の `core_response` を確認します。成功時は `core_response.payload.status == 0` と空でない `payload.text` が必要です。内部記録で変更箇所を通ったことを確認できない場合、その機能は未検証として報告してください。`--debug` はstderrの詳細ログ用であり、`--debug-jsonl` の代用にはなりません。`--json` / `-j` と `--debug-jsonl` は同時指定できません。

GGUFモデル設定時、`npm run cli` は共通FFIを通り、Swift UIと同じローカルLLM・E5 RAG/SurrealDB検索を使います。モデル未設定時は決定的なMarkdown検索へフォールバックします。CLIで確認できないSwift画面操作、Swift callbackを介した実機接続、承認後の実機変更は別に検証してください。`ok: true` と回答の存在だけで内容検証を完了しないでください。

## FastRouterと自然文フォールバックの検証

FastRouter・定型コマンドの追加や修正は、その経路の成功だけで完了にしません。同じ意図を自然文で言い換え、FastRouterに一致しない入力で通常のAgent/LLM経路も確認してください。内部ログや関連テストで、選択したツール・対象・引数、実際の結果、最終応答まで確認します。単なる文言一致やCLIの正常終了だけではフォールバックの合格にしません。

取得した結果から統計や回答を返す機能は、自然文経路でも必要な観測が取得できることを検証してから実装・完了してください。統計は実測結果に基づいて返し、成功率と損失率などの意味を明示します。回答を作るために完了した操作を繰り返さず、複数対象・複合依頼を単一操作の完了として扱わないでください。Swift callbackが必要な経路は本番FFI/Swiftの検証も併用し、モデルを置き換えたテストと実モデルの検証を区別して記録します。
