//! Stable checkpoint, cursor and bounded page planning for legacy revision residue.

use super::model::{
    LEGACY_REVISION_MAX_INVENTORY_OBJECTS, LegacyRevisionInventory,
    LegacyRevisionInventoryDigest, LegacyRevisionInventoryEntry,
    LegacyRevisionInventoryError,
};
use super::LegacyRevisionInventoryKind;
use super::wire::{decode_lower_hex_digest, hex};

const CHECKPOINT_DOMAIN: &[u8] = b"eliot-search/control-migration-orphans/v1";
const CURSOR_PREFIX: &str = "o1";
const MAX_CURSOR_BYTES: usize = 88;

/// Maximum object rows in one legacy orphan-inventory page.
pub const LEGACY_REVISION_MAX_PAGE_OBJECTS: usize = 32;
/// Maximum aggregate encoded object bytes represented by one page.
pub const LEGACY_REVISION_MAX_PAGE_STORED_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum canonical JSON bytes in one page response.
pub const LEGACY_REVISION_MAX_REPORT_BYTES: usize = 60 * 1024;

/// Canonical continuation cursor bound to one inventory checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyRevisionInventoryCursor {
    checkpoint: [u8; 32],
    next: usize,
}

impl LegacyRevisionInventoryCursor {
    /// Parses the exact `o1.<lower-hex-64>.<canonical-decimal>` grammar.
    pub fn parse(value: &str) -> Result<Self, LegacyRevisionInventoryError> {
        if value.len() > MAX_CURSOR_BYTES || !value.is_ascii() {
            return Err(LegacyRevisionInventoryError::CursorInvalid);
        }
        let mut parts = value.split('.');
        let prefix = parts.next();
        let checkpoint = parts.next();
        let next = parts.next();
        if prefix != Some(CURSOR_PREFIX) || parts.next().is_some() {
            return Err(LegacyRevisionInventoryError::CursorInvalid);
        }
        let checkpoint = checkpoint
            .and_then(decode_lower_hex_digest)
            .ok_or(LegacyRevisionInventoryError::CursorInvalid)?;
        let next_text = next.ok_or(LegacyRevisionInventoryError::CursorInvalid)?;
        let next = next_text
            .parse::<usize>()
            .map_err(|_| LegacyRevisionInventoryError::CursorInvalid)?;
        if next > LEGACY_REVISION_MAX_INVENTORY_OBJECTS
            || next.to_string() != next_text
        {
            return Err(LegacyRevisionInventoryError::CursorInvalid);
        }
        Ok(Self { checkpoint, next })
    }

    /// Encodes the exact canonical cursor representation.
    #[must_use]
    pub fn encode(self) -> String {
        format!("{CURSOR_PREFIX}.{}.{}", hex(&self.checkpoint), self.next)
    }

    /// Checkpoint bound by this cursor.
    #[must_use]
    pub const fn checkpoint(self) -> [u8; 32] {
        self.checkpoint
    }

    /// Number of unreferenced entries consumed before the next page.
    #[must_use]
    pub const fn next(self) -> usize {
        self.next
    }
}

/// One deterministic bounded page plan before object-content fingerprinting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyRevisionInventoryPage {
    inventory_digest: [u8; 32],
    checkpoint: [u8; 32],
    first: usize,
    next: usize,
    encoded_bytes: u64,
    exhausted: bool,
    entries: Vec<LegacyRevisionInventoryEntry>,
}

impl LegacyRevisionInventoryPage {
    /// Inventory digest this plan was selected from.
    #[must_use]
    pub const fn inventory_digest(&self) -> &[u8; 32] {
        &self.inventory_digest
    }

    /// Catalog/backend/inventory checkpoint bound by this plan.
    #[must_use]
    pub const fn checkpoint(&self) -> &[u8; 32] {
        &self.checkpoint
    }

    /// Number of unreferenced entries skipped before this page.
    #[must_use]
    pub const fn first(&self) -> usize {
        self.first
    }

    /// Number of unreferenced entries consumed through this page.
    #[must_use]
    pub const fn next(&self) -> usize {
        self.next
    }

    /// Selected physical entries in deterministic order.
    #[must_use]
    pub fn entries(&self) -> &[LegacyRevisionInventoryEntry] {
        &self.entries
    }

    /// Number of selected object rows.
    #[must_use]
    pub const fn object_count(&self) -> usize {
        self.entries.len()
    }

    /// Aggregate encoded bytes for selected entries.
    #[must_use]
    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    /// Whether no later unreferenced inventory entries remain.
    #[must_use]
    pub const fn exhausted(&self) -> bool {
        self.exhausted
    }

    /// Canonical continuation cursor, or `None` when exhausted.
    #[must_use]
    pub fn next_cursor(&self) -> Option<String> {
        (!self.exhausted).then(|| {
            LegacyRevisionInventoryCursor {
                checkpoint: self.checkpoint,
                next: self.next,
            }
            .encode()
        })
    }
}

/// Derives the inventory checkpoint from catalog, protection backend and inventory.
#[must_use]
pub fn legacy_revision_inventory_checkpoint<D: LegacyRevisionInventoryDigest>(
    catalog_snapshot: &[u8; 32],
    protection_backend: &str,
    inventory: &LegacyRevisionInventory,
) -> [u8; 32] {
    D::digest_parts(
        CHECKPOINT_DOMAIN,
        &[
            catalog_snapshot,
            protection_backend.as_bytes(),
            inventory.digest(),
        ],
    )
}

/// Selects one bounded deterministic page from an immutable inventory.
pub fn plan_legacy_revision_inventory_page(
    inventory: &LegacyRevisionInventory,
    checkpoint: [u8; 32],
    cursor: Option<&LegacyRevisionInventoryCursor>,
) -> Result<LegacyRevisionInventoryPage, LegacyRevisionInventoryError> {
    let first = cursor.map_or(0, |value| value.next);
    let count = inventory.unreferenced_count();
    if first > count
        || cursor.is_some_and(|value| value.checkpoint != checkpoint)
    {
        return Err(LegacyRevisionInventoryError::CursorStale);
    }

    let mut entries = Vec::with_capacity(LEGACY_REVISION_MAX_PAGE_OBJECTS);
    let mut encoded_bytes = 0_u64;
    for entry in inventory
        .entries()
        .iter()
        .filter(|entry| entry.kind() != LegacyRevisionInventoryKind::Referenced)
        .skip(first)
    {
        if entries.len() == LEGACY_REVISION_MAX_PAGE_OBJECTS {
            break;
        }
        let Some(next_bytes) = encoded_bytes.checked_add(entry.encoded_bytes())
        else {
            break;
        };
        if next_bytes > LEGACY_REVISION_MAX_PAGE_STORED_BYTES {
            break;
        }
        encoded_bytes = next_bytes;
        entries.push(entry.clone());
    }
    if entries.is_empty() && first != count {
        return Err(LegacyRevisionInventoryError::NoProgress);
    }
    let next = first
        .checked_add(entries.len())
        .ok_or(LegacyRevisionInventoryError::ObjectLimit)?;
    Ok(LegacyRevisionInventoryPage {
        inventory_digest: *inventory.digest(),
        checkpoint,
        first,
        next,
        encoded_bytes,
        exhausted: next == count,
        entries,
    })
}
