# mikomai-cli chat

`chat` はヘッドレスCLIから質問に回答するコマンドです。現在のCLIは `mikomai-core` のアプリケーションサービスとローカルMarkdown用知識ストアを使います。SwiftアプリのLLM設定、会話履歴、Keychain、実機接続状態を共有するものではありません。

```bash
npm run cli -- chat "FITELnet F220 の VLAN 設定方法を教えて"
npm run cli -- --json chat "ルーティングの基本を説明して"
```

`nw-docs/` が存在する場合、実行時に資料を取り込み、検索結果を回答に利用します。保存先は `MIKOMAI_KNOWLEDGE_DIR` で変更できます。

JSON出力では回答は `data.response` に入ります。

```json
{"ok":true,"data":{"response":"..."}}
```

CLIとmacOSアプリは別々の実行経路です。CLIでの動作確認は、Swift UI、アプリ内LLM設定、実機MCP操作の検証にはなりません。
