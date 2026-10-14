//! Result-handle composition behind the stable daemon-local facade.

#[path = "kernel/catalog.rs"]
mod catalog;
#[path = "kernel/error.rs"]
mod error;
#[path = "kernel/expand.rs"]
mod expand;
#[path = "kernel/model.rs"]
mod model;
#[path = "kernel/spec.rs"]
mod spec;

pub use catalog::ResultHandleCatalog;
pub use error::ResultHandleError;
pub use model::{PublicHandledMatch, ResultHandleExpansion};
pub use spec::{
    MAX_HANDLE_EXPANSION_BYTES, MAX_RESULT_HANDLES, RESULT_HANDLE_TTL,
};

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;
