# Canonical / Graphベースのインターフェース確認MVP

`NakaokuGWのLAN1がupしているか確認して` のような単独要求を、既存の `get_state` に渡します。Intent・Canonicalモデル・Graph化・検証はメーカーに依存しません。読み取りコマンドの選択だけを機器アダプターに置きます。

```json
{"device":"NakaokuGW","resource":"interfaces","interface":"lan1","refresh":true}
```

## 実行経路

1. 登録機器へ既存のSwift/Keychain/Netmiko経路で接続し、出力の秘密情報を除去する。
2. Raw観測を取得時刻・対象・scope付きで保存する。
3. 共通の候補抽出でインターフェース名・IP・出典ブロックを列挙する。これは値と出典の抽出であり、up/downの判定処理ではない。
4. 制約付きLLMが候補インデックスと `up / down / unknown` を選ぶ。ベンダー固有のCLI構造・意味はLLMが解釈する。
5. 共通コードがJSONスキーマ、インデックス、出典との共存関係、観測の網羅性、IP・prefix、機器名、重複を検証する。不正な選択の修正は最大3回。モデル未ロード・推論失敗・検証失敗を、専用パーサーや自由文推論で代替しない。
6. `UniversalInterfaceTable` を保存し、interfaceノード・has_interface辺・IPの関係をGraph化する。
7. GraphからCanonical観測を再取得し、その `status` を使って要求を検証する。保存失敗や再取得不一致では成功を返さない。

LLMによる意味の分類と、コードによる構造・出典検証は別である。一般のCLI表示の意味をコードだけで完全に証明するものではない。名前や値を候補に制限し、元の出力とCanonical化の出典を監査可能に保持する。

## 状態の意味

- `status` はoperational/link状態であり、admin_stateやIP疎通とは区別する。
- 複数の物理ポートを持つインターフェースでは、少なくとも1つのポートが明示的にリンクアップならup。すべて明示的にdownならdown。曖昧ならunknownとする。
- 設定された速度・IP・過去のカウンターだけをupの証拠としない。
- 未登録・曖昧な対象は確認要求。単独要求は同じtaskの観測を使い、回答のために同じ操作を再実行しない。
- 明示的な現在状態確認はrefresh=true。通常のget_stateにはscope単位の60秒キャッシュを用いる。新しいRawだけの観測がある場合、古いCanonical観測へ戻らない。

共通モデルは既存のインターフェーススキーマを再利用する。初期の実機検証対象はヤマハのLANだが、専用状態パーサーは追加しない。ヤマハのアダプターは有効なLAN名だけを受け付け、`show status lan1` を選ぶ。その他の既存機器は従来の `show interfaces` を使う。全メーカーのコマンド・表示形式を実機検証済みという意味ではない。

単独CLIにはSwiftの機器接続callbackがないため、この要求はアプリへの案内を返す。CLI応答を実機検証の代用にしない。別の自然文表現では通常Plannerも同じget_stateを使用する。

## 検証

```sh
cargo test -p mikomai-core
cargo test -p mikomai-ffi interface_executor_canonicalizes
./mikomai-desktop-mac/test-core.sh --filter InterfaceObservationPolicyTests
```

実機・実モデルは対象を明示して検証する。保存済み接続・資格情報を読み、グラフとtask eventは一時保存先を使う。変更計画が出た場合は失敗する。

```sh
MIKOMAI_EXECUTION_CHECK_SOURCE="$PWD/mikomai-desktop-mac/Tests/InterfaceChecks/InterfaceChecks.swift" \
MIKOMAI_INTERFACE_CHECK_TARGET=NakaokuGW \
MIKOMAI_INTERFACE_CHECK_LAN=LAN1 \
MIKOMAI_INTERFACE_CHECK_MODEL=/path/to/model.gguf \
./mikomai-desktop-mac/test-execution-queue.sh
```

`MIKOMAI_INTERFACE_CHECK_PROMPT` を指定すると通常Plannerへ入る言い換えも検証できる。
CLIは基本質問と機能の質問を必ず `chat --debug-jsonl` で実行し、全行をJSONとして解析する。

## 2026-10-04の検証結果

| 項目 | 結果 |
| --- | --- |
| Core | 130件成功。候補の出典検証、説明文からの誤抽出防止、対象省略時の再計画を含む |
| 対象FFI | 成功。Canonical保存、Graphのinterfaceノード・辺、再取得、キャッシュ、失敗したrefreshの古い結果不使用 |
| Swift | InterfaceObservationPolicyTests成功。アプリ最終ビルド・署名検証成功 |
| 必須CLI基本質問 | 隔離したRAG/Graphで437行すべて有効JSON、終了コード0、core_response.status=0。F220のAccess/Trunk資料と回答を照合 |
| 必須CLI機能質問 | 4行すべて有効JSON、終了コード0、status=0。実機接続経路のないCLIはアプリへ案内し、up/downを推測しない |
| 定型文の実機・実モデル | NakaokuGW、保存済みSSH/22、LAN1。Raw→実モデル→Canonical→Graph再取得→up、status=0 |
| 自然文の実機・実モデル | 「NakaokuGWでLAN1のリンク状態を観測してください…」で通常Agentを使用。target=NakaokuGW、interface=LAN1、refresh=true。取得1回、Graph再取得、最終応答「upです」、終了コード0・status=0 |

実モデルはGemmaのローカルGGUFを使用。自然文の観測時刻は2026-10-04 14:16:34 JST。設定変更は実行していない。アプリで使う場合はビルド済みの `mikomai-desktop-mac/dist/Mikomai.app` を再起動する。

検証の制限・別途残る問題:

- FFI全体のテストは終了処理で異常終了し、全体合格とは判定していない。新規インターフェーステストを除外しても再現した。対象FFIテストは終了コード0で成功。
- 既存のRAG索引でSurrealDB HNSW内部エラーが発生したため、基本質問の最終検証は隔離した新規Graph/Knowledgeで実施。ユーザーの索引は削除・修復していない。
- 実機検証はヤマハLAN1のみ。Cisco/Junosの表示例は共通Canonical化のテストで確認したが、各社実機・画面操作・設定変更の検証は含まない。

AFMのインターフェースCanonical化も対応した。バックエンド選択、共通検証、実モデル・実機の検証結果は [AFM対応の記録](afm-interface-canonicalization.md) を参照。
