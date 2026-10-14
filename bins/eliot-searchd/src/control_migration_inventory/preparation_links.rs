//! Current-profile preparation links derived once per complete inventory pass.
use std::ffi::{OsStr, OsString};
use std::path::Path;
use search_materializer::api::{LegacyPreparationInventoryTree as PrepTree,
    LegacyDirectPreparationBinding, LEGACY_PREPARATION_DIRECTORY, LEGACY_PREPARATION_MAX_OBJECT_BYTES,
    decode_legacy_preparation_reference, derive_legacy_preparation_lookup_key,
    derive_legacy_preparation_object_id, encode_legacy_preparation_binding,
    legacy_preparation_object_relative_locator, legacy_preparation_reference_relative_locator};
use super::{MigrationSource, io, Work};
use super::scan::{PreparationRepresentationDigest, digest, changed};

pub(super) const MAX_LINKS: usize = 2_000_000;

pub(super) struct View {
    references: Vec<[u8; 32]>,
    objects: Vec<([u8; 32], OsString)>,
    object_key_bytes: usize,
}

impl View {
    fn reference(&self, physical: &[u8; 32]) -> bool {
        self.references.binary_search(physical).is_ok()
    }
    fn object(&self, physical: &[u8; 32], name: &OsStr) -> bool {
        self.objects.binary_search_by(|(id, candidate)|
            id.cmp(physical).then_with(|| candidate.as_os_str().cmp(name))).is_ok()
    }
}

fn push<T>(source: &MigrationSource<'_, '_, '_>, values: &mut Vec<T>, value: T) -> Result<(), String> {
    if values.len() == MAX_LINKS { return Err(limit()); }
    if values.len() == values.capacity() {
        let capacity = values.capacity().saturating_mul(2).clamp(32, MAX_LINKS);
        let bytes = capacity.checked_mul(std::mem::size_of::<T>()).ok_or_else(limit)?;
        source.charge(Work::Allocate(bytes))?;
        values.try_reserve_exact(capacity - values.len()).map_err(|_| limit())?;
    }
    values.push(value);
    Ok(())
}

fn limit() -> String { "DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned() }

pub(super) fn view(source: &MigrationSource<'_, '_, '_>, root: &Path) -> Result<View, String> {
    let input = source.native().ok_or_else(changed)?;
    let locator_bytes = root.as_os_str().len().checked_add(2048)
        .and_then(|bytes| bytes.checked_mul(2)).ok_or_else(limit)?;
    let materializer = crate::direct_preparation::canonical_materializer_digest().map_err(str::to_owned)?;
    let unitizer = crate::direct_preparation::canonical_unitizer_digest().map_err(str::to_owned)?;
    let mut references = Vec::new();
    let mut objects = Vec::new();
    for revision in source.inventory_revisions() {
        source.charge(Work::Allocate(512))?;
        source.charge(Work::Compare(512))?;
        let binding = encode_legacy_preparation_binding(&LegacyDirectPreparationBinding {
            namespace: source.inventory_namespace(), source_id: digest(&revision.source_id)?,
            revision_id: digest(&revision.revision_id)?, content_digest: digest(&revision.content_digest)?,
            byte_length: revision.byte_length, materializer_digest: materializer, unitizer_digest: unitizer,
        });
        let key = derive_legacy_preparation_lookup_key::<PreparationRepresentationDigest>(&binding,
            input.protector.backend_name());
        source.charge(Work::AllocateLocator(locator_bytes))?;
        let path = root.join(LEGACY_PREPARATION_DIRECTORY)
            .join(legacy_preparation_reference_relative_locator(&key));
        let Ok(bytes) = io::read(source, &path, LEGACY_PREPARATION_MAX_OBJECT_BYTES)? else { continue; };
        let Ok(reference) = decode_legacy_preparation_reference(&bytes, &key) else { continue; };
        let object = derive_legacy_preparation_object_id::<PreparationRepresentationDigest>(&binding,
            input.protector.backend_name(), reference.manifest_digest());
        source.charge(Work::AllocateLocator(locator_bytes))?;
        let relative = legacy_preparation_object_relative_locator(&object, reference.protection());
        let target = root.join(LEGACY_PREPARATION_DIRECTORY).join(&relative);
        // An expected reference key alone is insufficient. Preserve the old
        // pinned target-read invariant; unresolved physical lookup remains Gap.
        if io::read(source, &target, LEGACY_PREPARATION_MAX_OBJECT_BYTES)?.is_err() { continue; }
        let name = Path::new(&relative).file_name().ok_or_else(changed)?.to_os_string();
        push(source, &mut references, key)?;
        push(source, &mut objects, (object, name))?;
    }
    io::order_metadata(source, &mut references, 32, Ord::cmp)?;
    references.dedup();
    let name_bytes = objects.iter().map(|(_, name)| name.len().saturating_mul(2)).max().unwrap_or(0);
    io::order_metadata(source, &mut objects, 32 + name_bytes,
        |left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)))?;
    objects.dedup();
    Ok(View { references, objects, object_key_bytes: 32 + name_bytes })
}

pub(super) fn is_linked(source: &MigrationSource<'_, '_, '_>, view: &View,
    tree: PrepTree, physical: &[u8; 32], name: &OsStr,
) -> Result<bool, String> {
    let (count, key_bytes) = match tree {
        PrepTree::References => (view.references.len(), 32),
        PrepTree::Objects => (view.objects.len(), view.object_key_bytes),
    };
    // std's shrinking interval can require ceil(log2(n)) probes and one
    // final comparison; floor(log2(n)) undercounts non-powers of two.
    let comparisons = if count < 2 { count } else {
        usize::try_from((count - 1).ilog2() + 2).map_err(|_| limit())?
    };
    source.charge(Work::Compare(2 * key_bytes * comparisons))?;
    Ok(match tree {
        PrepTree::References => view.reference(physical),
        PrepTree::Objects => view.object(physical, name),
    })
}
