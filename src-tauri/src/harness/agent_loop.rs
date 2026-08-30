use crate::harness::state_machine::{HarnessState, HarnessStateMachine};
use crate::llm::llm::LlamaState;
use crate::mcp::protocol::{ChatEvent, InitialFinishedPayload, InitialStartedPayload};
use crate::mcp::ToolKind;
use tauri::{AppHandle, Emitter, Window};

use crate::planner::llm_planner::LlmPlanner;
use crate::state::events::{
    Action, ActionType, Decision, HarnessEvent, Observation, ObservationSource, Provenance,
    ProvenanceOrigin,
};
use crate::state::network_state::NetworkState;
use crate::validator::policy::PolicyValidator;
use crate::validator::schema::SchemaValidator;
use std::str::FromStr;

pub struct AgentLoop {
    pub app: AppHandle,
    pub window: Window,
    pub state_machine: HarnessStateMachine,
    pub network_state: NetworkState,
}

fn is_configuration_change_goal(goal: &str) -> bool {
    let goal = goal.to_lowercase();
    [
        "設定する",
        "設定して",
        "設定を変更",
        "変更する",
        "追加する",
        "削除する",
        "投入する",
        "hostname",
    ]
    .iter()
    .any(|marker| goal.contains(marker))
}

fn builder_handoff_action(output: &str) -> ActionType {
    let output = output.to_lowercase();
    if [
        "不足",
        "missing",
        "parameter",
        "入力してください",
        "ask_user_choice",
    ]
    .iter()
    .any(|marker| output.contains(marker))
    {
        ActionType::AskHuman
    } else {
        // Cancellation, validation/dry-run failure, connection errors, and
        // successful deployments all terminate this agent run. In each case
        // the Builder result is the authoritative user-facing outcome.
        ActionType::Finish
    }
}

impl AgentLoop {
    pub fn new(app: AppHandle, window: Window, max_steps: usize) -> Self {
        Self {
            app,
            window,
            state_machine: HarnessStateMachine::new(max_steps),
            network_state: NetworkState::new(),
        }
    }

    fn has_builder_coworker_result(&self) -> bool {
        self.network_state
            .observed
            .observations
            .iter()
            .any(|observation| observation.source.tool_name.as_deref() == Some("builder_co_worker"))
    }

    fn latest_builder_coworker_result(&self) -> Option<&str> {
        self.network_state
            .observed
            .observations
            .iter()
            .rev()
            .find(|observation| {
                observation.source.tool_name.as_deref() == Some("builder_co_worker")
            })
            .map(|observation| observation.raw.as_str())
    }

