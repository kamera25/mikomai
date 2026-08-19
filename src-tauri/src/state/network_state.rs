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

        out.push_str("【Observed Facts / 観察された事実】\n");
        if self.observed.devices.is_empty() && self.observed.facts.is_empty() {
            out.push_str("（まだ観察された機器情報はありません）\n");
        } else {
            for (dev, fact) in &self.observed.devices {
                out.push_str(&format!("- 機器: {}\n", dev));
                for (cmd, raw) in &fact.raw_snapshots {
                    let truncated = if raw.len() > 1000 {
                        format!("{}... (truncated)", &raw[..1000])
                    } else {
                        raw.clone()
                    };
                    out.push_str(&format!("  * コマンド `{}` 結果:\n    {}\n", cmd, truncated.replace('\n', "\n    ")));
                }
            }
        }

        if !self.hypotheses.is_empty() {
            out.push_str("\n【Hypotheses / 検証中の仮説】\n");
            for h in &self.hypotheses {
                out.push_str(&format!("- [{}] {} (確信度: {:.2})\n", h.id, h.description, h.confidence));
            }
        }

        out
    }
}
