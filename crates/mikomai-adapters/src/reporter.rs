//! Presentation adapters. Core events are rendered here, never in domain code.
use mikomai_core::port::{ReportEvent, ReporterPort};

#[derive(Default, Clone, Copy)]
pub struct CliReporter;
impl ReporterPort for CliReporter {
    fn report(&self, event: ReportEvent) {
        match event {
            ReportEvent::Completed { answer, .. } => eprintln!("[mikomai] completed: {answer}"),
            ReportEvent::Status { status, .. } => eprintln!("[mikomai] {status}"),
            ReportEvent::TaskStarted { .. } | ReportEvent::Evidence { .. } => {}
        }
    }
}

/// Adapter for GUI/event-loop reporters. The callback is supplied by the
/// inbound application layer, so this crate stays independent from Tauri.
pub struct CallbackReporter<F>(pub F);
impl<F> ReporterPort for CallbackReporter<F>
where
    F: Fn(ReportEvent) + Send + Sync,
{
    fn report(&self, event: ReportEvent) {
        (self.0)(event);
    }
}