    pub async fn run(&mut self, goal: String, llama_state: &LlamaState) -> Result<String, String> {
        let task_id = uuid::Uuid::new_v4();
        self.network_state.set_goal(goal.clone());
        self.state_machine.transition(HarnessState::Observing)?;

        let _ = self.window.emit(
            "chat-event",
            ChatEvent::McpInitialStarted(InitialStartedPayload {
                task_id,
                has_image: false,
            }),
        );
        let _ = self.window.emit(
            "chat-event",
            ChatEvent::AgentSelected("エージェントによる解析を開始".to_string()),
        );

        let mut final_report = String::new();
        let mut initial_objective: Option<String> = None;

        loop {
            let current_step = self.state_machine.step_count() + 1;
            log::info!("[AgentLoop] === Step {}: Starting Cycle ===", current_step);

            // 1. Deciding Phase (LLM Planner)
            if self
                .state_machine
                .transition(HarnessState::Deciding)
                .is_err()
            {
                let msg = format!(
                    "最大ステップ数（{}回）に達したため、ループを安全に停止しました。",
                    self.state_machine.step_count()
                );
                log::warn!("[AgentLoop] {}", msg);
                final_report = msg;
                break;
            }

            let _ = self.window.emit(
                "chat-event",
                ChatEvent::LlmChunk(format!(
                    "\n```agent-step\nphase: planning\nstep: {}\n```\n",
                    self.state_machine.step_count()
                )),
            );

            let mut decision: Decision =
                match LlmPlanner::plan(&self.app, llama_state, &self.network_state).await {
                    Ok(d) => d,
                    Err(e) => {
                        log::warn!(
                            "[AgentLoop] Planner failed at step {}: {}",
                            self.state_machine.step_count(),
                            e
                        );
                        Decision {
                            id: uuid::Uuid::new_v4(),
                            timestamp: chrono::Utc::now(),
                            action_type: ActionType::Finish,
                            objective: initial_objective
                                .clone()
                                .unwrap_or_else(|| "目標達成または計画停止".to_string()),
                            tool: None,
                            target: None,
                            parameters: serde_json::Value::Null,
                            reason: vec![format!("Planning encountered error: {}", e)],
                            expected_observation: vec![],
                            final_answer: Some(format!(
                                "計画処理中にエラーが発生したため停止しました: {}",
                                e
                            )),
                        }
                    }
                };

            // Builder is a bounded co-worker. Once it returns, its outcome is
            // handed to the user or used to request missing input; the Agent
            // must not resume live commands, RAG, or a second configuration
            // attempt from that result.
            if let Some(builder_output) = self.latest_builder_coworker_result() {
                let action_type = builder_handoff_action(builder_output);
                let is_missing_input = action_type == ActionType::AskHuman;
                decision.action_type = action_type;
                decision.objective = if is_missing_input {
                    format!(
                        "Builderの結果で不足している値を確認してください。\n\n{}",
                        builder_output
                    )
                } else {
                    "Builder Co-Workerの結果を報告する".to_string()
                };
                decision.tool = None;
                decision.target = None;
                decision.parameters = serde_json::Value::Null;
                decision.reason = vec![
                    "Builder Co-Workerの処理が完了または中断したため、ネットワーク操作を再試行せず結果を返す。"
                        .to_string(),
                ];
                decision.expected_observation = Vec::new();
                decision.final_answer = if is_missing_input {
                    None
                } else {
                    Some(builder_output.to_string())
                };
            }

            // STEP 1のobjectiveをキャプチャし、STEP 2以降は当初のobjectiveを強制的に継承・挿入
            if initial_objective.is_none() {
                let captured = if !decision.objective.trim().is_empty() {
                    decision.objective.clone()
                } else {
                    goal.clone()
                };
                initial_objective = Some(captured);
            } else if let Some(ref init_obj) = initial_objective {
                // Step 2以降: LLMが生成したobjectiveが乖離するのを防ぐため、STEP1の当初objectiveを強制固定/継承
                decision.objective = init_obj.clone();
            }

            // FINISH is a user-facing terminal response. Keep its content in
            // final_answer only, even if the planner or a fallback attached a
            // diagnostic reason.
            if decision.action_type == ActionType::Finish {
                decision.reason.clear();
            }

            log::info!(
                "[AgentLoop] Step {}: Decision proposed -> action_type={:?}, objective='{}', tool={:?}, target={:?}, params={}",
                self.state_machine.step_count(),
                decision.action_type,
                decision.objective,
                decision.tool,
                decision.target,
                decision.parameters
            );
            if let Ok(decision_json) = serde_json::to_string_pretty(&decision) {
                log::info!(
                    "[AgentLoop] Step {}: Decision JSON:\n{}",
                    self.state_machine.step_count(),
                    decision_json
                );
            }

            self.network_state
                .event_log
                .push(HarnessEvent::Decision(decision.clone()));

            let decision_log = if decision.action_type == ActionType::Finish {
                format!(
                    "\n```agent-decision\nstep: {}\naction: {}\nobjective: {}\n```\n",
                    self.state_machine.step_count(),
                    decision.action_type.as_str(),
                    decision.objective.replace('\n', " ")
                )
            } else {
                format!(
                    "\n```agent-decision\nstep: {}\naction: {}\nobjective: {}\nreason: {}\n```\n",
                    self.state_machine.step_count(),
                    decision.action_type.as_str(),
                    decision.objective.replace('\n', " "),
                    decision.reason.join(", ").replace('\n', " ")
                )
            };
            let _ = self.window.emit("chat-event", ChatEvent::LlmChunk(decision_log));

            if decision.action_type == ActionType::Finish {
                self.state_machine.transition(HarnessState::Finished)?;
                let summary_text = if let Some(ref ans) = decision.final_answer {
                    ans.clone()
                } else if !decision.reason.is_empty() {
                    decision.reason.join("\n")
                } else {
                    "目標が達成されました。".to_string()
                };
                log::info!(
                    "[AgentLoop] Step {}: Goal reached / Completed: {}",
                    self.state_machine.step_count(),
                    summary_text
                );
                final_report = summary_text;
                break;
            }

            if decision.action_type == ActionType::AskHuman {
                self.state_machine.transition(HarnessState::AskingHuman)?;
                log::info!(
                    "[AgentLoop] Step {}: Asking human -> '{}'",
                    self.state_machine.step_count(),
                    decision.objective
                );
                final_report = format!("### ❓ 確認要求\n\n{}\n", decision.objective);
                break;
            }

            // 2. Validating Phase (Schema & Policy Validator)
            self.state_machine.transition(HarnessState::Validating)?;
            let action: Action = match SchemaValidator::validate_decision(&decision) {
                Ok(a) => a,
                Err(err_msg) => {
                    log::warn!(
                        "[AgentLoop] Step {}: Schema validation failed: {}",
                        self.state_machine.step_count(),
                        err_msg
                    );
                    continue;
                }
            };

            if let Err(policy_err) = PolicyValidator::validate_action(&action) {
                log::warn!(
                    "[AgentLoop] Step {}: Policy violation: {}",
                    self.state_machine.step_count(),
                    policy_err
                );
                let _ = self.window.emit(
                    "chat-event",
                    ChatEvent::LlmChunk(format!(
                        "\n```agent-warning\nmessage: ポリシー違反により中断: {}\n```\n",
                        policy_err.replace('\n', " ")
                    )),
                );
                continue;
            }

            self.network_state
                .event_log
                .push(HarnessEvent::Action(action.clone()));

            // 3. Acting & Executing Phase (Tool Execution)
            self.state_machine.transition(HarnessState::Acting)?;
            let tool_name = action
                .tool
                .clone()
                .unwrap_or_else(|| "network_show".to_string());
            let tool_kind_opt = ToolKind::from_str(&tool_name).ok();
            let tool_task_id = uuid::Uuid::new_v4();

            let mut tool_args = action.parameters.clone();
            if let Some(target) = &action.target {
                if let serde_json::Value::Object(ref mut map) = tool_args {
                    if !map.contains_key("target") {
                        map.insert(
                            "target".to_string(),
                            serde_json::Value::String(target.clone()),
                        );
                    }
                    if !map.contains_key("device_name") && !map.contains_key("deviceName") {
                        map.insert(
                            "device_name".to_string(),
                            serde_json::Value::String(target.clone()),
                        );
                    }
                    if !map.contains_key("host") {
                        map.insert(
                            "host".to_string(),
                            serde_json::Value::String(target.clone()),
                        );
                    }
                    if !map.contains_key("device") {
                        map.insert(
                            "device".to_string(),
                            serde_json::Value::String(target.clone()),
                        );
                    }
                }
            }

            // MCPツールの引数にも強制的にSTEP 1の当初objectiveを挿入
            if let Some(ref init_obj) = initial_objective {
                if let serde_json::Value::Object(ref mut map) = tool_args {
                    map.insert(
                        "objective".to_string(),
                        serde_json::Value::String(init_obj.clone()),
                    );
                }
            }

            log::info!(
                "[AgentLoop] Step {}: [MCP EXECUTE] tool='{}', target={:?}, params={}",
                self.state_machine.step_count(),
                tool_name,
                action.target,
                tool_args
            );

            let _ = self.window.emit(
                "commit-log",
                serde_json::json!({
                    "line": format!("[AgentLoop Step {}] 🚀 MCP Tool 実行開始: {} (対象: {:?}, 引数: {})", self.state_machine.step_count(), tool_name, action.target.as_deref().unwrap_or("localhost"), tool_args),
                    "stream": "stdout"
                }),
            );

            // A configuration-oriented RAG result is delegated to Builder as
            // a co-worker. The Agent does not infer template values itself;
            // Builder reasons over the request and RAG evidence, then starts
            // the existing review/approval flow.
            if tool_kind_opt.map_or(false, |kind| kind.is_rag_tool())
                && is_configuration_change_goal(&goal)
                && !self.has_builder_coworker_result()
            {
                let builder_result = crate::mcp::executor::flow::execute_mcp_tools_flow(
                    self.app.clone(),
                    self.window.clone(),
                    goal.clone(),
                    vec![crate::mcp::executor::flow::ToolCall {
                        tool: tool_name.clone(),
                        args: tool_args.clone(),
                    }],
                    vec![],
                    vec![],
                    0,
                    120,
                    0,
                    true,
                )
                .await
                .unwrap_or_else(|error| {
                    format!("Builder Co-Workerの実行に失敗しました: {}", error)
                });

                // Co-worker output is a fact for the Agent, not a terminal
                // answer. This lets the Agent decide how to recover from a
                // rejected commit, cancelled approval, or inconsistent input.
                self.network_state.apply_observation(Observation {
                    id: uuid::Uuid::new_v4(),
                    timestamp: chrono::Utc::now(),
                    raw: builder_result,
                    parsed: None,
                    source: ObservationSource {
                        device: action.target.clone(),
                        command: None,
                        tool_name: Some("builder_co_worker".to_string()),
                        tool_kind: None,
                        parameters: Some(serde_json::json!({
                            "delegated_tool": tool_name,
                            "delegated_parameters": tool_args,
                        })),
                    },
                    provenance: Provenance {
                        origin: ProvenanceOrigin::Llm,
                        confidence: None,
                    },
                });
                self.state_machine.transition(HarnessState::Evaluating)?;
                continue;
            }

            // Execute via existing tool executor (it emits McpToolStarted and McpToolFinished internally)
            let cmd_result = crate::mcp::executor::flow::execute_mcp_tool_raw(
                self.app.clone(),
                self.window.clone(),
                tool_task_id,
                tool_name.clone(),
                tool_kind_opt.map_or(tool_name.clone(), |k| k.label().to_string()),
                goal.clone(),
                tool_args.clone(),
                vec![],
                120,
            )
            .await
            .unwrap_or_else(|e| crate::network::CommandResult {
                success: false,
                output: format!("Execution error: {}", e),
                saved_path: None,
                is_cached: None,
                cache_time: None,
            });

            log::info!(
                "[AgentLoop] Step {}: [MCP FINISHED] tool='{}', success={}, output_len={} chars",
                self.state_machine.step_count(),
                tool_name,
                cmd_result.success,
                cmd_result.output.len()
            );

            let _ = self.window.emit(
                "commit-log",
                serde_json::json!({
                    "line": format!("[AgentLoop Step {}] ✅ MCP Tool 実行完了: {} (成否: {})", self.state_machine.step_count(), tool_name, if cmd_result.success { "成功" } else { "失敗" }),
                    "stream": "stdout"
                }),
            );

            // 4. Observing Phase (Wrap raw output into Observation)
            self.state_machine.transition(HarnessState::Observing)?;
            let observation = Observation {
                id: uuid::Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                raw: cmd_result.output.clone(),
                parsed: None,
                source: ObservationSource {
                    device: action.target.clone(),
                    command: action
                        .parameters
                        .get("command")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    tool_name: Some(tool_name.clone()),
                    tool_kind: tool_kind_opt,
                    parameters: Some(action.parameters.clone()),
                },
                provenance: Provenance {
                    origin: ProvenanceOrigin::Tool,
                    confidence: Some(1.0),
                },
            };

            // 5. State Update & Evaluation Phase
            self.network_state.apply_observation(observation);
            self.state_machine.transition(HarnessState::Evaluating)?;
        }

        let _ = self.window.emit(
            "chat-event",
            ChatEvent::McpInitialFinished(InitialFinishedPayload {
                task_id,
                content: final_report.clone(),
            }),
        );

        Ok(final_report)
    }
}

#[cfg(test)]
mod tests {
    use super::{builder_handoff_action, is_configuration_change_goal};
    use crate::state::events::ActionType;

    #[test]
    fn only_change_requests_handoff_rag_to_builder_coworker() {
        assert!(is_configuration_change_goal(
            "F220 に hostname aaa を設定する"
        ));
        assert!(!is_configuration_change_goal("F220 の設定を確認する"));
    }

    #[test]
    fn builder_failure_never_resumes_network_commands() {
        assert_eq!(
            builder_handoff_action("Config投入中にエラーが発生しました"),
            ActionType::Finish
        );
        assert_eq!(
            builder_handoff_action("設定に必要な値が不足しています: vlan_id"),
            ActionType::AskHuman
        );
    }
}
