//! Sealed catalog composition behind the stable facade.

mod bind;
mod codec;
mod model;
mod read;
mod spec;

pub use bind::bind_revision;
pub use model::{
    SealedCatalogBinding, SealedCatalogRead, SealedCatalogReceipt,
    SealedCatalogVerifyReceipt,
};
pub use read::{read_revision, verify_revision};
pub use spec::{MAX_CATALOG_IDENTIFIER_BYTES, SealedCatalogError};

#[cfg(test)]
mod tests;
