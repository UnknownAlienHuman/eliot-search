//! Canonical provider-protocol routing core for the loopback daemon.
//!
//! Closed wire specification, capability evidence, keyed proofing, bounded
//! framing/rendering, connection state, child outcome mapping and workspace
//! currentness are isolated behind the stable provider-composition facade.

#[path = "kernel/canonical.rs"]
mod canonical;
#[path = "kernel/capability.rs"]
mod capability;
#[path = "kernel/child.rs"]
mod child;
#[path = "kernel/codec.rs"]
mod codec;
#[path = "kernel/currentness.rs"]
mod currentness;
#[path = "kernel/indexed.rs"]
mod indexed;
#[path = "kernel/pairing.rs"]
mod pairing;
#[path = "kernel/render.rs"]
mod render;
#[path = "kernel/router.rs"]
mod router;
#[path = "kernel/spec.rs"]
mod spec;

pub use canonical::*;
pub use capability::*;
pub use child::*;
pub use codec::*;
pub use currentness::*;
pub use indexed::*;
pub use pairing::*;
pub use render::*;
pub use router::*;
pub use spec::*;

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;
