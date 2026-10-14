//! Restore composition behind the stable daemon-local facade.

#[path = "kernel/cutover.rs"]
mod cutover;
#[path = "kernel/fixture.rs"]
mod fixture;
#[path = "kernel/lifecycle.rs"]
mod lifecycle;
#[path = "kernel/manifest.rs"]
mod manifest;
#[path = "kernel/model.rs"]
mod model;
#[path = "kernel/receipt.rs"]
mod receipt;
#[path = "kernel/spec.rs"]
mod spec;

pub use cutover::*;
pub use fixture::*;
pub use lifecycle::*;
pub use manifest::*;
pub use model::*;
pub use receipt::*;
pub use spec::*;

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;
