# macOS 27 の ARP 取得と署名

macOS 27 は、Network Topology Observation capability を持たないプロセスに IPv4 ARP キャッシュを空で返すことがあります。`arp -an` の終了コードが 0 でも、空の stdout は「対象 MAC が存在しない」証拠にはなりません。Mikomai は空の生出力では有無を判定できない旨を回答します。非空の観測結果は MAC と IP を照合して判定します。

今回の再現では、ターミナルからの `arp -an` は 13 行、CLI とアプリの子プロセスからは 0 行でした。ユーザーの観測結果には `ea:f1:92:50:7b:c3` / `192.168.50.27` が含まれていました。

Apple の説明: https://developer.apple.com/forums/thread/822025?page=2

## アプリの署名

Apple Developer の App ID に Network Topology Observation capability を設定し、その権限を許可する署名証明書・プロビジョニングプロファイルを用意してください。権限を ad-hoc 署名に追加するだけでは、macOS が起動を拒否します。

`mikomai-desktop-mac/build-app.sh` は通常の開発用 ad-hoc ビルドを維持します。正規署名では以下の変数を設定します。

```sh
MIKOMAI_CODESIGN_IDENTITY='Developer ID Application: Your Name (TEAMID)' \
MIKOMAI_PROVISIONING_PROFILE='/absolute/path/profile.provisionprofile' \
bash mikomai-desktop-mac/build-app.sh
```

`NetworkTopology.entitlements` をアプリの署名に指定し、プロファイル指定時はバンドルへ格納します。署名者名は実際の証明書に置き換えてください。署名後はアプリを再起動して、JSONL の `tool_response.stdout` に実際の ARP 行があることを確認します。

## CLI の署名

`npm run cli` は Cargo 経由なので、再ビルドで署名が置き換わります。権限のある署名を検証する場合はビルド後に CLI 自体を署名してから直接起動します。

```sh
cargo build -p mikomai-cli
codesign --force --sign 'Developer ID Application: Your Name (TEAMID)' \
  --entitlements mikomai-desktop-mac/NetworkTopology.entitlements target/debug/mikomai-cli
target/debug/mikomai-cli chat 'localhost のARPテーブルにea:f1:92:50:7b:c3は存在する？' --debug-jsonl
```

正規署名がない環境の CLI 検証で空の取得結果になった場合、実機の MAC 有無判定は未検証です。パーサー試験の成功と実機取得の成功は別々に扱います。
