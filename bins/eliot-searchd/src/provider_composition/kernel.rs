//! Canonical provider-protocol routing core for the remaining provider composition.
//!
//! Closed wire specification, capability evidence, bounded framing/rendering,
//! connection state, child outcome mapping and workspace currentness remain
//! available only to tests or the historical wave4 query composition. The
//! daemon TCP/token loopback owner was removed in R019.

#[cfg(feature = "wave4-query")]
#[path = "kernel/canonical.rs"]
mod canonical;
#[cfg(any(test, feature = "wave4-query"))]
#[path = "kernel/capability.rs"]
mod capability;
#[cfg(any(test, feature = "wave4-query"))]
#[path = "kernel/child.rs"]
mod child;
#[cfg(any(test, feature = "wave4-query"))]
#[path = "kernel/codec.rs"]
mod codec;
// DIRECT source commands use currentness without either provider profile.
#[path = "kernel/currentness.rs"]
mod currentness;
#[cfg(any(test, feature = "wave4-query"))]
#[path = "kernel/indexed.rs"]
mod indexed;
#[cfg(any(test, feature = "wave4-query"))]
#[path = "kernel/pairing.rs"]
mod pairing;
#[cfg(any(test, feature = "wave4-query"))]
#[path = "kernel/render.rs"]
mod render;
#[cfg(any(test, feature = "wave4-query"))]
#[path = "kernel/router.rs"]
mod router;
#[cfg(any(test, feature = "wave4-query"))]
#[path = "kernel/spec.rs"]
mod spec;

#[cfg(feature = "wave4-query")]
pub use canonical::*;
#[cfg(any(test, feature = "wave4-query"))]
pub use capability::*;
#[cfg(any(test, feature = "wave4-query"))]
pub use child::*;
#[cfg(any(test, feature = "wave4-query"))]
pub use codec::*;
pub use currentness::*;
#[cfg(any(test, feature = "wave4-query"))]
pub use indexed::*;
#[cfg(any(test, feature = "wave4-query"))]
pub use pairing::*;
#[cfg(any(test, feature = "wave4-query"))]
pub use render::*;
#[cfg(any(test, feature = "wave4-query"))]
pub use router::*;
#[cfg(any(test, feature = "wave4-query"))]
pub use spec::*;

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;