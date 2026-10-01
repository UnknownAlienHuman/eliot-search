//! Canonical provider-protocol composition for the local daemon edge.
//!
//! The stable daemon-local surface is exported here. Closed wire specification,
//! capability gating, keyed envelope proofing, bounded framing and rendering,
//! connection state, child-outcome mapping, and workspace-currentness remain
//! behind one private owner while they are separated by responsibility.

#[path = "provider_composition/local_stream.rs"]
mod local_stream;
#[path = "provider_composition/kernel.rs"]
mod kernel;

pub(crate) use local_stream::LocalByteStream;
pub use kernel::*;
