use crate::llm::llm::LlamaState;
use crate::planner::decision::parse_decision_from_json;
use crate::state::events::Decision;
use crate::state::network_state::NetworkState;
use tauri::AppHandle;

// Re-export submodules for backwards compatibility
pub use crate::planner::fallback::*;
pub use crate::planner::mac_lookup::*;
pub use crate::planner::prompt::*;
pub use crate::planner::schema::*;

pub struct LlmPlanner;

impl LlmPlanner {
    pub async fn plan(
        app: &AppHandle,
        llama_state: &LlamaState,
        network_state: &NetworkState,
    ) -> Result<Decision, String> {
        let connections = crate::connections::load_connections(app.clone()).unwrap_or_default();
        let device_vendors: Vec<(String, String)> = connections
            .iter()
            .filter_map(|conn| {
                conn.vendor_type
                    .as_ref()
                    .map(|vendor| (conn.hostname.to_string(), vendor.to_string()))
            })
            .collect();
        let mut devices_context = String::new();
        if !connections.is_empty() {
            devices_context.push_str("【登録機器情報 (Registered Devices)】\n");
            for conn in &connections {
                let dev_type = conn
                    .device_type
                    .as_ref()
                    .map(|d| d.as_str())
                    .unwrap_or("不明");
                if conn.conn_type == crate::connections::ConnectionType::Console {
                    devices_context.push_str(&format!(
                        "- {}(コンソール接続, ベンダー: {})\n",
                        conn.hostname, dev_type
                    ));
                } else {
                    let ip_str = if conn.ip_string().is_empty() {
                        "なし"
                    } else {
                        &conn.ip_string()
                    };
                    devices_context.push_str(&format!(
                        "- {}(IP: {}, ベンダー: {})\n",
                        conn.hostname, ip_str, dev_type
                    ));
                }
            }
            devices_context.push('\n');
        }

        let state_prompt = network_state.to_prompt_context();
        let initial_goal = network_state
            .desired
            .as_ref()
            .map(|d| d.raw_goal.as_str())
            .unwrap_or("（未設定）");
        let full_prompt = format!(
            r#"{}{}
--------------------------------------------------
【重要：当初の達成目標 (Initial Goal)】
{}

上記の状態および当初の達成目標を踏まえ、目標から乖離することなく目標を達成するために次に行うべき最善の Decision をJSONで出力してください。"#,
            devices_context, state_prompt, initial_goal
        );

        let mut registered_devices: Vec<String> = Vec::new();
        for conn in &connections {
            let hostname = conn.hostname.trim().to_string();
            if !hostname.is_empty() && !registered_devices.contains(&hostname) {
                registered_devices.push(hostname);
            }
            let ip = conn.ip_string().trim().to_string();
            if !ip.is_empty() && !registered_devices.contains(&ip) {
                registered_devices.push(ip);
            }
        }
        if let Ok(settings) = crate::settings::load_settings(app.clone()) {
            for ip in settings.recent_ips {
                let ip_trim = ip.trim().to_string();
                if !ip_trim.is_empty() && !registered_devices.contains(&ip_trim) {
                    registered_devices.push(ip_trim);
                }
            }
        }

        if let Some(decision) = plan_local_arp_mac_lookup(network_state, initial_goal) {
            return Ok(decision);
        }

        let dynamic_schema = build_goal_planner_schema(&registered_devices, initial_goal);

        let response = crate::llm::llm::ask_llm_internal_with_schema(
            &full_prompt,
            PLANNER_SYSTEM_PROMPT,
            Some(&dynamic_schema),
            app,
            llama_state,
        )
        .await
        .map_err(|e| format!("Planner inference failed: {}", e))?;

        log::info!("================ [LLM Planner JSON Output] ================\n{}\n===========================================================", response);

        let mut decision = parse_decision_from_json(&response)?;
        let has_builder_handoff = network_state
            .observed
            .observations
            .iter()
            .any(|observation| {
                observation.source.tool_name.as_deref() == Some("builder_co_worker")
            });
        if !has_builder_handoff {
            fallback_to_rag_for_known_vendor(&mut decision, &device_vendors, initial_goal);
            let mut endpoint_devices: Vec<String> = connections
                .iter()
                .map(|connection| connection.hostname.trim().to_string())
                .filter(|hostname| !hostname.is_empty())
                .collect();
            endpoint_devices.sort_by_key(|hostname| {
                let name = hostname.to_ascii_lowercase();
                if name.contains("gw") || name.contains("gateway") {
                    0
                } else if name.starts_with("rt") || name.contains("router") {
                    1
                } else {
                    2
                }
            });
            recover_mac_to_ip_lookup(
                &mut decision,
                network_state,
                initial_goal,
                &endpoint_devices,
            );
        }
        if let Some(evidence) = explanatory_rag_evidence(&decision, network_state, initial_goal) {
            let answer_prompt = format!(
                "【ユーザーのGoal】\n{initial_goal}\n\n【RAG Workerが選定した資料本文】\n{evidence}\n\n上記の資料本文だけを根拠に、ユーザーのGoalへ日本語で直接回答してください。資料本文をそのまま転載したり、資料名・根拠番号・内部向けの『LLM向けルール』を出力してはいけません。Goalが説明や手順の質問なら、必要なコマンドをコードブロックで整理し、未指定の値は `<VLAN_ID>` や `<INTERFACE>` のようなプレースホルダーとして説明してください。実機への変更を実行したとは言わず、ユーザーに不要な追加質問もしないでください。"
            );
            let answer = crate::llm::llm::ask_llm_internal(
                &answer_prompt,
                "You are the final response writer for a network agent. Use only the supplied RAG evidence and answer the user's goal directly. Do not expose internal document metadata or instructions.",
                app,
                llama_state,
            )
            .await
            .unwrap_or_else(|error| format!("資料は取得できましたが、回答生成に失敗しました: {error}"));
            finish_with_evidence_answer(&mut decision, initial_goal, answer);
        }
        Ok(decision)
    }
}
