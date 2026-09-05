# mikomai-cli 実行マニュアル

`mikomai-cli` は、Mikomai デスクトップアプリケーションと共通の Rust コアランタイム（MCP、SurrealDB、RAG、ネットワーク機器接続エンジン）をコマンドラインから直接実行できる CLI ツールです。

GUI を起動せずに、登録済みネットワーク機器の状態確認（Read-Only）、SurrealDB ナレッジベースへのドキュメント取り込み（Ingest）、RAG 検索などを実行できます。

---

## 1. 実行方法

プロジェクトルートから実行する場合、複数の方法があります。通常は **npm スクリプト** または **cargo** を利用します。

### 方法 A: npm スクリプト経由（推奨）

`package.json` にショートカットが登録されているため、最も手軽に実行できます。引数を渡す際は `--` の後に続けて指定します。

```bash
# ヘルプ表示
npm run cli -- --help

# サブコマンドの実行例
npm run cli -- resources
npm run cli -- devices
npm run cli -- rag-search "VLAN 設定"
```

### 方法 B: Cargo 経由（プロジェクトルートから）

```bash
# プロジェクトルートから直接 cargo run
cargo run --manifest-path src-tauri/Cargo.toml --bin mikomai-cli -- <サブコマンド> [オプション]

# 例:
cargo run --manifest-path src-tauri/Cargo.toml --bin mikomai-cli -- devices
```

### 方法 C: Cargo 経由（`src-tauri` ディレクトリから）

```bash
cd src-tauri
cargo run --bin mikomai-cli -- <サブコマンド> [オプション]
```

### 方法 D: ビルド済みバイナリの直接実行

一度ビルドしておくと、Cargo の起動オーバーヘッドなしで高速に実行できます。

```bash
# デバッグビルド
cargo build --manifest-path src-tauri/Cargo.toml --bin mikomai-cli
./src-tauri/target/debug/mikomai-cli --help

# リリースビルド（最適化済み）
cargo build --release --manifest-path src-tauri/Cargo.toml --bin mikomai-cli
./src-tauri/target/release/mikomai-cli --help
```

---

## 2. グローバルオプション

すべてのサブコマンド共通で使用できるオプションです。

| オプション | 説明 |
| --- | --- |
| `--json` | 出力を機械可読な JSON 形式で標準出力に表示します（スクリプトや CI 連携向け）。 |
| `-h`, `--help` | ヘルプメッセージを表示します。 |
| `-V`, `--version` | バージョン情報を表示します。 |

> **Note**: `--json` オプションを指定した場合、出力形式は常に以下の共通ラッパー構造になります。
> ```json
> {
>   "ok": true,
>   "data": { ... }
> }
> ```

---

## 3. サブコマンド一覧

`mikomai-cli` には以下の 6 つの主要サブコマンドがあります。

| サブコマンド | 概要 | 主な用途 |
| --- | --- | --- |
| `chat` | AI アシスタント対話・エージェント実行 | 質問回答、コンフィグ生成、自律的機器診断 |
| `devices` | 登録済み機器一覧の表示 | ホスト名、IPアドレス、接続種別の確認 |
| `resources` | 対応状態リソース一覧の表示 | `get-state` で利用可能なリソース名の確認 |
| `get-state` | 機器の状態観測（読み取り専用） | ARP、ルーティング、インターフェース等の取得 |
| `rag-ingest` | ナレッジベースの再構築 | Markdown 技術文書を SurrealDB に取り込み |
| `rag-search` | ナレッジベースの検索 | 技術文書・コマンド仕様の RAG 検索 |

---

## 4. サブコマンド詳細・実行例

### 4.1 `devices`（登録機器一覧）

Mikomai デスクトップアプリで登録された管理対象ネットワーク機器（ホスト名、IP アドレス、接続プロトコル等）を一覧表示します。

