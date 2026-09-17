//! Deterministic legacy revision-inventory model and digest preimages.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::{
    LegacyRevisionInventoryKind, LegacyRevisionPhysicalKind,
    LEGACY_REVISION_MAX_OBJECT_BYTES, classify_legacy_revision_inventory_name,
    is_legacy_revision_inventory_shard,
    legacy_revision_rooted_locator,
};

const INVENTORY_DOMAIN: &[u8] = b"eliot-search/revision-residue-inventory/v1";
const SHARD_DOMAIN: &[u8] = b"eliot-search/revision-residue-shard/v1";
const FILE_DOMAIN: &[u8] = b"eliot-search/revision-residue-file/v1";
const NANOS_PER_SECOND: u32 = 1_000_000_000;

/// Maximum admitted shard directories in one legacy revision inventory.
pub const LEGACY_REVISION_MAX_INVENTORY_SHARDS: usize = 256;
/// Maximum admitted physical objects in one legacy revision inventory.
pub const LEGACY_REVISION_MAX_INVENTORY_OBJECTS: usize = 65_536;

/// Composition-supplied implementation of the frozen SHA-256 multipart profile.
pub trait LegacyRevisionInventoryDigest {
    /// Hashes one domain-separated ordered-parts preimage.
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32];
}

/// Closed pure-model failures for legacy revision inventory construction and paging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyRevisionInventoryError {
    /// A shard, locator, timestamp or physical/classification pairing is invalid.
    UnexpectedObject,
    /// The inventory contains too many shard directories.
    ShardLimit,
    /// The inventory contains too many physical objects.
    ObjectLimit,
    /// Aggregate unreferenced encoded bytes overflowed the accepted counter.
    BytesExceeded,
    /// A continuation cursor is malformed or noncanonical.
    CursorInvalid,
    /// A continuation cursor is bound to another inventory checkpoint.
    CursorStale,
    /// A non-exhausted page cannot admit even one object under the fixed bounds.
    NoProgress,
    /// The canonical report exceeds its fixed response bound.
    PageTooLarge,
}

impl LegacyRevisionInventoryError {
    /// Stable daemon-compatible reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnexpectedObject => "DIRECT_MIGRATION_UNEXPECTED_REVISION_OBJECT",
            Self::ShardLimit => "DIRECT_MIGRATION_SHARD_LIMIT",
            Self::ObjectLimit => "DIRECT_MIGRATION_OBJECT_LIMIT",
            Self::BytesExceeded => "DIRECT_MIGRATION_BYTES_EXCEEDED",
            Self::CursorInvalid => "DIRECT_MIGRATION_ORPHAN_CURSOR_INVALID",
            Self::CursorStale => "DIRECT_MIGRATION_ORPHAN_CURSOR_STALE",
            Self::NoProgress => "DIRECT_MIGRATION_NO_PROGRESS",
            Self::PageTooLarge => "DIRECT_MIGRATION_PAGE_TOO_LARGE",
        }
    }
}

impl fmt::Display for LegacyRevisionInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LegacyRevisionInventoryError {}

/// One normalized physical observation in the legacy revision tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyRevisionInventoryEntry {
    relative_locator: String,
    encoded_bytes: u64,
    modified_seconds: u64,
    modified_nanos: u32,
    kind: LegacyRevisionInventoryKind,
}

impl LegacyRevisionInventoryEntry {
    /// Creates one validated normalized physical observation.
    pub fn new(
        relative_locator: String,
        encoded_bytes: u64,
        modified_seconds: u64,
        modified_nanos: u32,
        kind: LegacyRevisionInventoryKind,
    ) -> Result<Self, LegacyRevisionInventoryError> {
        let maximum_bytes =
            u64::try_from(LEGACY_REVISION_MAX_OBJECT_BYTES).unwrap_or(u64::MAX);
        let (_, name) = relative_locator
            .split_once('/')
            .ok_or(LegacyRevisionInventoryError::UnexpectedObject)?;
        let classified = classify_legacy_revision_inventory_name(name)
            .ok_or(LegacyRevisionInventoryError::UnexpectedObject)?;
        let kind_matches = matches!(
            (classified.physical_kind(), kind),
            (
                LegacyRevisionPhysicalKind::Temporary,
                LegacyRevisionInventoryKind::Temporary
            ) | (
                LegacyRevisionPhysicalKind::Final(_),
                LegacyRevisionInventoryKind::Referenced
                    | LegacyRevisionInventoryKind::Orphan
            )
        );
        if !kind_matches
            || legacy_revision_rooted_locator(&relative_locator).is_none()
            || encoded_bytes > maximum_bytes
            || modified_nanos >= NANOS_PER_SECOND
        {
            return Err(LegacyRevisionInventoryError::UnexpectedObject);
        }
        Ok(Self {
            relative_locator,
            encoded_bytes,
            modified_seconds,
            modified_nanos,
            kind,
        })
    }

    /// Canonical shard-relative object locator.
    #[must_use]
    pub fn relative_locator(&self) -> &str {
        &self.relative_locator
    }

    /// Canonical data-root-relative object locator.
    #[must_use]
    pub fn rooted_locator(&self) -> String {
        format!(
            "{}/{}",
            super::LEGACY_REVISION_DIRECTORY,
            self.relative_locator
        )
    }

    /// Encoded object length observed from qualified metadata.
    #[must_use]
    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    /// Whole seconds since the Unix epoch from qualified metadata.
    #[must_use]
    pub const fn modified_seconds(&self) -> u64 {
        self.modified_seconds
    }

