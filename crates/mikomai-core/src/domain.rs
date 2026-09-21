//! Domain models and deterministic policies.
pub mod coordinator;
pub mod evidence;
pub mod harness;
pub mod observation;
pub mod operation;
pub mod task;
pub use coordinator::{Coordinator, CoordinatorNext, WorkerKind, WorkerOutcome};
pub use evidence::{Evidence, ObservationSource, Provenance, ProvenanceOrigin};
pub use harness::{HarnessState, HarnessStateMachine};
pub use observation::{ActionType, Decision, Observation};
pub use operation::{ChangePlanner, OperationClass, OperationGate, OperationPlan, OperationStatus};
pub use task::{Task, TaskSnapshot, TaskStatus};
