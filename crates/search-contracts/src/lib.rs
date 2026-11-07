//! Source identities, object digest bytes and the shared query frame bound.

#![forbid(unsafe_code)]

mod bounds;
mod error;
mod ids;

pub use bounds::MAX_FRAME_BYTES;
pub use error::{ContractError, ContractErrorCode, ContractErrorKind};
pub use ids::{Blake3Digest32, SourceId, SourceNamespaceId};
