//! Chat -> typed patch -> observed graph -> approved operation plan.
use mikomai_core::desired_change::{
    graph_from_interfaces, patch_device, prepare_change, proposal_schema, PatchProposal,
    PATCH_SYSTEM_PROMPT,
};
use tauri::{Emitter, Manager};

pub fn handles(message: &str) -> bool {
    let text = message.to_lowercase();
    if [
        "教えて",
        "方法",
        "設定例",
        "サンプル",
        "とは",
        "how to",
        "example",
    ]
    .iter()
    .any(|s| text.contains(s))
    {
        return false;
    }
    let property = [
        "mtu",
        "admin_state",
        "description",
        "説明文",
        "インターフェース",
        "interface",
        "shutdown",
        "upに",
        "downに",
        "up に",
        "down に",
    ]
    .iter()
    .any(|s| text.contains(s));
    property
        && (crate::harness::intent::is_configuration_change_request(&text)
            || [
                "変更して",
                "にして",
                "有効化して",
                "無効化して",
                "set ",
                "change ",
                "enable ",
                "disable ",
            ]
            .iter()
            .any(|s| text.contains(s)))
}

pub async fn prepare(
    app: &tauri::AppHandle,
    llama: &crate::llm::llm::LlamaState,
    message: &str,
) -> Result<String, String> {
    let connections = crate::connections::load_connections_raw(app).map_err(|e| e.to_string())?;
    let devices: Vec<String> = connections.iter().map(|c| c.hostname.to_string()).collect();
    if devices.is_empty() {
        return Ok("変更対象の機器を先に登録してください。".into());
    }
    let prompt =
        serde_json::json!({"registered_devices":devices,"user_request":message}).to_string();
    let raw = crate::llm::llm::ask_llm_internal_with_schema(
        &prompt,
        PATCH_SYSTEM_PROMPT,
        Some(&proposal_schema(&devices)),
        app,
        llama,
    )
    .await
    .map_err(|e| e.to_string())?;
    let proposal: PatchProposal =
        serde_json::from_str(&raw).map_err(|e| format!("変更計画のJSONが不正です: {e}"))?;
    let patch = match (proposal.patch, proposal.clarification) {
        (Some(patch), None) => patch,
        (None, Some(question)) if !question.trim().is_empty() => {
            return Ok(format!("### ❓ 確認要求\n{question}"))
        }
        _ => return Err("変更内容または確認事項を一意に指定できませんでした".into()),
    };
    let device = patch_device(&patch)?.to_string();
    let connection = connections
        .iter()
        .find(|c| c.hostname.as_str() == device)
        .ok_or("登録機器と変更対象が一致しません")?;
    // Never use the legacy resolver's default platform for command generation.
    let platform = connection
        .device_type
        .as_ref()
        .map(|v| v.as_str())
        .ok_or("機器の device_type を明示してください")?;
    if !matches!(platform, "cisco_ios" | "cisco_xe") {
        return Err(format!("DesiredStatePatch の未対応機種です: {platform}"));
    }
    let graph = app.state::<crate::graph::SurrealDbState>();
    let canonical = match graph.current_interfaces_for_change(&device).await? {
        Some(value) => value,
        None => {
            let window = app
                .get_window("main")
                .ok_or("チャットウィンドウがありません")?;
            let result = crate::mcp::executor::execute_mcp_tool_raw(
                app.clone(),
                window,
                uuid::Uuid::new_v4(),
                "get_state".into(),
                message.into(),
                serde_json::json!({"device":device,"resource":"interfaces"}),
                vec![],
                30,
            )
            .await?;
            if !result.success {
                return Err(format!("変更前の観測に失敗しました: {}", result.output));
            }
            graph
                .current_interfaces_for_change(&device)
                .await?
                .ok_or("新しいインターフェース観測を取得できませんでした")?
        }
    };
    let observed = graph
        .query_network(crate::graph::GraphQuery {
            query: String::new(),
            device_name: Some(device.clone()),
            ip_address: None,
            vlan: None,
            acl: None,
        })
        .await?;
    let current = graph_from_interfaces(&device, &canonical, observed.relationships)?;
    let prepared = prepare_change(&current, &patch, platform)?;
    if prepared.commands.is_empty() {
        return Ok("観測済みの設定値と要求が一致しているため、実行する差分はありません。".into());
    }
    if crate::llm::llm::is_cancelled() {
        return Err("変更計画の作成を中断しました".into());
    }
    let plan = crate::operations::ChangePlanner::create(
        "network_config".into(),
        Some(device.clone()),
        serde_json::json!({
            "deviceName":device,"commands":prepared.commands,"desiredStatePatch":patch,
            "currentGraph":current,"desiredGraph":prepared.desired,"propertyChanges":prepared.changes
        }),
        message.into(),
    )?;
    let plan = app
        .state::<crate::operations::OperationStore>()
        .insert(plan)?;
    app.emit("request-diff-commit", serde_json::json!({"id":plan.id,"hostname":device,
        "fileName":"desired-state.conf","config":prepared.commands.join("\n"),"operationPlan":plan}))
        .map_err(|e| e.to_string())?;
    Ok(format!("### ✅ 承認待ち\n{} の変更計画を作成しました。差分パネルでコマンドを確認して承認してください。まだ機器へ適用していません。\n\n設定値の差分（null は変更前の値が未観測）:\n```json\n{}\n```", device, serde_json::to_string_pretty(&prepared.changes).map_err(|e| e.to_string())?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_changes_but_not_explanations() {
        for message in [
            "gw の GigabitEthernet1/0/1 を up にして",
            "gw の interface の MTU を9000に変更して",
            "set gw interface description uplink",
        ] {
            assert!(handles(message), "{message}");
        }
        for message in [
            "F220のVLAN設定方法を教えて",
            "interfaceをupにしてよいか教えて",
            "gwのインターフェース状態を確認して",
        ] {
            assert!(!handles(message), "{message}");
        }
    }
}
