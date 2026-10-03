//! Command application for the ELIOT Search daemon binary.
//!
//! The stable binary entry delegates to bounded owners for the control
//! protocol, configuration-derived health, JSON emission, DIRECT commands and
//! top-level dispatch.

mod commands;
mod dispatch;
mod output;
mod protocol;
mod source_root_commands;
mod spec;
mod status;

pub use dispatch::run_main;

#[cfg(test)]
mod tests;
