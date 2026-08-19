use tauri::{AppHandle, Emitter, Window};
use crate::harness::state_machine::{HarnessState, HarnessStateMachine};
use crate::llm::llm::LlamaState;
use crate::mcp::protocol::{ChatEvent, InitialFinishedPayload, InitialStartedPayload, ToolFinishedPayload, ToolStartedPayload};
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
            // 1. Deciding Phase (LLM Planner)
            if self.state_machine.transition(HarnessState::Deciding).is_err() {
                final_report = format!("最大ステップ数（{}回）に達したため、ループを安全に停止しました。", self.state_machine.step_count());
                break;
            }

            let _ = self.window.emit(
                "chat-event",
                ChatEvent::LlmChunk(format!("\n\n🤖 **[Step {}: Planning]** 思考中...\n", self.state_machine.step_count())),
            );

            let decision: Decision = match LlmPlanner::plan(&self.app, llama_state, &self.network_state).await {
                Ok(d) => d,
                Err(e) => {
                    log::warn!("Planner failed, attempting fallback observe decision: {}", e);
                    // Fallback to finish if planning fails repeatedly
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
                    }
                }
            };

            self.network_state.event_log.push(HarnessEvent::Decision(decision.clone()));

            let _ = self.window.emit(
                "chat-event",
                ChatEvent::LlmChunk(format!("- **Decision**: [{}] {}\n- **理由**: {}\n", decision.action_type.as_str(), decision.objective, decision.reason.join(", "))),
            );

            if decision.action_type == ActionType::Finish {
                self.state_machine.transition(HarnessState::Finished)?;
                let summary_text = if !decision.reason.is_empty() {
                    decision.reason.join("\n")
                } else {
                    "目標が達成されました。".to_string()
                };
                final_report = format!("### 🎯 目標達成・調査完了\n\n{}\n", summary_text);
                break;
            }

            if decision.action_type == ActionType::AskHuman {
                self.state_machine.transition(HarnessState::AskingHuman)?;
                final_report = format!("### ❓ 確認要求\n\n{}\n", decision.objective);
                break;
            }

            // 2. Validating Phase (Schema & Policy Validator)
            self.state_machine.transition(HarnessState::Validating)?;
            let action: Action = match SchemaValidator::validate_decision(&decision) {
                Ok(a) => a,
                Err(err_msg) => {
                    log::warn!("Schema validation failed: {}", err_msg);
                    continue;
                }
            };

            if let Err(policy_err) = PolicyValidator::validate_action(&action) {
                log::warn!("Policy violation: {}", policy_err);
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
            let _ = self.window.emit(
                "chat-event",
                ChatEvent::McpToolStarted(ToolStartedPayload {
                    task_id: tool_task_id,
                    tool_id: tool_kind_opt.unwrap_or(ToolKind::SelfNetworkPing),
                    args: action.parameters.clone(),
                    resolved_host: action.target.clone(),
                }),
            );

            // Execute via existing tool executor
            let cmd_result = crate::mcp::executor::flow::execute_mcp_tool_raw(
                self.app.clone(),
                self.window.clone(),
                tool_task_id,
                tool_name.clone(),
                tool_kind_opt.map_or(tool_name.clone(), |k| k.label().to_string()),
                goal.clone(),
                action.parameters.clone(),
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

            let _ = self.window.emit(
                "chat-event",
                ChatEvent::McpToolFinished(ToolFinishedPayload {
                    task_id: tool_task_id,
                    success: cmd_result.success,
                    output: cmd_result.output.clone(),
                    saved_path: cmd_result.saved_path.clone(),
                    is_cached: cmd_result.is_cached,
                    cache_time: cmd_result.cache_time.clone(),
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
                    tool_kind: tool_kind_opt,
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
