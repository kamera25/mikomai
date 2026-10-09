# mikomai - ネットワークAIアシスタントツール

ネットワーク機器の診断と技術文書の参照を支援する、macOSネイティブのAIアシスタントです。Swift製デスクトップアプリは `mikomai-desktop-mac/`、共有Rustロジックは `mikomai-core/`、Swift/C#から呼び出す生成UniFFIは `crates/mikomai-bindings/` にあります。

## 機能

- ローカルLLMと外部LLMを使った日本語の質問応答
- `nw-docs/` の技術資料を使ったナレッジ検索
- MCPを使ったネットワーク機器の読み取り診断
- macOS Keychainを使った認証情報の保護

## セットアップと起動

必要な開発環境は Rust stable、Swift toolchain、macOS SDK です。アプリを起動するにはリポジトリのルートで実行します。

```bash
./mikomai-desktop-mac/run.sh
```

macOSアプリバンドルを作る場合:

```bash
./mikomai-desktop-mac/build-app.sh
```

## CLI

CLIはRust workspaceの独立したパッケージです。

```bash
npm run cli -- --help
npm run cli -- chat "FITELnet F220 の VLAN 設定方法を教えて"
npm run cli -- rag-search "VLAN"
```

ドキュメントを取り込むには `./ingest.sh` または `npm run cli -- rag-ingest nw-docs` を使います。利用可能なコマンドは `npm run cli -- --help` で確認できます。

## キャッシュのクリーンアップ

```bash
./clean.sh
```

標準ではRust・Swift・C#のビルド生成物、配布バンドル、Pythonキャッシュと既知のMikomai用一時ビルドディレクトリを削除します。ソース、ログ、技術資料、モデル、会話・検索データは保持します。実行前にビルドを停止してください。

`-n` は削除対象と容量を表示するだけで、確認入力は不要です。`-y` は確認なしの削除、`-d` は `node_modules`、`venv`、`.fastembed_cache` も削除する深いクリーンアップです。非対話環境での削除には `-y` が必要です。環境変数で変更した外部ビルド先は自動削除しません。

## ライセンス

[LICENSE.md](LICENSE.md) を参照してください。
