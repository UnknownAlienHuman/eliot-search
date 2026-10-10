//! Command application for the ELIOT Search daemon binary.
//!
//! The stable binary entry delegates to bounded owners for the control
//! protocol, configuration-derived health, JSON emission, DIRECT commands and
//! top-level dispatch.

#[path = "kernel/commands.rs"]
mod commands;
#[path = "kernel/dispatch.rs"]
mod dispatch;
#[path = "kernel/output.rs"]
mod output;
#[path = "kernel/protocol.rs"]
mod protocol;
#[path = "kernel/source_root_commands.rs"]
mod source_root_commands;
#[path = "kernel/spec.rs"]
mod spec;
#[path = "kernel/status.rs"]
mod status;

pub use dispatch::run_main;

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;
