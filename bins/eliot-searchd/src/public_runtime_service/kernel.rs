//! Owner-fenced DIRECT runtime composition.

mod codec;
mod diagnostics;
mod dispatch;
mod entry;
mod mutation;
mod query;
mod reporting;
mod runtime;
mod session;
mod spec;
mod state;

pub use entry::maybe_run;
