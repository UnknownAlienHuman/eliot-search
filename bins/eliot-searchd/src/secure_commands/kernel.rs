//! Persistent DIRECT command composition behind the stable facade.

mod commands;
mod dispatch;
mod entry;
mod output;
mod store;
mod support;

pub use entry::maybe_run;
