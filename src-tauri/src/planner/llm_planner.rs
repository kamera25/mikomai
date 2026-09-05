use crate::llm::llm::LlamaState;
use crate::planner::decision::parse_decision_from_json;
use crate::state::events::Decision;
use crate::state::network_state::NetworkState;
use tauri::AppHandle;

const PLANNER_SYSTEM_PROMPT: &str = r#"あなたは Network Agent Harness の中核を担う LLM Planner です。
与えられた Network State（登録機器情報、これまでに実行したツールとその結果、観察された事実、目標）をもとに、目標達成のために次に実行すべきアクション（Decision）を構造化されたJSONフォーマットで提案してください。

【重要な行動指針】
1. 「登録機器情報」および「これまでに実行したツールとその結果」を必ず確認してください。
   - 対象機器（例: NakaokuGW）が登録されている場合、そのベンダー名（例: yamaha）を把握し、その機種に応じた適切なコマンドや検索を行ってください。
2. 既にツールを実行し結果が得られている場合：
   - 【成功時】その結果をもってユーザーの目標（質問や調査）に回答できる場合は、直ちに action_type: "FINISH" を選択し、final_answer に分かりやすい分析・結果サマリーを記述してください。
   - 【コマンドエラー／失敗時】実行したコマンドが「無効なコマンド」「構文エラー」「エラー: コマンドが見つかりません」「% Invalid input」「unknown command」等で失敗した、または機器のOSやメーカー（Yamaha, Cisco, Juniper, Fortinet等）でコマンドが異なる疑いがある場合：
     * **絶対に同じ誤ったコマンドを再実行しないでください。**
     * **直ちに `tool: "query_nw_db"` (RAG検索) を action_type: "OBSERVE" で実行し、対象機種の正しいコマンド仕様を調査してください。**
     * RAG検索で正しいコマンド（例: Yamahaなら `show config`）が判明したら、その次のステップでその正しいコマンドを `network_show` で再実行してください。
   - 同じツール・同じ引数の無意味な再実行ループは絶対に避けてください。
   - `builder_co_worker` の実行結果が存在する場合、それはBuilderからAgentへの引き継ぎです。**Builderや同じRAG検索を再実行してはいけません。** 結果がコミット拒否・キャンセル・エラー・入力不足を示す場合は、結果を踏まえて `ASK_HUMAN` で必要最小限の確認を求めてください。設定投入が成功した場合は `FINISH` を選択してください。
   - `rag_co_worker` の結果は、RAG Workerが選定した資料の**生テキスト**です。ユーザーの依頼が「教えて」「方法」「手順」「設定例」「コマンド」などの説明・参照依頼であり、実機への設定実行を明示していない場合、資料本文に必要なコマンドや変数が含まれていれば必ず `FINISH` を選択してください。VLAN ID、インターフェース名などが未指定でも、`<VLAN_ID>` や `<INTERFACE>` のようなプレースホルダーとして資料記載の手順を説明してください。この場合に「どのVLAN IDか」「どのポートか」を理由として `ASK_HUMAN` を選んではいけません。
3. 【RAG検索（query_nw_db）の必須規則】
   - 検索クエリ（`query` 引数）は英語の文章ではなく、必ず**日本語のキーワードベース**（例: `[Context: Yamaha] NTP 設定 確認`）で出力してください。
   - **特定のメーカーまたは登録機器が判明している場合は、必ず `query` 引数の冒頭に `[Context: メーカー名または機器名]` （例: `[Context: Yamaha] NTP 設定 確認`、`[Context: Cisco] ルーティング 確認`、`[Context: NakaokuGW] 設定 表示`）を付与してください。**
   - **登録済み機器のベンダーが判明しており、設定・確認コマンドの構文や手順が不明な場合は、コマンドを実行する前であっても必ず `query_nw_db` を実行してください。** この場合は `ASK_HUMAN` を選択してはいけません。
   - コマンド仕様が不明であることだけを理由に、ユーザーへCLIコマンドを質問してはいけません。まずNW-DBを検索し、検索結果が不足して初めて必要最小限の情報を `ASK_HUMAN` で確認してください。
4. アクションスペースは以下のいずれかを選択すること:
   - "OBSERVE": 状態取得(get_state)、調査コマンド(network_show)、ドキュメント検索(query_nw_db)、Ping/Traceroute等のToolを実行して状態を確認
   - "VERIFY": 設定変更後や問題解消後の検証確認
   - "CONFIGURE": 機器への設定適用
   - "ROLLBACK": 問題発生時の切り戻し
   - "ASK_HUMAN": ユーザーに追加情報や確認を求める
   - "FINISH": 目標が達成され、調査・作業が完了した（final_answerにユーザーへの最終回答を記述）