```bash
# 通常出力（タブ区切りテキスト）
npm run cli -- devices

# 出力例:
# NakaokuGW    192.168.50.1    SSH
# F220                         Console

# JSON 形式で出力（jq 等でフィルタ可能）
npm run cli -- --json devices
```

### 4.2 `resources`（対応リソース一覧）

`get-state` サブコマンドで取得できるネットワーク状態リソースのキー一覧を出力します。

```bash
npm run cli -- resources

# 出力例:
# arp
# routes
# interfaces
# lldp
# mac_table
# bgp
# ospf
# cpu
```

### 4.3 `get-state`（機器状態の取得）

指定した登録済み機器から指定リソースの状態を読み取ります。**読み取り専用（Read-Only）の安全な操作**です。

```bash
npm run cli -- get-state <DEVICE> <RESOURCE> [OPTIONS]
```

#### 引数・オプション:
- `<DEVICE>`: 登録済みのホスト名（例: `NakaokuGW`, `F220`）または IP アドレス。
- `<RESOURCE>`: リソース名（エイリアス対応）。
  - `arp`
  - `routes`（エイリアス: `route`, `routing`）
  - `interfaces`（エイリアス: `interface`, `int`）
  - `lldp`（エイリアス: `cdp`）
  - `mac_table`（エイリアス: `mac-table`, `mactable`, `mac_address_table`, `mac`）
  - `bgp`
  - `ospf`
  - `cpu`
- `--message <MESSAGE>`: 実行コンテキストや証拠として残す補足メッセージ（任意）。
- `--output <pretty|raw>`: 出力形式の指定（デフォルト: `pretty`）。
  - `pretty`: デバイス名やヘッダー付きで整形出力。
  - `raw`: 機器からの応答生テキストのみを出力。

#### 実行例:

```bash
# NakaokuGW のインターフェース状態を取得（整形表示）
npm run cli -- get-state NakaokuGW interfaces

# 生テキストのみ取得
npm run cli -- get-state NakaokuGW routes --output raw

# JSON 形式で取得
npm run cli -- --json get-state NakaokuGW arp
```

### 4.4 `rag-ingest`（ナレッジベース取り込み）

Markdown 形式のネットワーク技術文書・設定ガイドをパースし、埋め込みベクトル（FastEmbed）を生成して SurrealDB のベクトルストア / グラフに保存します。

```bash
npm run cli -- rag-ingest [PATH] [OPTIONS]
```

#### 引数:
- `[PATH]`: Markdown ファイルまたはドキュメントフォルダのパス（省略時は `./nw-docs` が使用されます）。

#### 実行例:

```bash
# デフォルト（nw-docs ディレクトリ）を取り込み
npm run cli -- rag-ingest

# 特定のファイルを指定して取り込み
npm run cli -- rag-ingest nw-docs/yamaha/01-1_setup_ip.md

# JSON 形式で取り込み結果のチャンク数を確認
npm run cli -- --json rag-ingest nw-docs
```

> **Note**: プロジェクトルートにある `./ingest.sh` スクリプトも内部でこのコマンドを呼び出しています。

### 4.5 `rag-search`（ナレッジ検索）

SurrealDB ナレッジベースに対してベクトル類似度検索を実行し、根拠引用（ソースパス、類似度スコア、引用ブロック）と検索結果を表示します。

```bash
npm run cli -- rag-search <QUERY> [OPTIONS]
```

#### 引数:
- `<QUERY>`: 検索したい自然言語の質問文またはコマンド名。

#### 実行例:

```bash
# VLAN 設定に関するドキュメントを検索
npm run cli -- rag-search "VLAN 設定手順"

# FITELnet の trunk 設定について検索
npm run cli -- rag-search "FITELnet F220 trunk vlan"

# JSON 出力（RAG 評価スクリプト等で使用）
npm run cli -- --json rag-search "IP アドレス 設定"
```

### 4.6 `chat`（AI アシスタント対話・エージェント実行）

デスクトップアプリのチャット画面と同じ統合パイプライン（LLM、SurrealDB RAG、登録機器情報、過去履歴、MCP ツール実行エンジン）を CLI から直接実行します。

