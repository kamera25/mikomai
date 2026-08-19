use serde::{Deserialize, Serialize};
use crate::state::events::{HarnessEvent, Observation};
use crate::state::observed::ObservedState;
use crate::state::desired::DesiredState;
use crate::state::hypothesis::Hypothesis;
use crate::state::event_log::EventLog;

/// Central Network State tracking Observed, Desired, Hypotheses, and EventLog
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkState {
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
        if let Some(device_name) = &obs.source.device {
            let device_fact = self.observed.devices
                .entry(device_name.clone())
                .or_insert_with(|| crate::state::observed::DeviceFact::new(device_name.clone()));

            if let Some(cmd) = &obs.source.command {
                device_fact.raw_snapshots.insert(cmd.clone(), obs.raw.clone());
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
        self.event_log.push(HarnessEvent::Observation(obs));
    }

    pub fn rebuild_from_log(log: &EventLog) -> Self {
        let mut state = Self::new();
        for event in log.events() {
            match event {
                HarnessEvent::GoalSet { goal, .. } => {
                    state.desired = Some(DesiredState {
                        raw_goal: goal.clone(),
                        requirements: Vec::new(),
                    });
                }
                HarnessEvent::Observation(obs) => {
                    state.apply_observation(obs.clone());
                }
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

        out.push_str("【Tool Execution History & Observed Facts / これまでに実行したツールとその結果】\n");
        if self.observed.observations.is_empty() {
            out.push_str("（まだ実行されたツール・観察された情報はありません）\n");
        } else {
            for (idx, obs) in self.observed.observations.iter().enumerate() {
                let tool_info = obs.source.tool_name.as_deref().unwrap_or("unknown_tool");
                let target_info = obs.source.device.as_deref().unwrap_or("localhost/default");
                let params_info = obs.source.parameters.as_ref().map(|p| p.to_string()).unwrap_or_default();
                
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
                out.push_str(&format!("- [{}] {} (確信度: {:.2})\n", h.id, h.description, h.confidence));
            }
            out.push('\n');
        }

        out
    }

}
