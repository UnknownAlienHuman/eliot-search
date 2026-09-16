//! Inactive, typed source-mapping imports. This file format is deliberately not
//! a `ControlJournal`: it carries no live owner, admission, visibility or H5 receipt.
//!
//! The caller owns input exclusion and source replay. This module owns the
//! per-artifact output lock, immutable record publication, pending/final
//! database lifecycle and exact import/readback state machines. Resuming an
//! incomplete target verifies its entire committed prefix before appending rows.

use redb::TableDefinition;

use crate::ControlError;

mod codec;
mod content;
mod cutover;
mod mapping;
mod model;
mod output_artifact;
mod output_lock;
mod readback;
mod record_artifact;
mod record_chain;
mod writer;

pub use content::{
    SourceContentManifest, SourceContentManifestEncoder,
    SourceContentManifestEncodingError, SourceContentManifestHeader,
    SourceContentManifestSummary, SourceContentObjectReadback,
    source_content_profile_digest,
};
pub use cutover::{
    CONTROL_CUTOVER_MARKER_FILE, CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA,
    ControlCutoverMarker, ControlCutoverMarkerError,
    ControlCutoverReplayDecision, MAX_CONTROL_CUTOVER_MARKER_BYTES,
    classify_control_cutover_replay,
};
pub use mapping::{
    LegacySourceMappingEvent, LegacySourceMappingState, MappedSourceEvent,
    SourceMappingError, SourceMappingHeader, SourceMappingPlanner,
    SourceMappingSummary, source_mapping_profile_digest,
};
pub use model::{
    SourceImportBinding, SourceImportCounts, SourceImportRow,
    SourceLifecycleFlags,
};
pub use output_artifact::{
    SourceImportOutputArtifact, SourceImportOutputArtifactError,
    SourceImportOutputArtifactPlatform, SourceImportPendingArtifact,
    SourceImportPublishedArtifact,
};
pub use output_lock::{
    SourceImportOutputLock, SourceImportOutputLockError,
    SourceImportOutputLockPlatform,
};
pub use readback::SourceMappingReadback;
pub use record_artifact::{
    SourceImportFrozenRecordArtifact, SourceImportPublishedRecordArtifact,
    SourceImportRecordArtifact, SourceImportRecordArtifactError,
    SourceImportRecordArtifactObservation, SourceImportRecordReadback,
    SourceImportVerifiedRecordArtifact, inspect_source_import_record_artifact,
};
pub use record_chain::{
    MAX_SOURCE_IMPORT_RECORD_BYTES, MAX_SOURCE_IMPORT_ROW_BYTES,
    SourceImportRecordChain, SourceImportRecordChainError,
};
pub use writer::SourceMappingImport;

const META: TableDefinition<&str, &[u8]> =
    TableDefinition::new("eliot.import.source-map.meta.v1");
const EVENTS: TableDefinition<u64, &[u8]> =
    TableDefinition::new("eliot.import.source-map.events.v1");
const SOURCES: TableDefinition<&[u8], u64> =
    TableDefinition::new("eliot.import.source-map.sources.v1");
const REVISIONS: TableDefinition<&[u8], u64> =
    TableDefinition::new("eliot.import.source-map.revisions.v1");
const MAX_ROWS: u64 = 2_000_000;
const MAX_BYTES: u64 = 512 * 1024 * 1024;
const BATCH_ROWS: usize = 256;
const ROW_BYTES: usize = 83 + 9 * 32;
const HASH_BASE: usize = 83;
