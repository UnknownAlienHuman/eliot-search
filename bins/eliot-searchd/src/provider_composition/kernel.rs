//! Canonical provider-protocol routing core for the loopback daemon.
//!
//! Closed wire specification, capability evidence, keyed proofing, bounded
//! framing/rendering, connection state, child outcome mapping and workspace
//! currentness are isolated behind the stable provider-composition facade.

mod canonical;
mod capability;
mod child;
mod codec;
mod currentness;
mod indexed;
mod pairing;
mod render;
mod router;
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
mod tests;