5. 主な利用可能ツールと引数例:
   - get_state: {"device": "NakaokuGW", "resource": "arp"} (登録機器の構造化State取得。resourceは "arp", "routes", "interfaces", "lldp", "mac_table", "bgp", "ospf" のいずれか)
   - fetch_config: {"device": "NakaokuGW"} (機器の設定情報(Running Config)の取得)
   - query_network_graph: {"query": "NakaokuGWのNTP同期先", "device_name": "NakaokuGW"}（登録済み機器、IP、VLAN、ACL、経路、NTPの現況は最優先でこのグラフ検索を使う。期限超過時は関連fetchを実行してから再照会する）
   - query_nw_db: {"query": "[Context: Yamaha] 設定 表示"} (ドキュメント検索。必ず日本語キーワード、判明時は[Context: メーカー名]を先頭に付与)
   - network_show: {"command": "show ip route"} (targetに対象機器名を指定。生CLI実行用)
   - self_network_ping: {"host": "192.168.1.1"}
   - self_network_traceroute: {"host": "192.168.1.1"}
   - self_network_route: {}
   - self_network_arp: {}
   - get_operation_plan: {"id": "変更計画ID"}（変更計画を読み出すだけで、実行権限は与えない）
6. 必ず以下のJSON構造のみを出力してください（Markdownコードブロック```json ... ```で囲むこと）。

```json
{
  "action_type": "OBSERVE" | "VERIFY" | "CONFIGURE" | "ROLLBACK" | "ASK_HUMAN" | "FINISH",
  "objective": "このアクションの具体的な目的 (例: NakaokuGWのARPテーブルをState取得APIで確認する)",
  "tool": "利用するツール名 (例: get_state, query_nw_db, network_show 等。FINISHの場合はnull)",
  "target": "対象機器名またはホスト名 (不要な場合はnull)",
  "parameters": {
    "device": "NakaokuGW",
    "resource": "arp"
  },
  "reason": [
    "アクションを選択した理由 (例: 機器のARPエントリを確認するため) ※FINISHの場合はこのフィールドを出力しない"
  ],
  "expected_observation": [
    "このアクションで期待される観察結果 (例: 最新のARPテーブル情報)"
  ],
  "final_answer": "FINISHの場合にユーザーへ提示する最終報告サマリー（Markdown形式）。FINISHではreasonを書かず、このフィールドだけに集約する"
}
```
"#;

pub fn build_planner_schema(registered_devices: &[String]) -> String {
    let device_schema = if !registered_devices.is_empty() {
        let enum_json =
            serde_json::to_string(registered_devices).unwrap_or_else(|_| "[]".to_string());
        format!(r#"{{ "type": "string", "enum": {} }}"#, enum_json)
    } else {
        r#"{ "type": "string" }"#.to_string()
    };

    let target_schema = if !registered_devices.is_empty() {
        let enum_json =
            serde_json::to_string(registered_devices).unwrap_or_else(|_| "[]".to_string());
        format!(
            r#"{{ "anyOf": [ {{ "type": "string", "enum": {} }}, {{ "type": "null" }} ] }}"#,
            enum_json
        )
    } else {
        r#"{ "type": ["string", "null"] }"#.to_string()
    };

    format!(
        r#"{{
  "type": "object",
  "properties": {{
    "action_type": {{
      "type": "string",
      "enum": ["OBSERVE", "VERIFY", "CONFIGURE", "ROLLBACK", "ASK_HUMAN", "FINISH"]
    }},
    "objective": {{ "type": "string" }},
    "tool": {{ "type": ["string", "null"] }},
    "target": {target_schema},
    "parameters": {{
      "type": "object",
      "properties": {{
        "device": {device_schema},
        "resource": {{
          "type": "string",
          "enum": ["arp", "routes", "interfaces", "lldp", "mac_table", "bgp", "ospf"]
        }},
        "query": {{ "type": "string" }},
        "command": {{ "type": "string" }},
        "host": {{ "type": "string" }},
        "id": {{ "type": "string" }}
      }}
    }},
    "reason": {{
      "type": "array",
      "items": {{ "type": "string" }}
    }},
    "expected_observation": {{
      "type": "array",
      "items": {{ "type": "string" }}
    }},
    "final_answer": {{ "type": ["string", "null"] }}
  }},
  "required": ["action_type", "objective"]
}}"#
    )
}

