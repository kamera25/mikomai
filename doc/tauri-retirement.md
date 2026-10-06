# Tauri廃止とSwift移行

この文書はTauri廃止時点の記録です。現在の保存・取込・CLI方針は [再設計の実施状況](refactoring-plan.md#13-実施状況) を参照してください。以下の旧保存先・import記述は現行動作を表しません。

React/Tauriアプリ、Tauri Rustランタイム、専用IPC/MCP実装を削除し、デスクトップ入口をSwiftUI + Rust core/FFIへ統一しました。共有する推論・計画・実行ポリシーは `mikomai-core` と `mikomai-adapters` に置き、OS固有のKeychainとNetmiko起動はSwift callbackに閉じています。

| 機能 | Swift移行後 |
| --- | --- |
| Worker/Agent振り分けとplanner decision検証 | `mikomai-core` のportable APIをFFIから呼び出します。履歴、添付、取得資料を含めLLM plannerを複数stepのAgentLoopへ接続しています。 |
| 読み取り型の機器調査 | 疎通確認、route/IP情報、serial列挙、`get_state`、`fetch_config`、`fetch_routing`、`fetch_arp`、`network_show` をCore allow-listから実行します。Swift callbackがKeychain資格情報でNetmikoを呼び出します。取得したconfig等はRust側でsecret redactしてからplannerへ渡します。 |
| Graph / RAG | SurrealDB graphと `rag_chunk` を `~/Library/Application Support/com.mikomai.agent/surrealdb` から再利用し、MAC/IP lookup、subgraph、`query_nw_db` / `network_query_nw_db` / `query_rag` を提供します。RAGは旧Multilingual E5-Large vector検索・vendor filter・source citationを使い、初回時にFastEmbedがmodelをcacheへ取得します。 |
| Packet / NWDiag / Cisco config helpers | Packet analyze/prepare/safetyはportable Coreで評価し、送信を直接行いません。NWDiagは共有wrapperからSVGを生成しSwift artifactsへ保存します。Cisco validate/convertは移植したPython helperとArista/Juniper templatesを共有assetsから呼び出します。 |
| 設定・console・file transfer | `network_config`、`network_send_console_message`、FTP/TFTP upload/downloadはhash付きOperationPlanとしてSwiftの確認・承認UIへ送ります。FFIは承認plan hashとExecuting statusを再確認してからportable serial/transfer adapterを呼びます。Keychain資格情報はFTPにのみ渡し、TFTPへは渡しません。 |
| ユーザー選択 | ask-choice toolは質問と候補をnative chatに表示し、返信をpending task idで保持したAgent taskへ続けます。 |
| Scheduler / Watch | portable `PortableWatchService` が旧 `watches.json` とExecution IR v1を読み込み、共有Tokio runtimeで定期実行します。Swift UIから監視の作成・編集・開始・停止・即時実行・削除、実行状態と履歴を操作できます。CPU監視は旧vendor別show commandとnumeric usage parserを使い、条件成立時はnative通知を出します。IRで許可されるのはread-only `get_state(cpu)` と `notify` のみです。 |
| Agent task / operation audit | Agent eventとTaskSnapshot/evidenceをnative Application Supportへ保存し、旧task event JSONを一度だけ取り込みます。Swift UIで一覧・詳細・保存済みevidenceからの再開ができます。操作監査はsecret-redacted SHA256 chain付きNDJSONとして保存し、native UIから参照できます。 |
| LLM回答・system prompt・Netmiko wrapper | SwiftチャットとCLIが共通FFI推論器を使用します。共有資産は `mikomai-core/assets/` に置きます。 |
| CLI | `mikomai-cli` はTauriから独立し、ローカル設定または `MIKOMAI_MODEL_PATH` で選んだGGUFをFFIから読み込みます。モデル未設定時のみ、明示的な非生成ナレッジ応答へフォールバックします。 |
| Swift設定 | `~/Library/Application Support/MikomaiDesktopMac/settings.json` へ保存します。既存インストールの設定候補は読み込み時に一度だけimportし、以後の保存先はSwift-nativeパスです。 |
| 接続情報・資格情報 | Swift側の接続情報とmacOS Keychainを使用します。既存JSON/CSVのメタデータimport互換は維持します。 |

React/Tauri固有UI、IPC command transport、個別MCP server processは削除しました。ScheduledTasks/Watch/TaskAuditの旧レイアウトはSwiftへ再現していませんが、定期実行、監視、状態・履歴確認、auditからのtask再開はnative UI/FFIへ移植しています。agentから呼べる旧37 tool IDは、実行方式に応じてCore/adapter、Swift callback、または承認済みplanに割り当てています。主な対応群は以下です。

- Read-only/device: `self_network_ping`, `self_network_traceroute`, `self_network_test_connection` / `self_network_test_net_connection`, `self_network_route`, `network_get_ip_info`, `network_list_serial_ports`, `network_show`, `fetch_config`, `fetch_routing`, `fetch_arp`, `get_state`。
- Knowledge/graph: `query_nw_db`, `network_query_nw_db`, `query_rag`, `query_network_graph`, `get_subgraph`, `find_ip_by_mac`, `find_mac_by_ip`, `find_interface_by_mac`, `require_host_registered`。
- Analysis/helpers/choices: `network_packet_analyze`, `network_packet_prepare`, `network_packet_safety`, `self_network_nwdiag`, `validate_cisco_config`, `convert_cisco_config`, `ask_user_choice`, `ask_interface_choice`, `ask_ipaddress_choice`, `get_operation_plan`。
- Approval-gated writes: `network_config`, `network_send_console_message`, `network_ftp_download`, `network_ftp_upload`, `network_tftp_download`, `network_tftp_upload`。

MCP server process群とUI自体は削除しましたが、上記tool response pathを単に廃止したわけではありません。実機へのSSH/serial/file transferはユーザーが接続先・credential・承認を用意した場合のみ検証可能です。fastembed model取得ができない環境では、既存DBに対するvector RAG検索は起動できません。

## Portable資産

- `mikomai-core/assets/system_prompt.txt`
- `mikomai-core/assets/network/netmiko_wrapper.py`
- `mikomai-core/assets/network/netmiko_patches.py`
- `mikomai-core/assets/network/config_helper.py`
- `mikomai-core/assets/network/nwdiag_wrapper.py`
- `mikomai-core/assets/templates/arista.j2`
- `mikomai-core/assets/templates/juniper.j2`
- `mikomai-core/assets/bin/netmiko_wrapper-macos-arm64`

Tauri directoryやそのruntime pathをSwift build/runtimeから参照しません。接続・承認の安全ゲートはFFI境界とfake/loopback transportで検証し、実機SSH/serial/config changeの結果は実環境での操作を意味しません。
