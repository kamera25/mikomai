use crate::harness::ports::{
    AgentReport, LlmPlannerPort, McpToolExecutorPort, PlannerPort, ReporterPort, TauriReporterPort,
    ToolExecutorPort,
};
use crate::harness::state_machine::{HarnessState, HarnessStateMachine};
use crate::harness::{execution, intent};
use crate::llm::llm::LlamaState;
use crate::mcp::protocol::{ChatEvent, InitialFinishedPayload, InitialStartedPayload};
use crate::mcp::ToolKind;
use crate::state::events::{
    Action, ActionType, Decision, HarnessEvent, Observation, ObservationSource, Provenance,
    ProvenanceOrigin,
};
use crate::state::network_state::NetworkState;
use crate::validator::policy::PolicyValidator;
use crate::validator::schema::SchemaValidator;
use std::str::FromStr;
use tauri::{AppHandle, Manager, Window};

pub struct AgentLoop {
    pub app: Option<AppHandle>,
    pub window: Option<Window>,
    pub state_machine: HarnessStateMachine,
    pub network_state: NetworkState,
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
            app: Some(app),
            window: Some(window),
            state_machine: HarnessStateMachine::new(max_steps),
            network_state: NetworkState::new(),
        }
    }

    pub fn new_headless(max_steps: usize) -> Self {
        Self {
            app: None,
            window: None,
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
        let app = self
            .app
            .clone()
            .ok_or_else(|| "AppHandle is required for live AgentLoop execution".to_string())?;
        let window = self
            .window
            .clone()
            .ok_or_else(|| "Window is required for live AgentLoop execution".to_string())?;
        let planner = LlmPlannerPort::new(app.clone(), llama_state);
        let executor = McpToolExecutorPort::new(app, window.clone(), llama_state);
        let reporter = TauriReporterPort::new(window);
        self.run_with(goal, &planner, &executor, &reporter)
            .await
    }

    pub async fn run_with<P, E, R>(
        &mut self,
        goal: String,
        planner: &P,
        executor: &E,
        reporter: &R,
    ) -> Result<String, String>
    where
        P: PlannerPort,
        E: ToolExecutorPort,
        R: ReporterPort,
    {
        crate::llm::llm::reset_cancel();
        let task_id = uuid::Uuid::new_v4();
        self.network_state.start_task(task_id, goal.clone());
        self.state_machine.transition(HarnessState::Observing)?;

        reporter.report(AgentReport::Chat(ChatEvent::McpInitialStarted(
            InitialStartedPayload {
                task_id,
                has_image: false,
            },
        )));
        reporter.report(AgentReport::Chat(ChatEvent::AgentSelected(
            "エージェントによる解析を開始".to_string(),
        )));

        let mut initial_objective: Option<String> = None;

        let final_report = loop {
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
                break msg;
            }

            reporter.report(AgentReport::Chat(ChatEvent::LlmChunk(format!(
                "\n```agent-step\nphase: planning\nstep: {}\n```\n",
                self.state_machine.step_count()
            ))));

            let mut decision: Decision = match planner
                .plan(&self.network_state)
                .await
            {
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
                // ただし、AskHuman（ユーザーへの確認要求）やBuilder Co-Workerからのハンドオフ時は上書きしない
                if decision.action_type != ActionType::AskHuman
                    && self.latest_builder_coworker_result().is_none()
                {
                    decision.objective = init_obj.clone();
                }
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
            reporter.report(AgentReport::Chat(ChatEvent::LlmChunk(decision_log)));

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
                break summary_text;
            }

            if decision.action_type == ActionType::AskHuman {
                self.state_machine.transition(HarnessState::AskingHuman)?;
                log::info!(
                    "[AgentLoop] Step {}: Asking human -> '{}'",
                    self.state_machine.step_count(),
                    decision.objective
                );
                break format!("### ❓ 確認要求\n\n{}\n", decision.objective);
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
                reporter.report(AgentReport::Chat(ChatEvent::LlmChunk(format!(
                    "\n```agent-warning\nmessage: ポリシー違反により中断: {}\n```\n",
                    policy_err.replace('\n', " ")
                ))));
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

            let tool_args =
                execution::prepare_tool_arguments(&action, initial_objective.as_deref());

            log::info!(
                "[AgentLoop] Step {}: [MCP EXECUTE] tool='{}', target={:?}, params={}",
                self.state_machine.step_count(),
                tool_name,
                action.target,
                tool_args
            );

            reporter.report(AgentReport::CommitLog(format!(
                "[AgentLoop Step {}] 🚀 MCP Tool 実行開始: {} (対象: {:?}, 引数: {})",
                self.state_machine.step_count(),
                tool_name,
                action.target.as_deref().unwrap_or("localhost"),
                tool_args
            )));

            // A configuration-oriented RAG result is delegated to Builder as
            // a co-worker. The Agent does not infer template values itself;
            // Builder reasons over the request and RAG evidence, then starts
            // the existing review/approval flow.
            if tool_kind_opt.map_or(false, |kind| kind.is_rag_tool())
                && intent::is_configuration_change_request(&goal)
                && !self.has_builder_coworker_result()
            {
                let builder_result = executor
                    .execute_builder(
                        goal.clone(),
                        tool_name.clone(),
                        tool_args.clone(),
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
            let cmd_result = executor
                .execute_tool(
                    tool_task_id,
                    tool_name.clone(),
                    goal.clone(),
                    tool_args.clone(),
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

            reporter.report(AgentReport::CommitLog(format!(
                "[AgentLoop Step {}] ✅ MCP Tool 実行完了: {} (成否: {})",
                self.state_machine.step_count(),
                tool_name,
                if cmd_result.success {
                    "成功"
                } else {
                    "失敗"
                }
            )));

            // RAG retrieval is delegated before the next planning turn. The
            // RAG co-worker selects source documents with constrained decoding
            // and returns their original text. The Agent remains responsible
            // for the final decision and user-facing response.
            let (observation_tool_name, observation_tool_kind, observation_output) =
                if tool_kind_opt.map_or(false, |kind| kind.is_rag_tool()) && cmd_result.success {
                    reporter.report(AgentReport::Chat(ChatEvent::AgentSelected(
                        "RAG Worker (RAG回答員)".to_string(),
                    )));
                    reporter.report(AgentReport::Chat(ChatEvent::LlmChunk(format!(
                        "\n```agent-decision\nstep: {}\naction: RAG Co-Worker\nobjective: NW-DB候補資料を選定し、選定資料の本文をAgentへ返却する\nreason: 根拠番号ではなく、コマンドと手順を含む原文を次の判断に渡すため\n```\n",
                        self.state_machine.step_count()
                    ))));

                    let co_worker_result = executor
                        .execute_rag_co_worker(
                            goal.clone(),
                            cmd_result.output.clone(),
                        )
                        .await
                        .unwrap_or_else(|error| {
                            format!("RAG Co-Workerの資料選定に失敗しました: {error}")
                        });

                    log::info!(
                        "[AgentLoop] Step {}: RAG co-worker returned {} chars of selected document text",
                        self.state_machine.step_count(),
                        co_worker_result.len()
                    );
                    ("rag_co_worker".to_string(), None, co_worker_result)
                } else {
                    (tool_name.clone(), tool_kind_opt, cmd_result.output.clone())
                };

            // 4. Wrap the completed tool/co-worker result into an observation.
            // The state machine moves directly from Acting to Evaluating;
            // transitioning through Observing here would make the following
            // Observing -> Evaluating transition invalid and abort the loop.
            let observation = execution::tool_observation(
                &action,
                observation_tool_name,
                observation_tool_kind,
                tool_args,
                observation_output,
            );

            // 5. State Update & Evaluation Phase
            self.network_state
                .record_action_result(action, cmd_result.success, observation);
            self.state_machine.transition(HarnessState::Evaluating)?;
        };

        reporter.report(AgentReport::Chat(ChatEvent::McpInitialFinished(
            InitialFinishedPayload {
                task_id,
                content: final_report.clone(),
            },
        )));

        self.network_state.event_log.push(HarnessEvent::Finished {
            reason: final_report.clone(),
            timestamp: chrono::Utc::now(),
        });
        self.persist_event_log(task_id);

        Ok(final_report)
    }

    fn persist_event_log(&self, task_id: uuid::Uuid) {
        let Some(ref app) = self.app else {
            return;
        };
        let result = (|| -> Result<(), String> {
            let directory = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("Failed to resolve app data directory: {error}"))?
                .join("agent-events");
            std::fs::create_dir_all(&directory)
                .map_err(|error| format!("Failed to create event log directory: {error}"))?;
            self.network_state
                .event_log
                .save_to_path(&directory.join(format!("{task_id}.json")))
        })();

        if let Err(error) = result {
            // The live task result must never be lost merely because auditing
            // storage is unavailable; retain the failure in application logs.
            log::warn!("[AgentLoop] Could not persist task event log: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::builder_handoff_action;
    use crate::harness::intent::is_configuration_change_request;
    use crate::state::events::ActionType;

    #[test]
    fn only_change_requests_handoff_rag_to_builder_coworker() {
        assert!(is_configuration_change_request(
            "F220 に hostname aaa を設定する"
        ));
        assert!(!is_configuration_change_request("F220 の設定を確認する"));
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
