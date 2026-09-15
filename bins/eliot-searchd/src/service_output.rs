//! Bounded newline-delimited JSON output for the owner-fenced DIRECT service.
//!
//! Framing, indexed/search/page/provider renderers and regressions live in
//! bounded private owners behind this stable module surface.

#[path = "service_output/kernel.rs"]
mod kernel;

pub use kernel::*;
