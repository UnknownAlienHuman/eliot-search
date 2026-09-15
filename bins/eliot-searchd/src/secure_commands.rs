//! Secure one-shot dispatcher for persistent DIRECT commands.
//!
//! Command admission, store access, mutation handlers and JSON output live in
//! bounded private owners behind this stable entrypoint.

#[path = "secure_commands/kernel.rs"]
mod kernel;

pub use kernel::maybe_run;
