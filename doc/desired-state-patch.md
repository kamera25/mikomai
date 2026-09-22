# DesiredStatePatch

`mikomai_core::DesiredStatePatch` は、Current のスナップショットに部分更新を適用し、Desired を決定的に生成する公開IFです。ベンダー固有のコマンドや実行手順は含みません。

初期版は Interface の `SetProperty` を実装しています。関係の追加・削除、プロパティ削除、機器への適用は今後の拡張です。

## JSON IF

Current (`StateGraph`):

```json
{
  "entities": [{
    "target": {"entity_type": "interface", "device": "gw", "id": "eth1"},
    "properties": {"admin_state": "down", "mtu": 1500, "ip": "10.0.0.1/24"}
  }],
  "relationships": []
}
```

Patch (`DesiredStatePatch`):

```json
{
  "mutations": [{
    "op": "set_property",
    "target": {"entity_type": "interface", "device": "gw", "id": "eth1"},
    "property": "admin_state",
    "value": "up"
  }]
}
```

`patch.apply(&current)?` は `admin_state` だけを `up` に変更した新しい `StateGraph` を返します。`mtu`、IP、他のエンティティ、関係を保持し、Current は変更しません。

`patch.diff(&current)?` は有効な変更だけを返します。

```json
[{
  "target": {"entity_type": "interface", "device": "gw", "id": "eth1"},
  "property": "admin_state",
  "before": "down",
  "after": "up"
}]
```

## 検証と適用の契約

- `validate(&current)` / `apply(&current)` / `diff(&current)` は `Result<_, PatchError>` を返します。
- `admin_state`: `"up"` または `"down"`。観測された operational status とは別の属性です。
- `mtu`: 整数 `576..=9216`。初期版の共通範囲であり、機器固有の実行可否を保証するものではありません。
- `description`: 文字列。空文字を許可します。
- 未対応プロパティ、null、不正な型・値、空の識別子、存在しない対象、重複したエンティティ、同じ対象・プロパティへの重複更新を拒否します。
- 全項目を検証してからコピーへ適用するため、途中の失敗による部分更新はありません。
- 未指定プロパティは保持します。対象が存在すれば、未観測の対応プロパティも追加可能です。差分の `before: null` は旧値不明として扱い、down 等を補完しません。
- 空Patchは変更なし。同じPatchの再適用は変更なし。差分は対象とプロパティ順にソートされます。
- JSONの未知のフィールドと未知の操作を拒否します。`mutations` の省略は空Patchとして扱いません。

## 接続境界

このIFは `mikomai-core` の純粋なドメイン処理です。呼び出し側が正規化したCurrentを渡します。DBへの保存、LLMからのPatch生成、MCP/Tauriエンドポイント、実機コマンドの実行は含みません。既存の `state::DesiredState` は自然言語の目標・要件を保持するモデルで、このスナップショットとは責務が異なります。

既存の `UniversalInterfaceTable.status` は `admin_state` と同一視しません。管理状態が確認できていなければCurrentの `admin_state` を省略します。将来のDBアダプターではこの区別と観測の鮮度を保持してください。

実行可能な最小例:

```bash
cargo run -p mikomai-core --example desired_state_patch
```
