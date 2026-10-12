//! Canonical provider-protocol routing core for the loopback daemon.
//!
//! Closed wire specification, capability evidence, keyed proofing, bounded
//! framing/rendering, connection state, child outcome mapping and workspace
//! currentness are isolated behind the stable provider-composition facade.

// Canonical connections require the optional access/query composition.
#[cfg(feature = "wave4-query")]
#[path = "kernel/canonical.rs"]
mod canonical;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
#[path = "kernel/capability.rs"]
mod capability;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
#[path = "kernel/child.rs"]
mod child;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
#[path = "kernel/codec.rs"]
mod codec;
// DIRECT source commands use currentness without either provider profile.
#[path = "kernel/currentness.rs"]
mod currentness;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
#[path = "kernel/indexed.rs"]
mod indexed;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
#[path = "kernel/pairing.rs"]
mod pairing;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
#[path = "kernel/render.rs"]
mod render;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
#[path = "kernel/router.rs"]
mod router;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
#[path = "kernel/spec.rs"]
mod spec;

#[cfg(feature = "wave4-query")]
pub use canonical::*;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
pub use capability::*;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
pub use child::*;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
pub use codec::*;
pub use currentness::*;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
pub use indexed::*;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
pub use pairing::*;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
pub use render::*;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
pub use router::*;
#[cfg(any(test, feature = "wave4-query", feature = "legacy-loopback-harness"))]
pub use spec::*;

#[cfg(test)]
mod tests;
