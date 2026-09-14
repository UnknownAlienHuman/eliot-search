//! Owner-fenced DIRECT runtime composition.

#[path = "../service_session.rs"]
mod session;

mod codec;
mod diagnostics;
mod dispatch;
mod entry;
mod mutation;
mod query;
mod reporting;
mod runtime;
mod spec;
mod state;

pub use entry::maybe_run;
