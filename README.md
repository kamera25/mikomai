# mikomai - ネットワークAIアシスタントツール

ネットワーク機器の診断と技術文書の参照を支援する、macOSネイティブのAIアシスタントです。Swift製デスクトップアプリは `mikomai-desktop-mac/`、共有Rustロジックは `mikomai-core/`、Swiftから呼び出すRust FFIは `crates/mikomai-ffi/` にあります。

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

`-n` は削除対象の確認、`-d` は `node_modules`、`venv`、埋め込みモデルキャッシュを含む深いクリーンアップです。

## ライセンス

[LICENSE.md](LICENSE.md) を参照してください。
