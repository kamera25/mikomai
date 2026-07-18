<p align="center"><img src="src-tauri/icons/icon.png" width="128"></p>

# mikomai - ネットワークAIアシスタントツール

### Managed Infrastructure Knowledge Operator of Ml Agent Interface

<img src="doc/screenshot.png" width="1000">

ネットワーク機器の診断および技術文書の参照を支援するAIアシスタント。ローカルLLMとRAG（検索拡張生成）を統合したデスクトップアプリケーションです。

## 機能

- **推論エンジン**: ローカル環境のGPU（Metal等）を使用した低遅延な推論。
- **ネットワーク診断**: MCP（Model Context Protocol）によるツールの自動実行（Ping, Traceroute, ARP, IP情報取得等）。
- **ドキュメント検索**: 独自ナレッジベース（NW-DB）を対象としたRAG機能。
- **管理機能**: セッション履歴の自動要約、およびネットワーク接続ホスト管理。

## 技術構成

- **Core**: Tauri / Rust
- **Frontend**: React / TypeScript
- **Inference**: Llama.cpp
- **Storage**: LanceDB (Vector Store)

## セットアップ

### 依存関係のインストール

```bash
npm install
```

### 開発サーバーの起動

```bash
npm run tauri dev
```

### ビルド

```bash
npm run tauri build
```

### キャッシュのクリーンアップ

プロジェクト内の不要なビルドキャッシュや一時ファイルを一括で削除し、ディスク容量を解放するためのスクリプトが用意されています。

```bash
./clean.sh
```

- **通常クリーンアップ（デフォルト）**: `src-tauri/target`（Rustビルド成果物）、`dist`、`build`、Pythonキャッシュ（`__pycache__`、`*.pyc`）、`.pytest_cache`、OS一時ファイル（`.DS_Store`）などを安全に削除します。
- **ディープクリーンアップ（`-d` / `--deep`）**: 通常のターゲットに加え、再構築に時間がかかる `node_modules`、`venv`、`.fastembed_cache`（ダウンロード済みの埋め込みモデル）も削除対象に含めます。
- **ドライラン（`-n` / `--dry-run`）**: 実際の削除は行わず、どのファイルが削除され、どれだけの容量が解放されるかのシミュレーション結果を表示します。

## ライセンス

[LICENSE.md](LICENSE.md) を参照してください。
