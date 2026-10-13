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
use crate::revision_protection::RevisionProtector;
use crate::sha256;
use plaintext::{RevisionMetadata, verify_revision_identity};

#[path = "secure_direct_store/kernel.rs"]
mod kernel;
#[path = "control_migration_objects.rs"]
mod migration_objects;
#[path = "preparation_store.rs"]
mod preparation_store;
#[path = "secure_revision_writer.rs"]
mod revision_writer;
#[path = "secure_direct_store_storage_io.rs"]
mod storage_io;
#[path = "control_migration_context.rs"]
pub mod migration_context;

pub use kernel::DirectStore;
pub use plaintext::{
    IndexedSource, RevisionSlice, SourceSummary, StoreGap, StoreSearchResult, StoreVerification,
    StoredMatch,
};
pub use preparation_store::{PreparationBatch, PreparationCursor};

// Compatibility surface for existing store-owned revision children. Generic
// preparation-file reads continue to import `storage_io::read_regular_file`
// directly; legacy revision children receive the package-owned exact reader.
use kernel::{MAX_REVISION_OBJECT_BYTES, REVISION_DIRECTORY, verify_plaintext};
use storage_io::{
    legacy_path, protected_path, read_plaintext_path, read_revision_object as read_regular_file,
    remove_plaintext_after_readback,
};

#[path = "secure_direct_store/initialization.rs"]
mod initialization;
pub use initialization::{existing_key_binding, initialize_layout as initialize_only};

#[path = "secure_direct_store/existing_read.rs"]
mod existing_read;
pub use existing_read::{
    DirectRead, ExistingMigrationStore, ExistingReadStore, ExistingSyncStore, ReadOnlyStore,
    open_existing_migration, open_existing_read, open_existing_sync,
};

#[path = "secure_direct_store/existing_read_output.rs"]
mod existing_read_output;
pub use existing_read_output::NativeOutput;