pub const DECISION_JSON_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "action_type": {
      "type": "string",
      "enum": ["OBSERVE", "VERIFY", "CONFIGURE", "ROLLBACK", "ASK_HUMAN", "FINISH"]
    },
    "objective": { "type": "string" },
    "tool": { "type": ["string", "null"] },
    "target": { "type": ["string", "null"] },
    "parameters": {
      "type": "object",
      "properties": {
        "device": { "type": "string" },
        "resource": {
          "type": "string",
          "enum": ["arp", "routes", "interfaces", "lldp", "mac_table", "bgp", "ospf"]
        },
        "query": { "type": "string" },
        "command": { "type": "string" },
        "host": { "type": "string" },
        "id": { "type": "string" }
      }
    },
    "reason": {
      "type": "array",
      "items": { "type": "string" }
    },
    "expected_observation": {
      "type": "array",
      "items": { "type": "string" }
    },
    "final_answer": { "type": ["string", "null"] }
  },
  "required": ["action_type", "objective"]
}"#;

pub struct LlmPlanner;

/// Convert command-specification questions into a vendor-scoped RAG lookup.
/// This is a deterministic backstop for cases where the model ignores the
/// planner prompt and asks the user for a command that NW-DB may already have.
fn fallback_to_rag_for_known_vendor(
    decision: &mut Decision,
    device_vendors: &[(String, String)],
    goal: &str,
) {
    if decision.action_type != crate::state::events::ActionType::AskHuman {
        return;
    }

    let command_question =
        format!("{} {}", decision.objective, decision.reason.join(" ")).to_lowercase();
    let is_command_question = ["コマンド", "command", "構文", "仕様", "cli", "hostname"]
        .iter()
        .any(|term| command_question.contains(term));
    if !is_command_question {
        return;
    }

    let target = decision.target.as_deref().or_else(|| {
        device_vendors
            .iter()
            .find(|(hostname, _)| {
                let hostname = hostname.to_lowercase();
                command_question.contains(&hostname) || goal.to_lowercase().contains(&hostname)
            })
            .map(|(hostname, _)| hostname.as_str())
    });
    let Some(target) = target else { return };
    let Some((hostname, vendor)) = device_vendors
        .iter()
        .find(|(hostname, _)| hostname.eq_ignore_ascii_case(target))
    else {
        return;
    };

    let brand = crate::mcp::brands::get_brand(vendor).unwrap_or(vendor);
    decision.action_type = crate::state::events::ActionType::Observe;
    decision.objective = format!("{} ({}) のコマンド仕様をNW-DBで調査する", hostname, brand);
    decision.tool = Some("query_nw_db".to_string());
    decision.target = Some(hostname.clone());
    decision.parameters = serde_json::json!({
        "query": format!("[Context: {}] {} コマンド 設定", brand, goal),
    });
    decision.reason = vec![format!(
        "{} のベンダーは {} と判明しているため、ユーザーにコマンドを確認する前にNW-DBを検索する。",
        hostname, brand
    )];
    decision.expected_observation = vec!["対象機器で使える設定コマンドと手順".to_string()];
    decision.final_answer = None;
}

fn is_explanatory_request(goal: &str) -> bool {
    [
        "教えて",
        "方法",
        "手順",
        "設定例",
        "コマンド",
        "とは",
        "解説",
    ]
    .iter()
    .any(|marker| goal.contains(marker))
}

/// Returns source text when a documentation request was incorrectly routed
/// into a parameter interview after RAG had already found relevant material.
fn explanatory_rag_evidence<'a>(
    decision: &Decision,
    network_state: &'a NetworkState,
    goal: &str,
) -> Option<&'a str> {
    if decision.action_type != crate::state::events::ActionType::AskHuman
        || crate::harness::intent::is_configuration_change_request(goal)
        || !is_explanatory_request(goal)
    {
        return None;
    }

    let Some(evidence) = network_state
        .observed
        .observations
        .iter()
        .rev()
        .find(|observation| observation.source.tool_name.as_deref() == Some("rag_co_worker"))
        .map(|observation| observation.raw.trim())
    else {
        return None;
    };

    if evidence.is_empty()
        || evidence.contains("該当する情報が見つかりません")
        || evidence.contains("資料選定に失敗しました")
    {
        return None;
    }

    Some(evidence)
}

