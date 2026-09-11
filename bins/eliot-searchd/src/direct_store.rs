//! Concrete development DIRECT corpus.
//!
//! The daemon owns data-root exclusion, safe reads, filesystem publication and
//! exact readback. Legacy source-event schema, digest-chain and replay planning
//! are imported from `search-source-registry`; this module does not maintain a
//! second source-registry state machine.

use std::fs::{self, File};
use std::path::Path;

use crate::development::MAX_SCAN_INPUT_BYTES;
use crate::sha256;

#[path = "direct_store/model.rs"]
mod model;
#[path = "direct_store/store.rs"]
mod store;
#[path = "direct_store_ingest.rs"]
mod ingest;
#[path = "direct_store_catalog.rs"]
mod catalog;

use catalog::load_registry;
pub use catalog::{RevisionMetadata, verify_revision_identity};
use model::{
    CONTROL_DIRECTORY, DirectDigest, FileSnapshot, MAX_DIRECTORY_FILES,
    MAX_LOG_BYTES, MAX_LOG_LINE_BYTES, MAX_SOURCE_EVENTS, NAMESPACE_FILE, RecordDraft,
    RegistryState, SOURCE_LOG_FILE, SOURCE_LOG_HEADER, SourceRecord, SourceState,
    ZERO_DIGEST,
};
pub use model::{
    DirectStore, IndexedSource, RevisionSlice, SourceSummary, StoreGap,
    StoreSearchResult, StoreVerification, StoredMatch,
};
use store::{
    collect_regular_files, ensure_directory, ensure_regular_file, is_reparse,
    path_identity_bytes, read_file_snapshot,
};
