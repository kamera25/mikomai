use crate::state::desired::DesiredState;
use crate::state::event_log::EventLog;
use crate::state::events::{Action, ActionResult, HarnessEvent, Observation};
use crate::state::hypothesis::Hypothesis;
use crate::state::observed::ObservedState;
use serde::{Deserialize, Serialize};

/// Central Network State tracking Observed, Desired, Hypotheses, and EventLog
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkState {
    pub task_id: Option<uuid::Uuid>,
    pub observed: ObservedState,
    pub desired: Option<DesiredState>,
    pub hypotheses: Vec<Hypothesis>,
    pub event_log: EventLog,
}

impl NetworkState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_goal(goal: String) -> Self {
        let mut state = Self::default();
        state.set_goal(goal);
        state
    }

    pub fn start_task(&mut self, task_id: uuid::Uuid, goal: String) {
        self.task_id = Some(task_id);
        self.event_log.push(HarnessEvent::TaskStarted {
            task_id,
            timestamp: chrono::Utc::now(),
        });
        self.set_goal(goal);
    }

    pub fn set_goal(&mut self, goal: String) {
        self.desired = Some(DesiredState {
            raw_goal: goal.clone(),
            requirements: Vec::new(),
        });
        self.event_log.push(HarnessEvent::GoalSet {
            goal,
            timestamp: chrono::Utc::now(),
        });
    }

    pub fn apply_observation(&mut self, obs: Observation) {
        self.incorporate_observation(&obs);
        self.event_log.push(HarnessEvent::Observation(obs));
    }

    /// Records the result of an executed action as one causal event.
    ///
    /// Direct observations (for example, imported facts) use
    /// `apply_observation`; tool execution must use this method so a replay can
    /// retain the action-result relationship without duplicating facts.
    pub fn record_action_result(
        &mut self,
        action: Action,
        success: bool,
        observation: Observation,
    ) {
        self.incorporate_observation(&observation);
        self.event_log.push(HarnessEvent::Result(ActionResult {
            id: uuid::Uuid::new_v4(),
            action_id: action.id,
            timestamp: chrono::Utc::now(),
            success,
            observation,
        }));
    }

    fn incorporate_observation(&mut self, obs: &Observation) {
        if let Some(device_name) = &obs.source.device {
            let device_fact = self
                .observed
                .devices
                .entry(device_name.clone())
                .or_insert_with(|| crate::state::observed::DeviceFact::new(device_name.clone()));

            if let Some(cmd) = &obs.source.command {
                device_fact
                    .raw_snapshots
                    .insert(cmd.clone(), obs.raw.clone());
            }

            if let Some(ref parsed) = obs.parsed {
                if let Some(obj) = parsed.as_object() {
                    for (k, v) in obj {
                        if k.starts_with("if_") || k == "interfaces" {
                            device_fact.interfaces.insert(k.clone(), v.clone());
                        } else if k.starts_with("rt_") || k == "routes" {
                            device_fact.routes.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
        }

        self.observed.observations.push(obs.clone());
    }

    pub fn rebuild_from_log(log: &EventLog) -> Self {
        let mut state = Self::new();
        for event in log.events() {
            match event {
                HarnessEvent::TaskStarted { task_id, .. } => state.task_id = Some(*task_id),
                HarnessEvent::GoalSet { goal, .. } => {
                    state.desired = Some(DesiredState {
                        raw_goal: goal.clone(),
                        requirements: Vec::new(),
                    });
                }
                HarnessEvent::Observation(obs) => state.incorporate_observation(obs),
                HarnessEvent::Result(result) => state.incorporate_observation(&result.observation),
                _ => {}
            }
        }
        state.event_log = log.clone();
        state
    }

    pub fn to_prompt_context(&self) -> String {
        let mut out = String::new();
        if let Some(desired) = &self.desired {
            out.push_str(&format!("【Goal / 達成目標】\n{}\n\n", desired.raw_goal));
        }

        out.push_str(
            "【Tool Execution History & Observed Facts / これまでに実行したツールとその結果】\n",
        );
        if self.observed.observations.is_empty() {
            out.push_str("（まだ実行されたツール・観察された情報はありません）\n");
        } else {
            for (idx, obs) in self.observed.observations.iter().enumerate() {
                let tool_info = obs.source.tool_name.as_deref().unwrap_or("unknown_tool");
                let target_info = obs.source.device.as_deref().unwrap_or("localhost/default");
                let params_info = obs
                    .source
                    .parameters
                    .as_ref()
                    .map(|p| p.to_string())
                    .unwrap_or_default();

                let is_error_indication = obs.raw.contains("% Invalid")
                    || obs.raw.contains("Syntax error")
                    || obs.raw.contains("unknown command")
                    || obs.raw.contains("エラー: コマンド")
                    || obs.raw.contains("エラー:不正なコマンド")
                    || obs.raw.contains("Invalid input")
                    || obs.raw.contains("command not found")
                    || obs.raw.contains("unrecognized command");

                let error_hint = if is_error_indication {
                    "\n> ⚠️ **[コマンド不一致/構文エラーの兆候]**: この機器OS・メーカー（Yamaha等）ではコマンド構文が異なる可能性があります。`query_nw_db` (RAG検索) を使って正しいコマンドを調査してください。\n"
                } else {
                    ""
                };

                out.push_str(&format!(
                    "### [{}] ツール: `{}` (対象: {}, 引数: {})\n**実行結果 (Raw Output)**:{}\n```\n{}\n```\n\n",
                    idx + 1,
                    tool_info,
                    target_info,
                    params_info,
                    error_hint,
                    obs.raw.trim()
                ));
            }
        }

        if !self.hypotheses.is_empty() {
            out.push_str("【Hypotheses / 検証中の仮説】\n");
            for h in &self.hypotheses {
                out.push_str(&format!(
                    "- [{}] {} (確信度: {:.2})\n",
                    h.id, h.description, h.confidence
                ));
            }
            out.push('\n');
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::events::{
        Action, ActionType, ObservationSource, Provenance, ProvenanceOrigin,
    };

    fn observation(raw: &str) -> Observation {
        Observation {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            raw: raw.to_string(),
            parsed: None,
            source: ObservationSource {
                device: Some("R1".to_string()),
                command: Some("show version".to_string()),
                tool_name: Some("network_show".to_string()),
                tool_kind: None,
                parameters: None,
            },
            provenance: Provenance {
                origin: ProvenanceOrigin::Tool,
                confidence: Some(1.0),
            },
        }
    }

    #[test]
    fn action_result_replays_once_without_an_extra_observation_event() {
        let mut state = NetworkState::with_goal("R1 を確認".to_string());
        let action = Action {
            id: uuid::Uuid::new_v4(),
            decision_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            action_type: ActionType::Observe,
            tool: Some("network_show".to_string()),
            target: Some("R1".to_string()),
            parameters: serde_json::json!({"command": "show version"}),
        };
        state.record_action_result(action, true, observation("IOS XE"));

        assert_eq!(state.observed.observations.len(), 1);
        assert!(matches!(
            state.event_log.events().last(),
            Some(HarnessEvent::Result(_))
        ));
        let rebuilt = NetworkState::rebuild_from_log(&state.event_log);
        assert_eq!(rebuilt.observed.observations.len(), 1);
        assert_eq!(
            rebuilt.observed.devices["R1"].raw_snapshots["show version"],
            "IOS XE"
        );
    }

    #[test]
    fn task_identity_survives_event_replay() {
        let task_id = uuid::Uuid::new_v4();
        let mut state = NetworkState::new();
        state.start_task(task_id, "R1 を確認".to_string());

        let rebuilt = NetworkState::rebuild_from_log(&state.event_log);
        assert_eq!(rebuilt.task_id, Some(task_id));
        assert_eq!(rebuilt.desired.unwrap().raw_goal, "R1 を確認");
    }
}
