//! Inactive, typed source-mapping imports. This file format is deliberately not
//! a `ControlJournal`: it carries no live owner, admission, visibility or H5 receipt.
//!
//! The caller owns input exclusion and the admitted output file. Resuming an
//! incomplete target verifies its entire committed prefix before appending rows.

use redb::TableDefinition;

use crate::ControlError;

mod codec;
mod content;
mod mapping;
mod model;
mod readback;
mod writer;

pub use content::SourceContentManifest;
pub use mapping::{
    LegacySourceMappingEvent, LegacySourceMappingState, MappedSourceEvent,
    SourceMappingError, SourceMappingHeader, SourceMappingPlanner,
    SourceMappingSummary, source_mapping_profile_digest,
};
pub use model::{
    SourceImportBinding, SourceImportCounts, SourceImportRow,
    SourceLifecycleFlags,
};
pub use readback::SourceMappingReadback;
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
