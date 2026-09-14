//! Owner-fenced DIRECT runtime with paged search and opaque source handles.
//!
//! The stable process entry delegates to bounded runtime-service owners.

#[path = "public_runtime_service/kernel.rs"]
mod kernel;

pub use kernel::maybe_run;