fn finish_with_evidence_answer(decision: &mut Decision, goal: &str, answer: String) {
    decision.action_type = crate::state::events::ActionType::Finish;
    decision.objective = goal.to_string();
    decision.tool = None;
    decision.target = None;
    decision.parameters = serde_json::Value::Null;
    decision.reason.clear();
    decision.expected_observation.clear();
    decision.final_answer = Some(answer);
}

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

        let dynamic_schema = build_planner_schema(&registered_devices);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::decision::parse_decision_from_json;

    #[test]
    fn known_vendor_command_question_falls_back_to_rag() {
        let mut decision = parse_decision_from_json(
            r#"{
            "action_type": "ASK_HUMAN",
            "objective": "F220 の hostname 設定コマンドを確認する",
            "target": "F220",
            "reason": ["FITELnet のコマンド仕様が不明"]
        }"#,
        )
        .unwrap();

        fallback_to_rag_for_known_vendor(
            &mut decision,
            &[("F220".to_string(), "furukawa_fitelnet".to_string())],
            "F220 に hostname aaa を設定する",
        );

        assert_eq!(
            decision.action_type,
            crate::state::events::ActionType::Observe
        );
        assert_eq!(decision.tool.as_deref(), Some("query_nw_db"));
        assert_eq!(decision.target.as_deref(), Some("F220"));
        assert_eq!(
            decision
                .parameters
                .get("query")
                .and_then(|value| value.as_str()),
            Some("[Context: furukawa_fitelnet] F220 に hostname aaa を設定する コマンド 設定")
        );
    }

    #[test]
    fn non_command_question_remains_ask_human() {
        let mut decision = parse_decision_from_json(
            r#"{
            "action_type": "ASK_HUMAN",
            "objective": "変更実施の承認を確認する",
            "target": "F220",
            "reason": ["設定変更にはユーザー承認が必要"]
        }"#,
        )
        .unwrap();

        fallback_to_rag_for_known_vendor(
            &mut decision,
            &[("F220".to_string(), "furukawa_fitelnet".to_string())],
            "F220 に hostname aaa を設定する",
        );

        assert_eq!(
            decision.action_type,
            crate::state::events::ActionType::AskHuman
        );
    }

    #[test]
    fn explanatory_rag_request_finishes_without_requesting_parameters() {
        let mut state = NetworkState::with_goal("F220のTrunk VLANの設定を教えて".to_string());
        state.apply_observation(crate::state::events::Observation {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            raw: "=== 選択資料: Trunk VLAN ===\ninterface <INTERFACE>\nswitchport trunk allowed vlan <VLAN_ID>".to_string(),
            parsed: None,
            source: crate::state::events::ObservationSource {
                device: Some("F220".to_string()),
                command: None,
                tool_name: Some("rag_co_worker".to_string()),
                tool_kind: None,
                parameters: None,
            },
            provenance: crate::state::events::Provenance {
                origin: crate::state::events::ProvenanceOrigin::Llm,
                confidence: None,
            },
        });
        let mut decision = parse_decision_from_json(
            r#"{"action_type":"ASK_HUMAN","objective":"VLAN IDとポートを確認する","reason":["設定値が未指定"]}"#,
        )
        .unwrap();

        let evidence =
            explanatory_rag_evidence(&decision, &state, "F220のTrunk VLANの設定を教えて").unwrap();
        finish_with_evidence_answer(
            &mut decision,
            "F220のTrunk VLANの設定を教えて",
            format!("資料を要約した回答です。\n\n{evidence}"),
        );

        assert_eq!(
            decision.action_type,
            crate::state::events::ActionType::Finish
        );
        assert!(decision.final_answer.unwrap().contains("<VLAN_ID>"));
    }

    #[test]
    fn test_build_planner_schema_conversion() {
        let empty_schema = build_planner_schema(&[]);
        assert!(llama_cpp_2::json_schema_to_grammar(&empty_schema).is_ok());

        let devices = vec![
            "NakaokuGW".to_string(),
            "192.168.50.1".to_string(),
            "rt01".to_string(),
        ];
        let dynamic_schema = build_planner_schema(&devices);
        assert!(dynamic_schema.contains(r#""enum": ["NakaokuGW","192.168.50.1","rt01"]"#));
        let res = llama_cpp_2::json_schema_to_grammar(&dynamic_schema);
        assert!(res.is_ok(), "Schema conversion failed: {:?}", res);
    }
}
