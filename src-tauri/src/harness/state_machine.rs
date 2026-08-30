use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarnessState {
    Idle,
    Observing,
    Deciding,
    Validating,
    Acting,
    Evaluating,
    AskingHuman,
    Finished,
    Failed,
}

#[derive(Debug, Clone)]
pub struct HarnessStateMachine {
    current_state: HarnessState,
    step_count: usize,
    max_steps: usize,
}

impl HarnessStateMachine {
    pub fn new(max_steps: usize) -> Self {
        Self {
            current_state: HarnessState::Idle,
            step_count: 0,
            max_steps,
        }
    }

    pub fn state(&self) -> HarnessState {
        self.current_state
    }

    pub fn step_count(&self) -> usize {
        self.step_count
    }

    pub fn transition(&mut self, next: HarnessState) -> Result<(), String> {
        log::info!(
            "Harness State Transition: {:?} -> {:?}",
            self.current_state,
            next
        );
        self.current_state = next;
        if next == HarnessState::Deciding {
            self.step_count += 1;
            if self.step_count > self.max_steps {
                self.current_state = HarnessState::Failed;
                return Err(format!("Max steps ({}) exceeded", self.max_steps));
            }
        }
        Ok(())
    }
}
