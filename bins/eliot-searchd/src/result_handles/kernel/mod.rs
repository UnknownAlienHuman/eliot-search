//! Result-handle composition behind the stable daemon-local facade.

mod catalog;
mod error;
mod expand;
mod model;
mod spec;

pub use catalog::ResultHandleCatalog;
pub use error::ResultHandleError;
pub use model::{PublicHandledMatch, ResultHandleExpansion};
pub use spec::{
    MAX_HANDLE_EXPANSION_BYTES, MAX_RESULT_HANDLES, RESULT_HANDLE_TTL,
};

#[cfg(test)]
mod tests;
