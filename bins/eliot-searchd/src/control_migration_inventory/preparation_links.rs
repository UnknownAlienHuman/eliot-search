//! Current-profile preparation linkage using borrowed inventory and owning codecs.
use std::path::Path;
use search_materializer::api::{LegacyPreparationInventoryTree as PrepTree,
    LegacyDirectPreparationBinding, LEGACY_PREPARATION_DIRECTORY, LEGACY_PREPARATION_MAX_OBJECT_BYTES,
    decode_legacy_preparation_reference, derive_legacy_preparation_lookup_key,
    derive_legacy_preparation_object_id, encode_legacy_preparation_binding,
    legacy_preparation_reference_relative_locator, legacy_preparation_object_relative_locator};
use super::{MigrationSource, io, Work};
use super::scan::{PreparationRepresentationDigest, digest, changed};
// Reuse the current revision/profile binding and reference/object codecs. This
// bounded borrowed rescan retains no second map; an unmatched/unknown profile
// stays Gap, never a proven orphan or absence. A dangling reference is unlinked.
pub(super) fn is_linked(source: &MigrationSource<'_, '_, '_>, root: &Path,
    tree: PrepTree, physical: &[u8; 32], physical_name: &std::ffi::OsStr) -> Result<bool, String> {
    let input = source.native().ok_or_else(changed)?;
    source.charge(Work::Allocate(512))?;
    let materializer = crate::direct_preparation::canonical_materializer_digest().map_err(str::to_owned)?;
    let unitizer = crate::direct_preparation::canonical_unitizer_digest().map_err(str::to_owned)?;
    for revision in source.inventory_revisions() {
        source.charge(Work::Compare(512))?;
        let binding = encode_legacy_preparation_binding(&LegacyDirectPreparationBinding {
            namespace: source.inventory_namespace(), source_id: digest(&revision.source_id)?,
            revision_id: digest(&revision.revision_id)?, content_digest: digest(&revision.content_digest)?,
            byte_length: revision.byte_length, materializer_digest: materializer, unitizer_digest: unitizer,
        });
        let key = derive_legacy_preparation_lookup_key::<PreparationRepresentationDigest>(&binding,
            input.protector.backend_name());
        if tree == PrepTree::References && &key != physical { continue; }
        source.charge(Work::AllocateLocator(2048))?;
        let path = root.join(LEGACY_PREPARATION_DIRECTORY)
            .join(legacy_preparation_reference_relative_locator(&key));
        let Ok(bytes) = io::read(source, &path, LEGACY_PREPARATION_MAX_OBJECT_BYTES)? else { continue; };
        let Ok(reference) = decode_legacy_preparation_reference(&bytes, &key) else { continue; };
        let object = derive_legacy_preparation_object_id::<PreparationRepresentationDigest>(&binding,
            input.protector.backend_name(), reference.manifest_digest());
        if tree == PrepTree::Objects {
            let relative = legacy_preparation_object_relative_locator(&object, reference.protection());
            if &object == physical && Path::new(&relative).file_name() == Some(physical_name) { return Ok(true); }
            continue;
        }
        source.charge(Work::AllocateLocator(2048))?;
        let target = root.join(LEGACY_PREPARATION_DIRECTORY)
            .join(legacy_preparation_object_relative_locator(&object, reference.protection()));
        // Presence alone cannot resolve intermediate parents. Use the same
        // pinned no-follow, single-link reader before accepting a target link.
        return Ok(io::read(source, &target, LEGACY_PREPARATION_MAX_OBJECT_BYTES)?.is_ok());
    }
    Ok(false)
}

