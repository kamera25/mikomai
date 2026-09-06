//! Deterministic planner driven by FastRouter (shortcut) decisions.
//!
//! Provides a 0-LLM planner implementation that produces deterministic `Decision`
//! structures directly from regex-matched shortcuts or static replies, and drives
//! `AgentLoop` through execution and terminal reporting without model loading.

use crate::harness::ports::PlannerPort;
use crate::llm::router::{RouteAction, RoutingDecision};
use crate::state::events::{ActionType, Decision};
use crate::state::network_state::NetworkState;
use std::sync::Mutex;

pub struct ShortcutPlanner {
    initial_decision: Mutex<Option<Decision>>,
    tool_message: String,
}

impl ShortcutPlanner {
    pub fn new(routing: RoutingDecision) -> Self {
        let (initial_decision, tool_message) = match routing.action {
            RouteAction::DirectToolCall {
                tool_name,
                params,
                message,
            } => {
                let target = params
                    .get("host")
                    .or_else(|| params.get("device"))
                    .or_else(|| params.get("deviceName"))
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);

                let objective = if message.trim().is_empty() {
                    format!("{}を実行する", tool_name)
                } else {
                    message.clone()
                };

                let decision = Decision {
                    id: uuid::Uuid::new_v4(),
                    timestamp: chrono::Utc::now(),
                    action_type: ActionType::Observe,
                    objective,
                    tool: Some(tool_name.clone()),
                    target,
                    parameters: params,
                    reason: vec![format!(
                        "FastRouterによる決定的ショートカット実行: {}",
                        tool_name
                    )],
                    expected_observation: vec![format!("{}の実行結果", tool_name)],
                    final_answer: None,
                };
                (Some(decision), message)
            }
            RouteAction::StaticReply { message } => {
                let decision = Decision {
                    id: uuid::Uuid::new_v4(),
                    timestamp: chrono::Utc::now(),
                    action_type: ActionType::Finish,
                    objective: "静的応答".to_string(),
                    tool: None,
                    target: None,
                    parameters: serde_json::Value::Null,
                    reason: vec![],
                    expected_observation: vec![],
                    final_answer: Some(message.clone()),
                };
                (Some(decision), message)
            }
            _ => (None, String::new()),
        };

        Self {
            initial_decision: Mutex::new(initial_decision),
            tool_message,
        }
    }
}

impl PlannerPort for ShortcutPlanner {
    async fn plan(&self, state: &NetworkState) -> Result<Decision, String> {
        let initial = self
            .initial_decision
            .lock()
            .map_err(|e| format!("Failed to acquire lock on ShortcutPlanner: {}", e))?
            .take();

        if let Some(decision) = initial {
            return Ok(decision);
        }

        // Step 2+: The tool has executed and recorded an observation in NetworkState.
        // Complete the loop with a deterministic Finish decision incorporating the observation.
        let output = state
            .observed
            .observations
            .last()
            .map(|obs| obs.raw.as_str())
            .unwrap_or("");

        let final_answer = if output.is_empty() {
            self.tool_message.clone()
        } else if self.tool_message.is_empty() {
            output.to_string()
        } else {
            format!("{}\n\n```\n{}\n```", self.tool_message, output)
        };

        let objective = state
            .desired
            .as_ref()
            .map(|d| d.raw_goal.clone())
            .unwrap_or_else(|| "ツールの実行結果を報告する".to_string());

        Ok(Decision {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            action_type: ActionType::Finish,
            objective,
            tool: None,
            target: None,
            parameters: serde_json::Value::Null,
            reason: vec![],
            expected_observation: vec![],
            final_answer: Some(final_answer),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::agent_loop::AgentLoop;
    use crate::harness::fake::{FakeToolExecutor, RecordingReporter};
    use crate::llm::router::RoutingSource;
    use crate::network::CommandResult;

    #[tokio::test]
    async fn test_shortcut_planner_static_reply_finishes_immediately() {
        let routing = RoutingDecision {
            action: RouteAction::StaticReply {
                message: "こんにちは！".to_string(),
            },
            confidence: 1.0,
            device_contexts: vec![],
            source: RoutingSource::Shortcut,
        };

        let planner = ShortcutPlanner::new(routing);
        let state = NetworkState::new();

        let decision = planner.plan(&state).await.unwrap();
        assert_eq!(decision.action_type, ActionType::Finish);
        assert_eq!(decision.final_answer, Some("こんにちは！".to_string()));
    }

    #[tokio::test]
    async fn test_shortcut_planner_direct_tool_call_lifecycle() {
        let routing = RoutingDecision {
            action: RouteAction::DirectToolCall {
                tool_name: "self_network_ping".to_string(),
                params: serde_json::json!({ "host": "8.8.8.8" }),
                message: "Pingを実行します。".to_string(),
            },
            confidence: 1.0,
            device_contexts: vec![],
            source: RoutingSource::Shortcut,
        };

        let planner = ShortcutPlanner::new(routing);
        let executor = FakeToolExecutor::new();
        executor.set_tool_result(
            "self_network_ping",
            Ok(CommandResult {
                success: true,
                output: "4 packets transmitted, 4 packets received, 0.0% packet loss".to_string(),
                saved_path: None,
                is_cached: None,
                cache_time: None,
            }),
        );

        let reporter = RecordingReporter::new();
        let mut agent_loop = AgentLoop::new_headless(5);

        let result = agent_loop
            .run_with(
                "8.8.8.8にpingして".to_string(),
                &planner,
                &executor,
                &reporter,
            )
            .await
            .unwrap();

        assert!(result.contains("Pingを実行します。"));
        assert!(result.contains("4 packets transmitted"));

        // Verify that events recorded Decision for Observe, then Finish
        let decisions: Vec<_> = agent_loop
            .network_state
            .event_log
            .events()
            .iter()
            .filter_map(|e| match e {
                crate::state::events::HarnessEvent::Decision(d) => Some(d),
                _ => None,
            })
            .collect();

        assert_eq!(decisions.len(), 2);
        assert_eq!(decisions[0].action_type, ActionType::Observe);
        assert_eq!(decisions[0].tool.as_deref(), Some("self_network_ping"));
        assert_eq!(decisions[0].target.as_deref(), Some("8.8.8.8"));
        assert_eq!(decisions[1].action_type, ActionType::Finish);
    }
}
