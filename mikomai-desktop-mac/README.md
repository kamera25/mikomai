# Mikomai for macOS (デスクトップアプリ)

macOS 向けのネイティブ SwiftUI デスクトップアプリケーションです。共有 Rust ロジックを UniFFI（[`crates/mikomai-bindings`](file:///Users/kamera25/mikomai/crates/mikomai-bindings)）経由で呼び出し、ローカル LLM による対話、技術ドキュメントのナレッジ検索（RAG）、自律的なネットワーク機器調査および安全な設定変更支援を提供します。

---

## 主な機能

- **日本語チャット & 自律エージェント**:
  - **Worker 経路**: 技術ドキュメントに基づく手順解説や設定例の即時回答。
  - **Agent 経路**: 複数ステップにわたり、Ping/Trace、ARP・経路情報取得、機器設定取得、パケット解析などを自律実行して原因究明。
- **ローカル推論 (llama.cpp / Apple FM)**:
  - GGUF モデルをローカルで高速推論（Metal 最適化）。
  - Vision（Gemma 4 ＋ mmproj）対応により、トポロジ図や画面キャプチャの画像添付解析が可能。
  - Apple Foundation Models（`fm` CLI）バックエンドの利用にも対応。
- **安全な変更承認ゲート (OperationGate)**:
  - 設定変更、シリアルコンソール送信、FTP/TFTP 転送などの変更操作は、ハッシュ付き承認計画（`OperationPlan`）を作成して UI 上でユーザー承認を要求。未承認操作の実行を防止。
- **ネットワーク構成図の自動描画**:
  - 自然言語や会話履歴から構成図（nwdiag DSL）を自動生成し、SVG ベクター画像としてチャット内に埋め込み表示・保存。
- **機器接続 & 認証情報の保護**:
  - パスワードや Enable パスワードなどの機密情報は macOS Keychain に暗号化保存。LLM プロンプトやログへの露出を自動防止。
- **CPU監視・通知・履歴管理**:
  - バックグラウンドでの定期死活・CPU監視と通知機能。
  - エージェントの調査履歴および操作監査ログの閲覧。

---

## アーキテクチャ連携

```text
[SwiftUI UI層]
    │  ▲
    │  │ UniFFI (crates/mikomai-bindings)
    ▼  │
[mikomai-app] ── MikomaiService (Tokio Runtime / 状態・承認ゲート所有)
    │
    ├── [mikomai-core] (ドメイン・エージェント判断・NW正規化)
    └── [mikomai-adapters]
          ├── [mikomai-llm-llamacpp / apple] (推論)
          ├── FastEmbed E5 (ベクトル RAG)
          ├── SurrealDB / RocksDB (データ永続化)
          └── Netmiko Worker (SSH / Telnet / Serial 機器通信)
```

---

## セットアップと起動

### 開発実行
リポジトリルートから以下のスクリプトを実行します。

```bash
./mikomai-desktop-mac/run.sh
```

### 配布用アプリバンドル（.app）のビルド
```bash
./mikomai-desktop-mac/build-app.sh
```

---

## 設定と保存場所

- **設定ファイル**: `~/Library/Application Support/MikomaiDesktopMac/settings.json`
- **データベース (SurrealDB)**: `~/Library/Application Support/MikomaiDesktopMac/surrealdb/`
- **監視設定・タスク**: `~/Library/Application Support/MikomaiDesktopMac/watches.json`
- **生成アーティファクト (NW図 SVG 等)**: `~/Library/Application Support/MikomaiDesktopMac/artifacts/`
- **認証情報**: macOS Keychain（`keyring` クレート経由で安全に管理）

---

## テストと検証

### 1. Swift テスト
```bash
# 全体テスト
./mikomai-desktop-mac/test-core.sh

# 特定のテストのみ実行
./mikomai-desktop-mac/test-core.sh --filter AgentProgressTests
```

### 2. 実行キューと送信確認テスト
```bash
./mikomai-desktop-mac/test-execution-queue.sh
```

### 3. NW図の描画確認テスト
```bash
MIKOMAI_WINDOW_CHECK_SOURCE="$PWD/mikomai-desktop-mac/Tests/NetworkDiagramChecks/NetworkDiagramChecks.swift" \
  sh mikomai-desktop-mac/test-chat-window.sh
```

### 4. アクセシビリティ・UI 操作テスト
```bash
# キーボード操作、VoiceOver 読み上げ、フォーカス遷移の検証
sh mikomai-desktop-mac/test-accessibility.sh

# 入力フォーム・IME・補完の検証
sh mikomai-desktop-mac/test-chat-composer.sh
```

---

## アクセシビリティとキーボード操作

- **キーボードナビゲーション**: 全ての入力欄、ボタン、リスト項目は `Tab` / `Shift+Tab` で前後に移動可能。
- **ショートカット**:
  - `Enter` / `Space`: ボタンの実行、リスト行の選択
  - `F2`: 会話履歴の名前変更
  - `Esc`: モーダルや補完のキャンセル
  - `Tab`: 機器名候補の自動補完
- **VoiceOver**: 全ボタン、履歴、機器一覧セル、ステータスアイコンに適切なラベルと状態（選択中、未設定等）が付与されています。
