//! Persistent DIRECT command composition behind the stable facade.

#[path = "kernel/commands.rs"]
mod commands;
#[path = "kernel/dispatch.rs"]
mod dispatch;
#[path = "kernel/entry.rs"]
mod entry;
#[path = "kernel/output.rs"]
mod output;
#[path = "kernel/read.rs"]
mod read;
#[path = "kernel/source_roots.rs"]
mod source_roots;
#[path = "kernel/store.rs"]
mod store;
#[path = "kernel/support.rs"]
mod support;

pub use entry::maybe_run;
