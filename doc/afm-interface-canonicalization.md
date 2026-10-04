# AFMでのインターフェースCanonical化

インターフェースの`get_state`は選択中のモデルを使う。AFM選択中はGGUFをロードせず、Foundation Modelsの構造化生成で選択データを取得する。GGUF選択中は従来のGBNF制約を使う。Coreが生成する共通出力契約に、型スキーマとGBNFを格納する。

AFMにも候補インデックスの数値範囲と`up / down / unknown`の列挙を制約する。生成結果は両バックエンドで同じCore検証に通す。候補と出典との関係、全ブロック・IPの網羅性、IPに結び付くprefixの保持、機器名、重複を検証し、不正な選択は最大3回まで修正を要求する。元の行番号と選択用ブロックインデックスは明示的に区別する。

`prefix_len`は必須の数値/null型で生成する。単一のIP prefixが観測されている場合は、その値を省略できない。インターフェース名中の`/1`などをIP prefixの証拠として扱わない。

この環境の実モデル比較では、ネストした配列にminimum=0等の件数ガイドを付けるとFoundation Modelsの内部エラーが発生したため、配列件数は共通コードで検証する。数値範囲と状態列挙の制約は実モデルで確認した。AFMブリッジは型・配列・オブジェクト・列挙・数値範囲・nullableの対応範囲を実装し、未対応の型はエラーにする。nullable生成にはmacOS 26.4以降のAPIを使う。MikomaiのAFM 3 Core選択はmacOS 27以降を対象とする。

通常Agentの計画は、型不正を実行前に検出して一度だけ再計画する。既知の読み取り`get_state`が明示的なtool/target/resourceを別の階層に置いた場合は、既存の正規化方式に従って標準の位置へ移し、許可リストと対象機器の検証を通す。インターフェース観測の同一操作判定では、登録名・ID・IP、冗長なdevice引数、インターフェース名の大文字小文字を正規化し、取得済みの操作の繰り返しを止める。

## 検証方法

```sh
cargo test -p mikomai-core
cargo test --manifest-path crates/mikomai-llm-apple/Cargo.toml
cargo test -p mikomai-ffi apple_canonicalizes_interface_fixtures_with_common_validation -- --ignored --nocapture
cargo test -p mikomai-ffi interface_executor_canonicalizes_with_llm_and_reads_saved_graph_state
```

AFMの実機経路は次で確認する。保存済みの接続・Keychainを読み取り、観測の保存先は一時領域に隔離する。GGUFモデル指定は不要。変更計画が出た場合は失敗する。

```sh
MIKOMAI_EXECUTION_CHECK_SOURCE="$PWD/mikomai-desktop-mac/Tests/InterfaceChecks/InterfaceChecks.swift" \
MIKOMAI_INTERFACE_CHECK_TARGET=NakaokuGW \
MIKOMAI_INTERFACE_CHECK_LAN=LAN2 \
MIKOMAI_INTERFACE_CHECK_BACKEND=apple \
./mikomai-desktop-mac/test-execution-queue.sh
```

`MIKOMAI_INTERFACE_CHECK_PROMPT`を指定して通常Agentの自然文経路も確認する。

## 対応範囲

今回のAFM対応はインターフェースCanonical化。ARPなど、GBNFだけを渡す既存のCanonical化はAFM用の型スキーマがまだなく、AFM選択時は未対応を返す。選択と異なるGGUFへ自動的に切り替えない。

実機検証はヤマハNakaokuGWのLAN2。Cisco/Junosは表示例のAFM実モデル検証であり、各社実機の確認は含まない。設定変更や全体回帰テストの成功を示すものではない。

参考: [AppleのDynamicGenerationSchema](https://developer.apple.com/documentation/foundationmodels/dynamicgenerationschema)

## 2026-10-04の検証結果

| 項目 | 結果 |
| --- | --- |
| Core | 136件成功。出力契約、prefix保持、計画の正規化・型不正の再計画、同一操作判定、単独自然文の観測後完了・複合依頼の除外 |
| Appleブリッジ | 通常のunitテスト4件成功。モデル会話履歴テスト1件はignore。今回のモデル動作は下記の実モデル検証で別に確認 |
| FFI対象テスト | Canonical/Graph保存・再取得・キャッシュ・失敗時の古い状態不使用を検証して成功 |
| AFM表示例 | 実モデルでヤマハup/down、Cisco up、Junos形式unknownが共通検証を通過 |
| AFM実機の定型文 | GGUF未ロードの独立プロセスでNakaokuGW/LAN2を取得。prefix=30/status=up、Graph再取得、最終status=0、終了コード0 |
| AFM実機の自然文 | 通常Agent/AFMが最初のget_state・機器・IF・refresh=trueを選択。取得1回、AFMでCanonical化、Graph再取得後に共通コードでup判定。最終status=0、終了コード0。観測時刻15:46:38 JST |
| GGUF実機比較 | 同じLAN2でGBNF経路を使用。prefix=30/status=up、Graph再取得、最終status=0、終了コード0 |
| 必須CLI JSONL | 基本質問437行、機能質問4行をすべてJSONとして解析。どちらも終了コード0・最終status=0。基本回答はF220のAccess/Trunk資料と整合。機能回答は接続機能のないCLIからアプリへ案内し、up/downを推測しない |
| アプリ | 最終版のビルド・署名検証成功 |

単独の「機器でIFのリンク状態を観測/確認してください」という自然文は、最初の操作を通常Plannerに選択させ、必要なGraph観測が取得された後は共通コードが完了を判定する。AFMのFINISH選択に依存せず、同じ機器を再取得しない。対象・IFが違う観測や、後続の操作を含む複合依頼を、この完了処理で完了扱いにしない。

CLIにはSwiftの実機接続callbackがないため、AFM実機検証の代用にはしていない。基本質問のCLIは既存設定のGGUF経路を通る。RAG/Graphを一時領域に隔離し、既存のユーザー索引を削除・変更していない。

使用するにはビルド済みの `mikomai-desktop-mac/dist/Mikomai.app` を再起動してAFM 3 Coreを選択し、`NakaokuGWのLAN2がupしているか確認して`と入力する。
