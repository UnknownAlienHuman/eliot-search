//! Canonical legacy revision orphan-inventory page projection.

use super::model::{
    LegacyRevisionInventory, LegacyRevisionInventoryDigest,
    LegacyRevisionInventoryEntry, LegacyRevisionInventoryError,
};
use super::page::{
    LEGACY_REVISION_MAX_REPORT_BYTES, LegacyRevisionInventoryPage,
};
use super::wire::{hex, json_string};

const PAGE_DOMAIN: &[u8] = b"eliot-search/control-migration-orphan-page/v1";

/// Exact object-content evidence associated with one selected inventory entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyRevisionObjectEvidence {
    relative_locator: String,
    encoded_sha256: [u8; 32],
}

impl LegacyRevisionObjectEvidence {
    /// Binds one exact encoded-object digest to a selected inventory entry.
    #[must_use]
    pub fn new(
        entry: &LegacyRevisionInventoryEntry,
        encoded_sha256: [u8; 32],
    ) -> Self {
        Self {
            relative_locator: entry.relative_locator().to_owned(),
            encoded_sha256,
        }
    }

    /// Shard-relative locator this evidence was read from.
    #[must_use]
    pub fn relative_locator(&self) -> &str {
        &self.relative_locator
    }

    /// SHA-256 of the exact encoded object bytes.
    #[must_use]
    pub const fn encoded_sha256(&self) -> &[u8; 32] {
        &self.encoded_sha256
    }
}

/// Renders the exact `legacy-revision-orphans-v1` operator response.
///
/// The inventory and page must remain equal to their planning snapshot, and
/// every evidence record must correspond positionally to the selected entry.
pub fn render_legacy_revision_inventory_report<D: LegacyRevisionInventoryDigest>(
    namespace_id: &str,
    catalog_snapshot: &[u8; 32],
    inventory: &LegacyRevisionInventory,
    page: &LegacyRevisionInventoryPage,
    evidence: &[LegacyRevisionObjectEvidence],
) -> Result<String, LegacyRevisionInventoryError> {
    if page.inventory_digest() != inventory.digest()
        || page.entries().len() != evidence.len()
        || page
            .entries()
            .iter()
            .zip(evidence)
            .any(|(entry, proof)| {
                entry.relative_locator() != proof.relative_locator()
            })
    {
        return Err(LegacyRevisionInventoryError::UnexpectedObject);
    }

    let mut rows = Vec::with_capacity(page.entries().len());
    for (entry, proof) in page.entries().iter().zip(evidence) {
        rows.push(format!(
            concat!(
                "{{\"object_locator\":{},\"kind\":{},",
                "\"encoded_bytes\":{},\"encoded_sha256\":\"{}\",",
                "\"catalog_referenced\":false,",
                "\"source_binding_verified\":false,",
                "\"deletion_authorized\":false}}"
            ),
            json_string(&entry.rooted_locator()),
            json_string(entry.kind().tag()),
            entry.encoded_bytes(),
            hex(proof.encoded_sha256()),
        ));
    }
    let body = rows.join(",");
    let first = u64::try_from(page.first())
        .map_err(|_| LegacyRevisionInventoryError::ObjectLimit)?;
    let next = u64::try_from(page.next())
        .map_err(|_| LegacyRevisionInventoryError::ObjectLimit)?;
    let page_digest = D::digest_parts(
        PAGE_DOMAIN,
        &[
            page.checkpoint(),
            &first.to_be_bytes(),
            &next.to_be_bytes(),
            body.as_bytes(),
        ],
    );
    let next_cursor = page
        .next_cursor()
        .map_or_else(|| "null".to_owned(), |value| json_string(&value));
    let output = format!(
        concat!(
            "{{\"event\":\"control_migration_orphans\",",
            "\"schema\":\"legacy-revision-orphans-v1\",",
            "\"scope\":\"revision_tree_only\",",
            "\"namespace_id\":{},",
            "\"catalog_snapshot_sha256\":\"{}\",",
            "\"inventory_sha256\":\"{}\",",
            "\"inventory_basis\":\"names_sizes_mtimes\",",
            "\"shards\":{},\"inventory_files\":{},",
            "\"catalog_referenced_files\":{},",
            "\"orphan_objects\":{},\"temporary_objects\":{},",
            "\"unreferenced_bytes\":{},",
            "\"after_object\":{},\"next_object\":{},",
            "\"page_objects\":{},\"page_encoded_bytes\":{},",
            "\"entries\":[{}],",
            "\"page_sha256\":\"{}\",\"next_cursor\":{},",
            "\"exhausted\":{},\"read_only\":true,",
            "\"page_encoded_bytes_hashed\":true,",
            "\"all_object_contents_hashed\":false,",
            "\"preparation_orphans_enumerated\":false,",
            "\"deletion_authorized\":false,",
            "\"cutover_revalidation_required\":true,",
            "\"canonical_mapping_complete\":false}}"
        ),
        json_string(namespace_id),
        hex(catalog_snapshot),
        hex(inventory.digest()),
        inventory.shard_count(),
        inventory.object_count(),
        inventory.referenced_count(),
        inventory.orphan_count(),
        inventory.temporary_count(),
        inventory.unreferenced_bytes(),
        page.first(),
        page.next(),
        page.object_count(),
        page.encoded_bytes(),
        body,
        hex(&page_digest),
        next_cursor,
        page.exhausted(),
    );
    if output.len() > LEGACY_REVISION_MAX_REPORT_BYTES {
        return Err(LegacyRevisionInventoryError::PageTooLarge);
    }
    Ok(output)
}
