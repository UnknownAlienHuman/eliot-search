//! Final-handle adapter composition behind the stable daemon-local facade.

#[path = "kernel/backend.rs"]
mod backend;
#[path = "kernel/identity.rs"]
mod identity;
#[path = "kernel/path.rs"]
mod path;
#[path = "kernel/read.rs"]
mod read;
#[path = "kernel/spec.rs"]
mod spec;

pub use backend::{FinalHandle, FinalHandleBackend};
pub use identity::{file_identity_digest, root_identity_digest};
pub use path::{
    DerivedLocator, canonicalize_admitted_root, derive_locator,
    validate_relative_token,
};
pub use read::{FullFileRead, FullReadError, read_full_file_via_kernel};
pub use spec::{
    ADAPTER_MAX_ATTEMPTS, ADAPTER_NO_EXECUTE, ADAPTER_SINGLE_READ_BYTES,
    AdapterError, qualified_profile,
};

#[cfg(test)]
mod tests;
