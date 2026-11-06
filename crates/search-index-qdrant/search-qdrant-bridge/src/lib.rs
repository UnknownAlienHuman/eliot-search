//! Bounded vectorless Qdrant 1.20 BM25 operations for immutable generations.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod backend;
mod codec;
mod error;
mod model;
mod schema;

pub use backend::Backend;
pub use error::BackendError;
pub use model::{
    BackendLimits, DEFAULT_HITS, Hit, MAX_HITS, MAX_PAGE_BYTES, MAX_PAGE_POINTS,
    MAX_POINT_TEXT_BYTES, MAX_QUERY_BYTES, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, PointInput,
};

#[cfg(test)]
mod tests;
