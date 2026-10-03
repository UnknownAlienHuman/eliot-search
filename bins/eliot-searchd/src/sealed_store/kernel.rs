//! Sealed-store composition behind the stable module facade.

#[path = "kernel/api.rs"]
mod api;
#[path = "kernel/envelope.rs"]
mod envelope;
#[path = "kernel/model.rs"]
mod model;
#[path = "kernel/platform.rs"]
mod platform;
#[path = "kernel/spec.rs"]
mod spec;

pub use api::{delete_sealed, open_sealed, seal_immutable, verify_sealed};
pub use model::{DeleteReceipt, SealReceipt, SensitiveBytes, VerifyReceipt};
pub use spec::{
    MAX_ENVELOPE_BYTES, MAX_OBJECT_ID_BYTES, MAX_PLAINTEXT_BYTES,
    SealedStoreError,
};

#[cfg(test)]
mod tests;
