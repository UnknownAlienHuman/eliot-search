//! Revision-protected facade over the append-only DIRECT catalog.
//!
//! The stable store surface delegates to bounded private owners for catalog
//! opening, source mutation, canonical read-only search, exact revision reads,
//! verification and referenced-plaintext migration. Existing migration and
//! preparation children retain their store-internal compatibility surface.

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines
)]

use crate::plaintext_direct_store as plaintext;
use plaintext::{RevisionMetadata, verify_revision_identity};
use crate::revision_protection::RevisionProtector;
use crate::sha256;

#[path = "secure_direct_store_storage_io.rs"]
mod storage_io;
#[path = "secure_revision_writer.rs"]
mod revision_writer;
#[path = "preparation_store.rs"]
mod preparation_store;
#[path = "control_migration_objects.rs"]
mod migration_objects;
#[path = "secure_direct_store/kernel.rs"]
mod kernel;

pub use kernel::DirectStore;
pub use preparation_store::{PreparationBatch, PreparationCursor};
pub use plaintext::{
    IndexedSource, RevisionSlice, SourceSummary, StoreGap, StoreSearchResult,
    StoreVerification, StoredMatch,
};

// Compatibility surface for existing store-owned children. These names stay
// private to the DIRECT module and do not widen the daemon API.
use kernel::{
    MAX_REVISION_OBJECT_BYTES, REVISION_DIRECTORY, verify_plaintext,
};
use storage_io::{
    legacy_path, protected_path, read_plaintext_path, read_regular_file,
    read_revision_object, remove_plaintext_after_readback,
};
