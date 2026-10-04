# macOS のローカル NDP 取得

Mikomai は `get_state` の `resource: ndp`、対象 `localhost` で、macOS の IPv6 近隣キャッシュを `/usr/sbin/ndp -a` から取得します。登録済みのリモート機器の NDP 取得は未対応です。IPv4 の ARP キャッシュとは別の観測結果です。

アプリのチャットで次を入力します。

- `ndp -a`（FastRouter）
- `このMacのIPv6近隣キャッシュを見せてください`（Agent）
- `localhost のNDPテーブルを確認して`（Agent）

アプリでは Swift callback が `LocalNDPUtility.read()` を呼び、アプリの子プロセスとして実行します。`tool_response` の `command`、`exit_code`、`success`、`stdout`、`stderr` が実行の証拠です。権限・署名の確認には、CLI の成功に加えて実際のアプリの結果を確認してください。

CLI でも実行できます。

```sh
npm run --silent cli -- chat 'ndp -a' --debug-jsonl
```

CLI の `agent_event` にコマンド・終了コード・生出力が記録されます。CLI の自然文もローカルコマンドへ直接渡され、アプリの Agent/Swift callback は通りません。

取得に失敗した場合や stdout が空の場合は、成功した観測として回答しません。ヘッダーだけ取得できた場合は、キャッシュが空か、取得が制限されているかを判別できない旨を表示します。`(incomplete)` や状態・有効期限は生出力のまま保持します。読み取り時点のキャッシュなので、一覧にない機器がネットワークに存在しないことは証明しません。

## macOS 27 での実測

開発用 ad-hoc 署名の Mikomai アプリと CLI の内部で、`ndp -a` の終了コード 0、stderr 空、10行の取得を確認しました。ただし MAC は `2:0:0:0:0:0` に置き換わり、取得できた行は自機の情報が中心でした。ターミナル直実行では他の近隣と実 MAC も返ったため、内部からのコマンド実行成功と、完全な近隣情報の取得成功は分けて扱います。マスク値を検出した場合はその制限を回答に表示します。

署名・権限の影響が疑われますが、NDP に対して正規署名と権限を付けた再検証は未実施です。Apple DTS は macOS 27 の取得情報のスクラブと Network Topology Observation capability を説明しています: [Apple Developer Forums](https://developer.apple.com/forums/thread/822025?page=2)。署名方法は [macos-arp.md](macos-arp.md) を参照してください。

## 動作確認（2026-10-04）

- core の対象テスト: ローカル対象・単一依頼の限定、空/ヘッダー/マスク値の扱い、Agent の観測1回と失敗時の判定が合格。
- registry の対象テスト: 未登録の localhost は取得可能。リモート対象や対象と引数の食い違いは transport の呼び出し前に拒否。
- CLI: コマンド入力と自然文の両方で、全 stdout 行を JSON として解析し、内部コマンド、終了コード 0、生出力、最終 `core_response.payload.status == 0`、マスクの説明を確認。ログは `/private/tmp/mikomai-ndp-cli.jsonl` と `/private/tmp/mikomai-ndp-cli-natural.jsonl`、stderr はそれぞれ同名の `.stderr`。
- アプリ: 最終ビルドを起動し、`ndp -a` の FastRouter と自然文の Agent の両方で実測。Agent は資料検索なしで実行し、依頼ごとに Swift callback の呼び出し1回、終了コード 0、10行の生出力、最終応答への一致を確認。30行の JSONL を全行解析済み。保存先は `/private/tmp/mikomai-ndp-app.jsonl`。
- 基本質問 `F220のVLAN設定方法を教えて` も最終変更後の `chat --debug-jsonl` で合格。438行を全行解析し、local_model の資料取得・LLM要求/応答・終了コード 0・最終 status 0 と、FITELnet 資料の VLAN コマンドとの整合を確認。ログは `/private/tmp/mikomai-ndp-cli-basic-final.jsonl`、stderr は `/private/tmp/mikomai-ndp-cli-basic-final.stderr`。
- `build-app.sh` のビルドと ad-hoc 署名検証は成功。
- 完全な近隣一覧・実 MAC の取得、正規署名と Network Topology Observation capability による取得は未検証。
