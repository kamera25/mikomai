# 段階3〜6: Rust app・常駐ワーカー・排他・UniFFI

2026-10-07。`greenfield_redesign.md` §9の段階3〜6を実装。Windows/Vulkan/WinUIは今回の対象外。

## 最終構成

- `mikomai-app`が操作、機器判定、OSコマンド、資格情報、ワーカー、保存を所有する。Swift DesktopCoreは表示DTO、画面状態、Rustへの委譲を担当する。旧取込・CSV/JSON import・Swiftの機器実行callbackとProcess/Keychain実装を廃止。
- sessions/connections/settings/tasks/operations/approvals/watches/audit/claims/graph/rag_chunkをschema_version付きSurrealDB正本へ保存。GUIとCLIの既定DBを統一。最初にDBを開いたプロセスが同一ユーザー専用Unixソケットで保存サービスを提供し、他プロセスは公開メソッドだけを呼ぶ。別タスクの並行追加をマージし、同じ項目の競合は拒否する。
- Rust keyringはUUIDをキーとしてmacOS Keychainを利用する。資格情報はワーカーのstdinだけに渡し、出力を秘密値・設定内の秘密行について除去する。CLIは資格情報をargvで受けず、`credentials-stdin`を使う。
- PyInstallerの常駐ワーカーにNetmiko・pyserial・Cisco設定検証/変換・テンプレート・nwdiag・日本語フォントを同梱。日本語SVGには使用グリフを埋め込む。利用者のPython/OSフォント探索をしない。中断要求後にワーカーを終了し、変更の送信後に応答が確定しない場合はUnknownとする。次の要求で再起動し、変更を自動再送しない。
- Schedulerは5枠。人の判断待ちと機器ロック待ちでは枠を解放。推論は最終回答 > Planner > Watch、同優先度はFIFO。待機中・実行中のキャンセルを区別する。
- 機器ID単位の公平な読み書きロック、serialポート排他、変更/serialのプロセス間advisory lockを導入。読み取り60秒・変更300秒の待機期限はsettingsの`deviceReadLockWaitSeconds`/`deviceWriteLockWaitSeconds`で変更できる。期限後はAwaitingUserとして明示resume/cancelを待つ。Swiftに継続/中止操作を追加。Watchはbusyを見送り監査へ記録する。
- 承認対象の変更不能な内容とplanHashを照合し、正本側でApproved→Executingを一度だけ取得する。operations/approvalsを一つのトランザクションで保存。保存失敗・未承認・再利用は送信前に拒否。設定変更前後のread-backと行差分を返し、不確定な送信・検証失敗はUnknownとする。read-backをdesired stateの完全な意味的検証とは扱わない。
- UniFFI `=0.31.0`、C# generator `v0.11.0+v0.31.0`を固定。`mikomai-bindings`のみがアプリのforeign ABIを公開し、旧`mikomai-ffi`と手書きheader/shimを削除。Rust内部の旧関数とSwiftのowned adapterは互換呼出し用で、言語境界は生成UniFFIを通る。submit/query/cancel/resume/subscribeを公開。
- TaskEventはtask_id/seq/versionを持ち、通知前に正本へ保存。queryでイベントとsnapshotを回復する。CLI JSONLも正本に記録し、`task-query <id>`で別プロセスから取得できる。LLM要求は内部のscheduled Chatタスクで実行する。

## 検証

- Rust全ワークスペース: 255件合格、8件ignore。`/tmp/mikomai-stages36-workspace-final5.log`。資料検索fixtureの不備も修正し、以前の失敗をskipしていない。
- Swift全テスト: 96件/25suite合格。`/tmp/mikomai-stages36-swift-last.log`。通常実行の別プロセス再読込み専用1件はskipだが、下記CLIの正本回復と跨言語ブローカー検証を別に実施。
- Swift/C#/CLI契約: 同じfixtureのTaskEvent順序・日本語結果・callback/queryが一致。`/tmp/mikomai-stages36-contracts-final2.log`、`/tmp/mikomai-test-contracts.dmFCSZ`。
- フェイク機器: 実CLI→Rust→同梱Netmiko→SSH/Telnet/PTY(serial)が合格。未承認送信0、承認の一回限り実行、前後read-back/差分、再利用拒否、C#がDBを保持した状態で複数CLIの変更直列化を確認。`/tmp/mikomai-stages36-devices-teardown.log`。fixture資格情報は検証後にKeychainから削除。
- 故障注入: 正本保存失敗時の送信0、承認トランザクションのrollback、待機期限後の明示resume、5枠解放、推論/ロック取消、ワーカークラッシュ後のUnknownと次要求再起動をRustテストで確認。
- 必須CLI: 実GGUF/E5のF220 VLAN質問、終了0、stdout全441行JSON、core_response.status 0。Access/TrunkのGigaEthernet/vlan-id/channel-groupと出典が資料に一致。別CLIプロセスの`task-query`で全イベントとpayloadを照合。`/tmp/mikomai-stages36-vlan-last.jsonl`、`/tmp/mikomai-stages36-verification-last.json`。モデルはMIKOMAI_MODEL_PATHで明示し、旧設定を移行していない。
- 最終変更後に決定的挨拶もchat --debug-jsonlで実行し、正常core_responseを確認。描画とアプリ再同梱の最終結果は末尾へ記録。

## 運用上の制約

- フェイク機器の検証は実機検証ではない。GUIの画面操作、実機固有の設定構文/rollback、公証と配布署名、リモートCIは別途検証する。設定のdry-runは静的検証であり、実機TAB補完による構文検証ではない。
- DBを保持するプロセスが終了した場合、既存クライアントは保存エラーを返す。自動で書込みを再試行しない。アプリ/CLIを再起動すると新しいプロセスがDBを開き、残ったExecuting承認をUnknownへ回復する。
- Schedulerの同時実行上限は現状5で固定。実行枠数の設定UI、独立したplatform/parser crateへの整理は後続。Windows/Vulkan/WinUIとWindows worker exeは未実装。
- Swift/C#/CLIの契約fixtureは同じ順序のTaskEvent、日本語結果、callback/query一致を検証する。macOS CIにも生成差分・契約・フェイク機器テストを追加したが、この作業ではローカル実行結果を検証証拠とする。

## 最終同梱確認

- `workers/device-worker/tests/bundled_helpers.py`で実バイナリのconfig_validate/config_convert/nwdiag_renderが合格。日本語SVGにはフォントを埋め込む。動的描画プラグインと依存パッケージのmetadataも同梱し、CIで検証する。
- `.app`を再構築し、実際のResources内ワーカーでも同じ3テストが合格。codesign --verify --deep --strictも合格。ログ: `/tmp/mikomai-stages36-app-final-verified.log`、`/tmp/mikomai-stages36-app-helpers.log`。
- C#のbin/objをソース管理対象から削除し、生成bindingsのソースと固定生成手順だけを保持する。

フェイク機器のPTY後処理は読取りスレッドを停止してからポートを閉じる。 macOSでcloseが停止する問題を修正した。
