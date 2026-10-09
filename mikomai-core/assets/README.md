# mikomai-core assets (共有ランタイムアセット)

ネットワーク機器連携、設定変換、トポロジ図描画などに使用する Python ヘルパーやテンプレート、フォントなどのランタイムアセット群です。

## ディレクトリ構成

```text
assets/
├── bin/                 # 配布用スタンドアロンバイナリ (PyInstaller 生成物)
├── fonts/               # 構成図レンダリング用の日本語フォント
├── network/             # Python 製ネットワーク支援スクリプト群
│   ├── config_helper.py    # Cisco 設定の構文検証およびベンダー間変換
│   ├── netmiko_wrapper.py  # SSH / Telnet / Serial 経由の機器操作ワーカー
│   ├── nwdiag_wrapper.py   # nwdiag による SVG ネットワーク構成図生成
│   └── netmiko_patches.py  # Telnet/暗号化等の互換性パッチ
├── templates/           # 設定変換用 Jinja2 テンプレート (Juniper, Arista 等)
└── system_prompt.txt    # LLM 推論用の標準システムプロンプト
```

---

## 主要スクリプトの仕様と利用方法

### 1. `network/config_helper.py` (設定の検証と変換)

標準入力から JSON を受け取り、標準出力に結果 JSON を返します。

#### 構文検証 (`validate`)
```bash
python3 mikomai-core/assets/network/config_helper.py << 'EOF'
{
  "action": "validate",
  "config": "hostname RouterA\ninterface GigabitEthernet0/0\n ip address 192.168.1.1 255.255.255.0"
}
EOF
```

#### ベンダー間変換 (`convert`)
`templates/` 配下の Jinja2 テンプレートを参照して、Cisco 形式の設定を Juniper や Arista 形式へ変換します。
```bash
python3 mikomai-core/assets/network/config_helper.py << 'EOF'
{
  "action": "convert",
  "target_vendor": "juniper",
  "config": "hostname RouterA\ninterface GigabitEthernet0/0\n ip address 192.168.1.1 255.255.255.0"
}
EOF
```

---

### 2. `network/nwdiag_wrapper.py` (構成図の描画)

nwdiag DSL から SVG ネットワーク構成図を生成します。

```bash
python3 mikomai-core/assets/network/nwdiag_wrapper.py -T svg -o network.svg input.diag
```

- **依存関係**: Python 環境に `nwdiag` および `Pillow` がインストールされている必要があります。
- **フォント**: `fonts/` ディレクトリ内の日本語フォントを自動検出し、文字化けを防ぎます。

---

### 3. `network/netmiko_wrapper.py` (機器接続ワーカー)

Netmiko を利用して SSH、Telnet、シリアルポート経由で機器を操作するワーカーです。標準入出力を通じて Rust 側と 1 行ずつの JSON プロトコルで通信します。

- 資格情報は引数や環境変数ではなく、標準入力の要求本文からのみ受け取り、機密漏洩を防ぎます。
- macOS 向けには `bin/netmiko_wrapper-macos-arm64` として PyInstaller 単一バイナリが同梱されており、ユーザー環境への Python 導入なしでも動作可能です。
