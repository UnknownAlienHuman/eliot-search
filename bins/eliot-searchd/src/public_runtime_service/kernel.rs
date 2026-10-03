//! Owner-fenced DIRECT runtime composition.

#[path = "kernel/codec.rs"]
mod codec;
#[path = "kernel/diagnostics.rs"]
mod diagnostics;
#[path = "kernel/dispatch.rs"]
mod dispatch;
#[path = "kernel/entry.rs"]
mod entry;
#[path = "kernel/mutation.rs"]
mod mutation;
#[path = "kernel/output_deadline.rs"]
mod output_deadline;
#[path = "kernel/query.rs"]
mod query;
#[path = "kernel/reporting.rs"]
mod reporting;
#[path = "kernel/runtime.rs"]
mod runtime;
#[path = "kernel/session.rs"]
mod session;
#[path = "kernel/spec.rs"]
mod spec;
#[path = "kernel/state.rs"]
mod state;

pub use entry::maybe_run;
