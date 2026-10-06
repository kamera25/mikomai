# 段階3の業務ルール移動・旧取込廃止

2026-10-06時点の記録。現在の段階3〜6の状態は[統合実装・検証記録](mikomai-app-stage3-6.md)を参照。

2026-10-06。再設計書の未実施事項を照合し、段階3を進めた。

## 実装した範囲

- Swiftのセッション正規化・作成・選択・削除・改名をRust appのポリシーへ移動。SwiftはDTOと表示状態を保ち、Rustの結果を反映する。
- 接続情報の検証、既定ポート、Telnetドライバー解決、接続種別変更、登録/編集/削除、資格情報の有無の判定をRustへ移動。秘密値は新しいポリシーAPIへ渡さない。
- 機器種別/aliasとモデルプリセットの正本をRustの同梱JSONへ集約。Swiftは表示用DTOと検索/ラベルだけを持つ。
- dry-run判定をRustへ移動。プロセス失敗、不正JSON、空のresults、1件でもok=falseなら投入不可という境界を維持。
- SwiftUIとSwift CoreのCSV/JSON接続情報取込を廃止。CSV入出力のUIも削除し、1件ずつ登録・編集する形にした。
- 旧設定の自動取込、旧Watchのコピー、旧Agentイベント形式の変換、旧graph保存先へのフォールバックを廃止。CLIも旧設定ディレクトリを自動探索しない。
- 外部connections.jsonからの補完候補読込みを廃止し、登録済みの接続情報から候補を表示。
- 旧ディレクトリは検出・案内だけを行い、ファイルを削除しない。設定画面の状態メッセージに案内を表示。
- `doc/greenfield_redesign.md` をリポジトリへ取り込み、旧計画§12–13を履歴へ退避。`doc/refactoring-plan.md` の現行方針と実施状況を更新。

既存C ABIに一時的な `mikomai_native_query` を追加し、Swift CoreからRustポリシーへ接続した。手書きABI全体の廃止は段階6のUniFFI移行で行う。

## 未完了

sessions/connections/settingsは[保存サービス実装](mikomai-app-stage3-storage.md)で追加し、その後、残っていた正本集約・Rust資格情報/操作/プロセス管理・ワーカー・排他/推論キュー・UniFFI・TaskEvent・共通契約試験を[段階3〜6](mikomai-app-stage3-6.md)で実装した。Windows/Vulkan/WinUI（段階7〜8）は現在の対象外。以下の検証欄は段階3初回実装当時の記録。

## 検証

事前の条件は、移したポリシーをSwiftの実際の呼出しで通すこと、無効接続の拒否、削除後も有効セッションを保つこと、dry-run失敗時にconfigureを呼ばないこと、旧Agent履歴の拒否と現行履歴の再開、必須CLIの完結した資料付き回答。

- Rust/app 50件、CLI 6件、FFI境界2件が合格。実モデル専用5件はignore。段階1で変更前でも再現した `c_abi_reports_errors_answers` のfixture失敗1件は今回も分離した。
- 新しいFFI境界テストでJSON scalarの返却、null入力のエラー、廃止したimport要求の拒否を確認。
- 指定SDKでのmacOSアプリビルドとSwift Coreテスト95件が合格。会話セッション、接続ポート/種別・登録/資格情報flag、全機器カタログ、モデルプリセット、dry-run失敗時の送信0回を既存テストで確認。
- 通常の `swift test` は新しいSDKの `SwiftUIMacros.StateMacro` が見つからず失敗。プロジェクト指定の `test-core.sh` / MacOSX26.5 SDKで検証し直した。
- F220の基本質問を実GGUF/E5と一時DBで `--debug-jsonl` 実行。stdout全行をJSONとして解析し、reference_context、llm_request/response、最後のcore_response.status 0を確認。Access/Trunk VLANのGigaEthernet・vlan-id・channel-groupと出典を元資料に照合。
- Swift画面の手動操作、実機への送信、Windowsは未検証。OSキー保管は現行Swift実装のままで、Rust keyring移行の成功とは扱わない。

ログ:

- `/tmp/mikomai-stage3-build.log`
- `/tmp/mikomai-stage3-rust.log`
- `/tmp/mikomai-stage3-bridge.log`
- `/tmp/mikomai-stage3-swift-final.log`
- `/tmp/mikomai-stage3-vlan-final.jsonl`
- `/tmp/mikomai-stage3-vlan-final.stderr`
