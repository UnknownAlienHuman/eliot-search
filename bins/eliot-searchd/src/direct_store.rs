//! Concrete development DIRECT corpus.
//!
//! The daemon owns data-root exclusion, safe reads, filesystem publication and
//! exact readback. Legacy source-event schema, digest-chain and replay planning
//! are imported from `search-source-registry`; this module does not maintain a
//! second source-registry state machine.

#[cfg(test)]
use std::fs;
use std::fs::File;
use std::path::Path;

use crate::development::MAX_SCAN_INPUT_BYTES;
use crate::sha256;

#[path = "direct_store_catalog.rs"]
mod catalog;
#[cfg(test)]
#[path = "direct_store_ingest.rs"]
mod ingest;
#[path = "direct_store/model.rs"]
mod model;
#[path = "direct_store/store.rs"]
mod store;

#[cfg(test)]
use catalog::load_registry;
pub use catalog::{RevisionMetadata, verify_revision_identity};
#[cfg(test)]
pub use model::IndexedSource;
use model::{
    CONTROL_DIRECTORY, DirectDigest, MAX_LOG_BYTES, MAX_LOG_LINE_BYTES, MAX_SOURCE_EVENTS,
    NAMESPACE_FILE, RegistryState, SOURCE_LOG_FILE, SOURCE_LOG_HEADER, SourceRecord, SourceState,
};
pub use model::{
    DirectStore, RevisionSlice, SourceSummary, StoreGap, StoreSearchResult, StoreVerification,
    StoredMatch,
};
#[cfg(test)]
use model::{FileSnapshot, MAX_DIRECTORY_FILES, RecordDraft, ZERO_DIGEST};
#[cfg(test)]
use store::{collect_regular_files, ensure_directory, path_identity_bytes, read_file_snapshot};
use store::{ensure_regular_file, is_reparse};

#[path = "direct_store/initialization.rs"]
mod initialization;
pub use initialization::{initialize_layout, verify_layout as verify_initialization_layout};
