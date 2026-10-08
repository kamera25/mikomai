# mikomai-cli 実行マニュアル

`mikomai-cli` は Rust workspace に含まれるヘッドレスCLIです。Swiftアプリとは別プロセスで動作し、macOSアプリの設定やKeychainの内容を自動共有しません。

## 実行方法

リポジトリのルートで `cargo run` またはnpmショートカットを使います。

```bash
cargo run -p mikomai-cli -- --help
npm run cli -- --help
npm run cli -- chat "FITELnet F220 の VLAN 設定方法を教えて"
```

利用できるサブコマンドとオプションは `--help` を参照してください。JSON形式が必要な場合は `--json` を指定できます。

`chat --debug-jsonl` を指定すると、Swift版と同じ形式のデバッグ記録を標準出力へJSONLで出力できます。使い方とレコード形式は [Chatコマンド仕様](mikomai-cli-chat.md) を参照してください。

## 知識文書

Markdown資料は `nw-docs/` から取り込み・検索できます。

```bash
npm run cli -- rag-ingest nw-docs
npm run cli -- rag-search "VLAN 設定"
```

独立CLIの知識ストア保存先は `MIKOMAI_KNOWLEDGE_DIR` で指定します。省略時はOSの一時ディレクトリ配下の `mikomai-knowledge` が使われます。

## デバイス操作

`devices` はCLI用に設定されたデバイス一覧を表示します。Swiftアプリが保持する接続情報や認証情報は自動では引き継がれません。実機への接続経路はアプリの構成と区別して扱ってください。

`./ingest.sh` は `nw-docs/` を取り込むショートカットです。

`query-state '<JSON>'` と `diff-state '<JSON>'` は実機アクセスを行わず保存済み観測を検索・比較します。入力・出力・上限は [保存済み状態API](stored-state.md) を参照してください。
