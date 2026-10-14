//! Persistent DIRECT command composition behind the stable facade.

#[cfg(test)]
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
#[path = "kernel/source_registration.rs"]
mod source_registration;
#[path = "kernel/source_roots.rs"]
mod source_roots;
#[path = "kernel/source_sync.rs"]
mod source_sync;
#[cfg(test)]
#[path = "kernel/store.rs"]
mod store;
#[path = "kernel/support.rs"]
mod support;

pub use entry::maybe_run;
pub use entry::maybe_refuse_legacy_effects;
pub use support::LEGACY_EFFECTS_UNAVAILABLE;