    /// Subsecond nanoseconds from qualified metadata.
    #[must_use]
    pub const fn modified_nanos(&self) -> u32 {
        self.modified_nanos
    }

    /// Catalog-overlay inventory classification.
    #[must_use]
    pub const fn kind(&self) -> LegacyRevisionInventoryKind {
        self.kind
    }

    pub(super) fn shard(&self) -> &str {
        self.relative_locator
            .split_once('/')
            .map_or("", |(shard, _)| shard)
    }
}

/// One immutable deterministic legacy revision inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyRevisionInventory {
    shards: Vec<String>,
    entries: Vec<LegacyRevisionInventoryEntry>,
    digest: [u8; 32],
    referenced_count: usize,
    orphan_count: usize,
    temporary_count: usize,
    unreferenced_bytes: u64,
}

impl LegacyRevisionInventory {
    /// Sorted admitted shard names.
    #[must_use]
    pub fn shards(&self) -> &[String] {
        &self.shards
    }

    /// Sorted normalized physical entries.
    #[must_use]
    pub fn entries(&self) -> &[LegacyRevisionInventoryEntry] {
        &self.entries
    }

    /// Frozen names/sizes/mtimes/classifications digest.
    #[must_use]
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Number of admitted shard directories.
    #[must_use]
    pub const fn shard_count(&self) -> usize {
        self.shards.len()
    }

    /// Number of admitted physical objects.
    #[must_use]
    pub const fn object_count(&self) -> usize {
        self.entries.len()
    }

    /// Number of final objects referenced by the current catalog overlay.
    #[must_use]
    pub const fn referenced_count(&self) -> usize {
        self.referenced_count
    }

    /// Number of final objects not referenced by the current catalog overlay.
    #[must_use]
    pub const fn orphan_count(&self) -> usize {
        self.orphan_count
    }

    /// Number of admitted writer-temporary objects.
    #[must_use]
    pub const fn temporary_count(&self) -> usize {
        self.temporary_count
    }

    /// Aggregate encoded bytes for orphan and temporary objects.
    #[must_use]
    pub const fn unreferenced_bytes(&self) -> u64 {
        self.unreferenced_bytes
    }

    pub(super) const fn unreferenced_count(&self) -> usize {
        self.orphan_count + self.temporary_count
    }
}

/// Builds the deterministic inventory from qualified physical observations.
///
/// Shards and entries are sorted internally. Duplicate shards/locators,
/// missing shard parents, malformed timestamps, inconsistent physical classes
/// and all fixed-limit violations fail closed.
pub fn build_legacy_revision_inventory<D: LegacyRevisionInventoryDigest>(
    shards: Vec<String>,
    entries: Vec<LegacyRevisionInventoryEntry>,
) -> Result<LegacyRevisionInventory, LegacyRevisionInventoryError> {
    let mut sorted_shards = BTreeSet::new();
    for shard in shards {
        if sorted_shards.len() >= LEGACY_REVISION_MAX_INVENTORY_SHARDS {
            return Err(LegacyRevisionInventoryError::ShardLimit);
        }
        if !is_legacy_revision_inventory_shard(&shard)
            || !sorted_shards.insert(shard)
        {
            return Err(LegacyRevisionInventoryError::UnexpectedObject);
        }
    }

    let mut sorted_entries = BTreeMap::new();
    for entry in entries {
        if sorted_entries.len() >= LEGACY_REVISION_MAX_INVENTORY_OBJECTS {
            return Err(LegacyRevisionInventoryError::ObjectLimit);
        }
        if !sorted_shards.contains(entry.shard())
            || sorted_entries
                .insert(entry.relative_locator.clone(), entry)
                .is_some()
        {
            return Err(LegacyRevisionInventoryError::UnexpectedObject);
        }
    }

    let mut digest = D::digest_parts(INVENTORY_DOMAIN, &[]);
    for shard in &sorted_shards {
        digest = D::digest_parts(SHARD_DOMAIN, &[&digest, shard.as_bytes()]);
    }

    let mut referenced_count = 0_usize;
    let mut orphan_count = 0_usize;
    let mut temporary_count = 0_usize;
    let mut unreferenced_bytes = 0_u64;
    for entry in sorted_entries.values() {
        digest = D::digest_parts(
            FILE_DOMAIN,
            &[
                &digest,
                entry.relative_locator.as_bytes(),
                &entry.encoded_bytes.to_be_bytes(),
                &entry.modified_seconds.to_be_bytes(),
                &entry.modified_nanos.to_be_bytes(),
                entry.kind.tag().as_bytes(),
            ],
        );
        match entry.kind {
            LegacyRevisionInventoryKind::Referenced => referenced_count += 1,
            LegacyRevisionInventoryKind::Orphan => {
                orphan_count += 1;
                unreferenced_bytes = unreferenced_bytes
                    .checked_add(entry.encoded_bytes)
                    .ok_or(LegacyRevisionInventoryError::BytesExceeded)?;
            }
            LegacyRevisionInventoryKind::Temporary => {
                temporary_count += 1;
                unreferenced_bytes = unreferenced_bytes
                    .checked_add(entry.encoded_bytes)
                    .ok_or(LegacyRevisionInventoryError::BytesExceeded)?;
            }
        }
    }

    Ok(LegacyRevisionInventory {
        shards: sorted_shards.into_iter().collect(),
        entries: sorted_entries.into_values().collect(),
        digest,
        referenced_count,
        orphan_count,
        temporary_count,
        unreferenced_bytes,
    })
}
