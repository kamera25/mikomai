use tauri::{AppHandle, Emitter, Window};
use crate::harness::state_machine::{HarnessState, HarnessStateMachine};
use crate::llm::llm::LlamaState;
use crate::mcp::protocol::{ChatEvent, InitialFinishedPayload, InitialStartedPayload};
use crate::mcp::ToolKind;

use crate::planner::llm_planner::LlmPlanner;
use crate::state::events::{Action, ActionType, Decision, HarnessEvent, Observation, ObservationSource, Provenance, ProvenanceOrigin};
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

impl AgentLoop {
    pub fn new(app: AppHandle, window: Window, max_steps: usize) -> Self {
        Self {
            app,
            window,
            state_machine: HarnessStateMachine::new(max_steps),
            network_state: NetworkState::new(),
        }
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

        let mut final_report = String::new();

        loop {
            let current_step = self.state_machine.step_count() + 1;
            log::info!("[AgentLoop] === Step {}: Starting Cycle ===", current_step);

            // 1. Deciding Phase (LLM Planner)
            if self.state_machine.transition(HarnessState::Deciding).is_err() {
                let msg = format!("最大ステップ数（{}回）に達したため、ループを安全に停止しました。", self.state_machine.step_count());
                log::warn!("[AgentLoop] {}", msg);
                final_report = msg;
                break;
            }

            let _ = self.window.emit(
                "chat-event",
                ChatEvent::LlmChunk(format!("\n\n🤖 **[Step {}: Planning]** 思考中...\n", self.state_machine.step_count())),
            );

            let decision: Decision = match LlmPlanner::plan(&self.app, llama_state, &self.network_state).await {
                Ok(d) => d,
                Err(e) => {
                    log::warn!("[AgentLoop] Planner failed at step {}: {}", self.state_machine.step_count(), e);
                    Decision {
                        id: uuid::Uuid::new_v4(),
                        timestamp: chrono::Utc::now(),
                        action_type: ActionType::Finish,
                        objective: "目標達成または計画停止".to_string(),
                        tool: None,
                        target: None,
                        parameters: serde_json::Value::Null,
                        reason: vec![format!("Planning encountered error: {}", e)],
                        expected_observation: vec![],
                        final_answer: Some(format!("計画処理中にエラーが発生したため停止しました: {}", e)),
                    }
                }
            };

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
                log::info!("[AgentLoop] Step {}: Decision JSON:\n{}", self.state_machine.step_count(), decision_json);
            }

            self.network_state.event_log.push(HarnessEvent::Decision(decision.clone()));

            let _ = self.window.emit(
                "chat-event",
                ChatEvent::LlmChunk(format!("- **Decision**: [{}] {}\n- **理由**: {}\n", decision.action_type.as_str(), decision.objective, decision.reason.join(", "))),
            );

            if decision.action_type == ActionType::Finish {
                self.state_machine.transition(HarnessState::Finished)?;
                let summary_text = if let Some(ref ans) = decision.final_answer {
                    ans.clone()
                } else if !decision.reason.is_empty() {
                    decision.reason.join("\n")
                } else {
                    "目標が達成されました。".to_string()
                };
                log::info!("[AgentLoop] Step {}: Goal reached / Completed: {}", self.state_machine.step_count(), summary_text);
                final_report = format!("### 🎯 目標達成・調査完了\n\n{}\n", summary_text);
                break;
            }

            if decision.action_type == ActionType::AskHuman {
                self.state_machine.transition(HarnessState::AskingHuman)?;
                log::info!("[AgentLoop] Step {}: Asking human -> '{}'", self.state_machine.step_count(), decision.objective);
                final_report = format!("### ❓ 確認要求\n\n{}\n", decision.objective);
                break;
            }

            // 2. Validating Phase (Schema & Policy Validator)
            self.state_machine.transition(HarnessState::Validating)?;
            let action: Action = match SchemaValidator::validate_decision(&decision) {
                Ok(a) => a,
                Err(err_msg) => {
                    log::warn!("[AgentLoop] Step {}: Schema validation failed: {}", self.state_machine.step_count(), err_msg);
                    continue;
                }
            };

            if let Err(policy_err) = PolicyValidator::validate_action(&action) {
                log::warn!("[AgentLoop] Step {}: Policy violation: {}", self.state_machine.step_count(), policy_err);
                let _ = self.window.emit(
                    "chat-event",
                    ChatEvent::LlmChunk(format!("⚠️ **ポリシー違反により中断**: {}\n", policy_err)),
                );
                continue;
            }

            self.network_state.event_log.push(HarnessEvent::Action(action.clone()));

            // 3. Acting & Executing Phase (Tool Execution)
            self.state_machine.transition(HarnessState::Acting)?;
            let tool_name = action.tool.clone().unwrap_or_else(|| "network_show".to_string());
            let tool_kind_opt = ToolKind::from_str(&tool_name).ok();
            let tool_task_id = uuid::Uuid::new_v4();

            let mut tool_args = action.parameters.clone();
            if let Some(target) = &action.target {
                if let serde_json::Value::Object(ref mut map) = tool_args {
                    if !map.contains_key("target") {
                        map.insert("target".to_string(), serde_json::Value::String(target.clone()));
                    }
                    if !map.contains_key("device_name") && !map.contains_key("deviceName") {
                        map.insert("device_name".to_string(), serde_json::Value::String(target.clone()));
                    }
                    if !map.contains_key("host") {
                        map.insert("host".to_string(), serde_json::Value::String(target.clone()));
                    }
                    if !map.contains_key("device") {
                        map.insert("device".to_string(), serde_json::Value::String(target.clone()));
                    }
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
                    command: action.parameters.get("command").and_then(|v| v.as_str()).map(|s| s.to_string()),
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