入力されたメッセージの内容に応じて、システムが以下の 2 つの実行モードを自動的に選択します：
- **Worker モード**: ドキュメント解説やコンフィグ生成など、機器の直接観測を伴わないタスク（Router / Knowledge Worker による高速生成）。
- **Agent モード**: 実機の状態確認やトラブルシューティングなど（`AgentLoop` が自律的に安全ポリシー検査を行い、MCP ツールで状態を取得して総合診断）。

```bash
npm run cli -- chat "<MESSAGE>" [OPTIONS]
```

#### 引数:
- `<MESSAGE>`: ネットワークアシスタントへの指示、質問、またはトラブル調査依頼。

#### 実行例:

```bash
# ドキュメントベースの質問・コンフィグ生成（Worker モード）
npm run cli -- chat "YAMAHA RTX1210 で LAN1 に 192.168.100.1/24 を設定するコマンドを教えて"

# 実機状態の確認と自律診断（Agent モード: 登録済みホスト名を含める）
npm run cli -- chat "NakaokuGW のインターフェース状態を確認して問題がないか要約して"

# JSON 形式で回答を取得（スクリプトやボット連携向け）
npm run cli -- --json chat "FITELnet F220 の VLAN 設定手順の要約"
```

> 詳しい内部アーキテクチャや自律実行の流れは [mikomai-cli-chat.md](file:///Users/kamera25/mikomai/doc/mikomai-cli-chat.md) を参照してください。

---

## 5. スクリプト・自動化での活用例

### 5.1 `jq` と組み合わせた機器情報抽出

```bash
# 登録済みデバイスから SSH 接続のホスト名一覧を取得
npm run cli -- --json devices | jq -r '.data[] | select(.type=="SSH") | .hostname'
```

### 5.2 RAG の自動評価スイートでの利用

RAG の検索精度評価スクリプト `scripts/rag_eval.py` は、内部で `mikomai-cli --json rag-search` を実行して各クエリの Recall@k や根拠スコアを検証しています。

```bash
# RAG 評価スイートの実行
python scripts/rag_eval.py --cases eval/rag_cases.json --report eval/rag-report.json
```

### 5.3 `chat` を用いた定期ヘルスチェック・自動レポート

シェルスクリプトや cron と組み合わせることで、実機の状態確認と AI による自然言語サマリーを自動生成できます。

```bash
#!/bin/bash
# 毎朝のネットワーク簡易診断
REPORT=$(npm run cli -- --json chat "NakaokuGW のルートテーブルとインターフェースを確認して異常の有無を教えて" | jq -r '.data.response')
echo "=== 診断結果 ==="
echo "$REPORT"
```

---

## 6. 注意点

1. **SurrealDB の排他制御**:
   SurrealDB は組み込み RocksDB（`~/Library/Application Support/com.mikomai.agent/surrealdb`）を使用しています。Tauri GUI アプリが起動中の状態で `rag-ingest` などの書き込み系 CLI を実行すると、DB ロック競合が発生する場合があります。DB 操作を行う際は、GUI アプリを終了してから実行することを推奨します。
2. **LLM 設定の事前準備**:
   `chat` コマンドはデスクトップアプリと共通の LLM 設定（`settings.json`）を参照します。ローカルモデル（GGUF）または API キー（OpenAI, Claude, Ollama 等）が正しく設定されていない場合、回答の生成に失敗します。事前に GUI アプリの設定画面で LLM の接続を確認してください。
3. **モデル読み込みと初回実行時間**:
   ローカル LLM（llama.cpp）を使用している場合、初回起動時にモデルファイルのロードに数秒〜十数秒を要することがあります。
4. **ログ出力**:
   初回のモデル読み込み時や ONNX Runtime（ort）初期化時に、デバッグログが標準エラー／標準出力に出力される場合があります。JSON 連携する際は、`--json` モードでのパースに標準出力の JSON 部分をご利用ください。
