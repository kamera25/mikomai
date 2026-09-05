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
        if !Self::can_transition(self.current_state, next) {
            return Err(format!(
                "Invalid harness transition: {:?} -> {:?}",
                self.current_state, next
            ));
        }
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

    fn can_transition(current: HarnessState, next: HarnessState) -> bool {
        use HarnessState::*;
        matches!(
            (current, next),
            (Idle, Observing)
                | (Observing, Deciding)
                | (Deciding, Validating)
                | (Deciding, AskingHuman)
                | (Deciding, Finished)
                | (Validating, Acting)
                | (Validating, Deciding)
                | (Acting, Observing)
                | (Acting, Evaluating)
                | (Evaluating, Deciding)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_agent_cycle_and_counts_only_decisions() {
        let mut machine = HarnessStateMachine::new(2);
        for state in [
            HarnessState::Observing,
            HarnessState::Deciding,
            HarnessState::Validating,
            HarnessState::Acting,
            HarnessState::Evaluating,
            HarnessState::Deciding,
            HarnessState::Finished,
        ] {
            machine.transition(state).unwrap();
        }
        assert_eq!(machine.state(), HarnessState::Finished);
        assert_eq!(machine.step_count(), 2);
    }

    #[test]
    fn rejects_invalid_and_terminal_transitions_without_mutating_state() {
        let mut machine = HarnessStateMachine::new(1);
        assert!(machine.transition(HarnessState::Acting).is_err());
        assert_eq!(machine.state(), HarnessState::Idle);
        machine.transition(HarnessState::Observing).unwrap();
        machine.transition(HarnessState::Deciding).unwrap();
        machine.transition(HarnessState::Finished).unwrap();
        assert!(machine.transition(HarnessState::Observing).is_err());
        assert_eq!(machine.state(), HarnessState::Finished);
    }

    #[test]
    fn exceeding_the_budget_enters_failed_state() {
        let mut machine = HarnessStateMachine::new(1);
        machine.transition(HarnessState::Observing).unwrap();
        machine.transition(HarnessState::Deciding).unwrap();
        machine.transition(HarnessState::Validating).unwrap();
        machine.transition(HarnessState::Acting).unwrap();
        machine.transition(HarnessState::Evaluating).unwrap();
        let error = machine.transition(HarnessState::Deciding).unwrap_err();
        assert!(error.contains("Max steps (1) exceeded"));
        assert_eq!(machine.state(), HarnessState::Failed);
    }
}
