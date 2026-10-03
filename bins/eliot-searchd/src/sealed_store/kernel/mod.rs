//! Sealed-store composition behind the stable module facade.

mod api;
mod envelope;
mod model;
mod platform;
mod spec;

pub use api::{delete_sealed, open_sealed, seal_immutable, verify_sealed};
pub use model::{DeleteReceipt, SealReceipt, SensitiveBytes, VerifyReceipt};
pub use spec::{
    MAX_ENVELOPE_BYTES, MAX_OBJECT_ID_BYTES, MAX_PLAINTEXT_BYTES,
    SealedStoreError,
};

#[cfg(test)]
mod tests;
