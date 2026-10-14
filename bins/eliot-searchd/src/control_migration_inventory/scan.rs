//! The twelve concrete inventory denominators; no whole-inventory collection.

use super::{Family, NativeMigrationInput, Status, Stream, Work, io, sha256};
use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{LegacyInventoryLimits, LegacyInventoryProfile};
use search_materializer::api::{
    LEGACY_PREPARATION_BINDING_BYTES, LEGACY_PREPARATION_DIRECTORY,
    LEGACY_PREPARATION_MAX_OBJECT_BYTES, LEGACY_PREPARATION_REFERENCE_BYTES,
    LegacyDirectPreparationBinding, LegacyDirectRepresentationDigest,
    LegacyPreparationInventoryTree as PrepTree, LegacyPreparationProtection,
    LegacyPreparationStoreDigest, classify_legacy_preparation_inventory_name,
    decode_legacy_preparation_reference, derive_legacy_preparation_lookup_key,
    derive_legacy_preparation_object_id, encode_legacy_preparation_binding,
    legacy_preparation_object_relative_locator, legacy_preparation_reference_relative_locator,
    verify_legacy_preparation_manifest, verify_legacy_preparation_payload,
};
use search_revision_store::{
    LEGACY_REVISION_DIRECTORY, LEGACY_REVISION_MAX_OBJECT_BYTES, LegacyRevisionPhysicalKind,
    classify_legacy_revision_inventory_name, is_legacy_revision_inventory_shard,
};
use std::path::Path;

const ZERO: [u8; 32] = [0; 32];

pub(super) fn family(
    stream: &mut Stream<'_, '_, '_, '_>,
    family: Family,
    root: &Path,
    target: SourceNamespaceId,
    input: &NativeMigrationInput<'_, '_, '_>,
) -> Result<(), String> {
    match family {
        Family::Owner => stream.item(
            family,
            Status::Present,
            1,
            [ZERO; 3],
            [
                input.inventory_owner.epoch,
                input.inventory_owner.generation,
                1,
            ],
        ),
        Family::Sources => sources(stream, target),
        Family::Roots => roots(stream, root, input),
        Family::DirectoryManifests => manifests(stream, root, target),
        Family::Revisions => revisions(stream, root, input),
        Family::Preparation => preparation(stream, root, Family::Preparation, false),
        Family::Residue => residue(stream, root),
        Family::Operations => {
            control(stream, root, family)?;
            // V1 effect IDs are catalog-derived, not original invocation receipts.
            stream.item(family, Status::Gap, 348, [ZERO; 3], [348, 0, 0])
        }
        Family::Publications | Family::Security => control(stream, root, family),
        Family::Credentials => {
            let binding = input
                .protector
                .initialization_key_binding()
                .map_err(|error| error.code().to_owned())?;
            stream.item(
                family,
                Status::Present,
                20,
                [binding, ZERO, ZERO],
                [0, 0, 0],
            )?;
            stream.item(
                family,
                Status::Present,
                21,
                [crate::direct_preparation::profile_digest(), ZERO, ZERO],
                [1, 1, 0],
            )
        }
        Family::Limits => limits(stream),
    }
}

fn limits(stream: &mut Stream<'_, '_, '_, '_>) -> Result<(), String> {
    stream.item(
            Family::Limits,
            Status::Present,
            22,
            [*LegacyInventoryProfile::LocationV2.digest().as_bytes(), ZERO, ZERO],
            [
                LegacyInventoryLimits::BASELINE.records,
                LegacyInventoryLimits::BASELINE.encoded_bytes,
                LegacyInventoryLimits::BASELINE.page_names as u64,
            ],
        )?;
    stream.item(Family::Limits, Status::Present, 30, [ZERO; 3],
        [search_control_redb::migration::MAX_SOURCE_IMPORT_ROW_BYTES as u64,
            io::NAME_BYTES as u64, io::CHUNK as u64])?;
    stream.item(Family::Limits, Status::Present, 31, [ZERO; 3],
        [io::CONTROL_BYTES as u64, LEGACY_REVISION_MAX_OBJECT_BYTES as u64,
            LEGACY_PREPARATION_MAX_OBJECT_BYTES as u64])?;
    stream.item(Family::Limits, Status::Present, 32, [ZERO; 3],
        [io::MAX_PARENTS as u64,
            search_source_registry::LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS as u64,
            search_source_registry::LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES as u64])?;
    stream.item(Family::Limits, Status::Present, 33, [ZERO; 3],
        [io::DIRECTORY_NAMES as u64, super::preparation_links::MAX_LINKS as u64,
            (2 * std::mem::size_of::<usize>()) as u64])
}

fn sources(stream: &mut Stream<'_, '_, '_, '_>, target: SourceNamespaceId) -> Result<(), String> {
    let source = stream.source;
    let header = source.source_mapping_header(target)?;
    stream.item(
        Family::Sources,
        Status::Present,
        2,
        [header.legacy_namespace, header.catalog_snapshot, ZERO],
        [header.expected_events, header.expected_sources, 0],
    )?;
    source.compile_source_mapping_with_rows(
        target,
        source.native().ok_or_else(changed)?.context.deadline(),
        |_| source.checkpoint(),
        |row| source_event(stream, &row),
    )?;
    source_heads(stream)
}

fn source_event(stream: &mut Stream<'_, '_, '_, '_>, row: &search_control_redb::migration::SourceImportRow) -> Result<(), String> {
    stream.reserve_row()?;
    let encoded = stream.encoder.source_event(row).map_err(super::codec_reason)?;
    stream.statuses[Status::Present as usize] += 1;
    stream.send(&encoded)?;
    if row.lifecycle.opens_revision() {
        stream.item(Family::Sources, Status::Present, 3,
            [*row.legacy_source.as_bytes(), *row.legacy_revision.as_bytes(), *row.event.as_bytes()],
            [row.occurrence, row.sequence, row.source_bytes])?;
    }
    Ok(())
}

pub(super) fn sources_from_mapping(
    stream: &mut Stream<'_, '_, '_, '_>, target: SourceNamespaceId,
    input: &NativeMigrationInput<'_, '_, '_>, mapping: &mut super::super::source_stage::PreparedSourceMap,
) -> Result<(), String> {
    let source = stream.source;
    let header = source.source_mapping_header(target)?;
    stream.item(Family::Sources, Status::Present, 2,
        [header.legacy_namespace, header.catalog_snapshot, ZERO],
        [header.expected_events, header.expected_sources, 0])?;
    mapping.visit(source, input, |row| source_event(stream, &row))?;
    source_heads(stream)
}

fn source_heads(stream: &mut Stream<'_, '_, '_, '_>) -> Result<(), String> {
    let source = stream.source;
    for head in source.inventory_heads() {
        source.charge(Work::Compare(192))?;
        stream.item(
            Family::Sources,
            Status::Present,
            4,
            [
                digest(&head.source_id)?,
                digest(&head.revision_id)?,
                digest(&head.record_digest)?,
            ],
            [
                head.sequence,
                head.byte_length,
                u64::from(head.state.tag() == "A"),
            ],
        )?;
    }
    Ok(())
}

fn roots(
    stream: &mut Stream<'_, '_, '_, '_>,
    root: &Path,
    input: &NativeMigrationInput<'_, '_, '_>,
) -> Result<(), String> {
    let source = stream.source;
    let current = io::read(source, &root.join("control/source-roots.v1"), 64 * 1024)?;
    match (current, input.registration) {
        (Ok(current), Some(original)) if current.as_slice() == original => {
            let raw = io::fingerprint(source, original)?;
            stream.item(
                Family::Roots,
                Status::Present,
                5,
                [raw, ZERO, ZERO],
                [original.len() as u64, 0, 0],
            )?;
            source.charge(Work::Allocate(original.len().saturating_mul(2)))?;
            source.charge(Work::Compare(original.len()))?;
            let decoded = crate::source_roots::migration_input_from_bytes(root, Some(original))
                .map_err(|_| changed())?;
            for path in &decoded.paths {
                source.charge(Work::Allocate(2048))?;
                let native = io::native_name(path.as_os_str());
                let reference = io::fingerprint(source, &native)?;
                stream.item(
                    Family::Roots,
                    Status::Present,
                    6,
                    [reference, raw, ZERO],
                    [0, 0, 0],
                )?;
            }
        }
        (Err(Status::AbsentProven), None) => {
            stream.item(Family::Roots, Status::AbsentProven, 5, [ZERO; 3], [0; 3])?;
        }
        _ => return Err(changed()),
    }
    Ok(())
}

fn manifests(
    stream: &mut Stream<'_, '_, '_, '_>,
    root: &Path,
    target: SourceNamespaceId,
) -> Result<(), String> {
    let source = stream.source;
    let directory = root.join("control/directory-manifests");
    let namespace = source.source_mapping_header(target)?.legacy_namespace;
    let outcome = io::visit(source, &directory, |name| {
        let path = directory.join(&name.value);
        let subject = io::locator(source, &path)?;
        match io::read(source, &path, io::CONTROL_BYTES)? {
            Ok(bytes) => {
                let evidence = io::fingerprint(source, &bytes)?;
                source.charge(Work::Compare(bytes.len().saturating_mul(2)))?;
                source.charge(Work::Allocate(8192))?;
                // Validate the complete digest/order before emitting any bindings.
                match crate::directory_manifest::inventory_manifest(
                    &path,
                    &bytes,
                    &namespace,
                    |_| Ok(()),
                ) {
                    Ok((generation, count, directory_id, manifest)) => {
                        stream.item(
                            Family::DirectoryManifests,
                            Status::Present,
                            7,
                            [subject, evidence, manifest],
                            [generation, count, bytes.len() as u64],
                        )?;
                        crate::directory_manifest::inventory_manifest(
                            &path,
                            &bytes,
                            &namespace,
                            |refs| {
                                stream.item(
                                    Family::DirectoryManifests,
                                    Status::Present,
                                    8,
                                    refs,
                                    [generation, 0, 0],
                                )
                            },
                        )?;
                        let _ = directory_id;
                    }
                    Err(_) => stream.item(
                        Family::DirectoryManifests,
                        Status::Corrupt,
                        7,
                        [subject, evidence, ZERO],
                        [bytes.len() as u64, 0, 0],
                    )?,
                }
            }
            Err(status) => stream.item(
                Family::DirectoryManifests,
                status,
                7,
                [subject, ZERO, ZERO],
                [0; 3],
            )?,
        }
        Ok(())
    })?;
    directory_outcome(stream, Family::DirectoryManifests, &directory, outcome)?;
    // Physical exhaustion cannot prove an entirely lost historical generation.
    stream.item(
        Family::DirectoryManifests,
        Status::Gap,
        23,
        [ZERO; 3],
        [0; 3],
    )
}

fn revisions(
    stream: &mut Stream<'_, '_, '_, '_>,
    root: &Path,
    input: &NativeMigrationInput<'_, '_, '_>,
) -> Result<(), String> {
    let source = stream.source;
    for revision in source.inventory_revisions() {
        source.charge(Work::Allocate(512))?;
        let revision = super::super::super::RevisionMetadata::from(revision);
        let id = digest(&revision.revision_id)?;
        let verified = super::super::super::read_import_revision_with_context(
            root,
            Some(input.protector),
            &revision,
            input.context,
        );
        // A native/context refusal is terminal, not a row claiming corrupt bytes.
        source.checkpoint()?;
        let status = match verified {
            Ok(bytes) => {
                drop(bytes);
                Status::Present
            }
            Err(ref reason)
                if matches!(
                    reason.as_str(),
                    "DIRECT_REVISION_OBJECT_READ_FAILED"
                        | "DIRECT_REVISION_OBJECT_INSPECTION_FAILED"
                        | "DIRECT_REVISION_KEY_UNAVAILABLE"
                        | "DIRECT_REVISION_ENCRYPTION_UNAVAILABLE"
                ) =>
            {
                Status::Unreadable
            }
            Err(ref reason) if reason == "DIRECT_REVISION_MISSING" => Status::Gap,
            Err(_) => Status::Corrupt,
        };
        stream.item(
            Family::Revisions,
            status,
            9,
            [
                id,
                digest(&revision.content_digest)?,
                digest(&revision.source_id)?,
            ],
            [revision.byte_length, 0, 0],
        )?;
    }
    revision_tree(stream, root, Family::Revisions, false)?;
    if stream.statuses.iter().all(|count| *count == 0) {
        stream.item(
            Family::Revisions,
            Status::AbsentProven,
            9,
            [ZERO; 3],
            [0; 3],
        )?;
    }
    Ok(())
}

pub(super) fn revisions_from_content(
    stream: &mut Stream<'_, '_, '_, '_>,
    root: &Path,
    input: &NativeMigrationInput<'_, '_, '_>,
    content: &mut super::super::content_readback::PreparedContent,
) -> Result<(), String> {
    let source = stream.source;
    let mut revisions = source.inventory_revisions();
    content.visit(source, input.context.deadline(), |object| {
        source.charge(Work::Compare(512))?;
        let revision = revisions.next().ok_or_else(changed)?;
        let id = digest(&revision.revision_id)?;
        let sha = digest(&revision.content_digest)?;
        let source_id = digest(&revision.source_id)?;
        if object.legacy_revision_id.as_bytes() != &id
            || object.content_sha256.as_bytes() != &sha
            || object.legacy_source_id.as_bytes() != &source_id
            || object.byte_length != revision.byte_length
        {
            return Err(changed());
        }
        stream.item(Family::Revisions, Status::Present, 9,
            [id, sha, source_id], [revision.byte_length, 0, 0])
    })?;
    if revisions.next().is_some() { return Err(changed()); }
    revision_tree(stream, root, Family::Revisions, false)?;
    if stream.statuses.iter().all(|count| *count == 0) {
        stream.item(Family::Revisions, Status::AbsentProven, 9, [ZERO; 3], [0; 3])?;
    }
    Ok(())
}

fn revision_tree(
    stream: &mut Stream<'_, '_, '_, '_>,
    root: &Path,
    family: Family,
    residue_only: bool,
) -> Result<(), String> {
    let source = stream.source;
    let directory = root.join(LEGACY_REVISION_DIRECTORY);
    let outcome = io::visit(source, &directory, |shard| {
        let path = directory.join(&shard.value);
        if !shard
            .value
            .to_str()
            .is_some_and(is_legacy_revision_inventory_shard)
        {
            return stream.item(
                family,
                Status::Unsupported,
                10,
                [io::locator(source, &path)?, ZERO, ZERO],
                [0; 3],
            );
        }
        let outcome = io::visit(source, &path, |name| {
            let subject = io::child_locator(source, &path, &name.value)?;
            let classified = name
                .value
                .to_str()
                .and_then(classify_legacy_revision_inventory_name);
            let (status, kind, reference) = match classified {
                Some(value) => {
                    if !shard
                        .value
                        .to_str()
                        .is_some_and(|shard| value.id().starts_with(shard))
                    {
                        return stream.item(
                            family,
                            Status::Corrupt,
                            10,
                            [subject, ZERO, digest(value.id())?],
                            [0; 3],
                        );
                    }
                    let referenced = source.retained_revision(value.id()).is_some();
                    let temporary = value.physical_kind() == LegacyRevisionPhysicalKind::Temporary;
                    if residue_only && referenced && !temporary {
                        return Ok(());
                    }
                    (
                        if temporary || !referenced {
                            Status::Orphan
                        } else {
                            Status::Present
                        },
                        if temporary { 11 } else { 10 },
                        digest(value.id())?,
                    )
                }
                None => (Status::Unsupported, 10, ZERO),
            };
            match io::read(
                source,
                &path.join(&name.value),
                LEGACY_REVISION_MAX_OBJECT_BYTES,
            )? {
                Ok(bytes) => stream.item(
                    family,
                    status,
                    kind,
                    [subject, io::fingerprint(source, &bytes)?, reference],
                    [bytes.len() as u64, 0, 0],
                ),
                Err(observed) => stream.item(
                    family,
                    if observed == Status::AbsentProven {
                        Status::Gap
                    } else {
                        observed
                    },
                    kind,
                    [subject, ZERO, reference],
                    [0; 3],
                ),
            }
        })?;
        directory_outcome(stream, family, &path, outcome)
    })?;
    directory_outcome(stream, family, &directory, outcome)
}

fn preparation(
    stream: &mut Stream<'_, '_, '_, '_>,
    root: &Path,
    family: Family,
    residue_only: bool,
) -> Result<(), String> {
    let source = stream.source;
    let base = root.join(LEGACY_PREPARATION_DIRECTORY);
    // Both families share one pass-local metadata view. A second complete
    // stream has its own Stream and independently rebuilds the view.
    let linkage = if let Some(view) = &stream.preparation_links {
        std::sync::Arc::clone(view)
    } else {
            source.charge(Work::Allocate(std::mem::size_of::<super::preparation_links::View>()
                + 2 * std::mem::size_of::<usize>()))?;
            let view = std::sync::Arc::new(super::preparation_links::view(source, root)?);
            stream.preparation_links = Some(std::sync::Arc::clone(&view));
            view
    };
    for tree in [PrepTree::References, PrepTree::Objects] {
        let directory = base.join(tree.directory_name());
        let outcome = io::visit(source, &directory, |shard| {
            let path = directory.join(&shard.value);
            if !shard
                .value
                .to_str()
                .is_some_and(is_legacy_revision_inventory_shard)
            {
                return stream.item(
                    family,
                    Status::Unsupported,
                    12,
                    [io::locator(source, &path)?, ZERO, ZERO],
                    [0; 3],
                );
            }
            let outcome = io::visit(source, &path, |name| {
                let subject = io::child_locator(source, &path, &name.value)?;
                let classified = name
                    .value
                    .to_str()
                    .and_then(|name| classify_legacy_preparation_inventory_name(tree, name));
                let status = match classified {
                    None => Status::Unsupported,
                    Some(value) if value.kind().tag() == "uncommitted_temporary_object" => Status::Orphan,
                    Some(value) if super::preparation_links::is_linked(source, &linkage, tree, &digest(value.id())?, &name.value)? => Status::Present,
                    Some(_) => Status::Gap,
                };
                if let Some(value) = classified
                    && !shard
                        .value
                        .to_str()
                        .is_some_and(|shard| value.id().starts_with(shard))
                {
                    return stream.item(
                        family,
                        Status::Corrupt,
                        12,
                        [subject, ZERO, digest(value.id())?],
                        [0; 3],
                    );
                }
                match io::read(
                    source,
                    &path.join(&name.value),
                    LEGACY_REVISION_MAX_OBJECT_BYTES,
                )? {
                    Ok(bytes) => {
                        if residue_only && status == Status::Present { return Ok(()); }
                        let evidence = io::fingerprint(source, &bytes)?;
                        if tree == PrepTree::References && matches!(status, Status::Present | Status::Gap) {
                            let key = classified
                                .map(|value| digest(value.id()))
                                .transpose()?
                                .ok_or_else(changed)?;
                            match decode_legacy_preparation_reference(&bytes, &key) {
                                Ok(reference) => {
                                    // Keep the semantic key and its exact physical
                                    // observation paired without changing key authority.
                                    stream.item(family, status, 37,
                                        [subject, key, evidence], [bytes.len() as u64, 0, 0])?;
                                    stream.item(
                                    family,
                                    status,
                                    12,
                                    [key, *reference.manifest_digest(), evidence],
                                    [
                                        reference.manifest_bytes(),
                                        u64::from(reference.protection().tag()),
                                        LEGACY_PREPARATION_REFERENCE_BYTES as u64,
                                    ],
                                    )
                                },
                                Err(_) => stream.item(
                                    family,
                                    Status::Corrupt,
                                    12,
                                    [subject, evidence, ZERO],
                                    [bytes.len() as u64, 0, 0],
                                ),
                            }
                        } else if status == Status::Present
                            && name.value.to_str().is_some_and(|value| {
                                Path::new(value).extension().is_some_and(|extension| {
                                    extension == LegacyPreparationProtection::Plaintext.extension()
                                })
                            })
                        {
                            preparation_manifest(stream, family, subject, evidence, &bytes)
                        } else {
                            stream.item(
                                family,
                                status,
                                13,
                                [subject, evidence, ZERO],
                                [bytes.len() as u64, 0, 0],
                            )
                        }
                    }
                    Err(status) => stream.item(
                        family,
                        if status == Status::AbsentProven {
                            Status::Gap
                        } else {
                            status
                        },
                        12,
                        [subject, ZERO, ZERO],
                        [0; 3],
                    ),
                }
            })?;
            directory_outcome(stream, family, &path, outcome)
        })?;
        directory_outcome(stream, family, &directory, outcome)?;
    }
    if residue_only {
        Ok(())
    } else {
        preparation_links(stream, root)?;
        stream.item(
            family,
            Status::Present,
            21,
            [crate::direct_preparation::profile_digest(), ZERO, ZERO],
            [1, 1, 0],
        )
    }
}

// The materializer owns the complete persisted frame and BLAKE3 representation
// preimage. Composition supplies the primitive exactly as its existing codec.
pub(super) struct PreparationRepresentationDigest;
impl LegacyPreparationStoreDigest for PreparationRepresentationDigest {
    fn digest(bytes: &[u8]) -> [u8; 32] {
        sha256::digest(bytes)
    }
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        sha256::digest_parts(domain, parts)
    }
}
impl LegacyDirectRepresentationDigest for PreparationRepresentationDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        for part in parts {
            hasher.update(part);
        }
        *hasher.finalize().as_bytes()
    }
}

fn preparation_links(stream: &mut Stream<'_, '_, '_, '_>, root: &Path) -> Result<(), String> {
    let source = stream.source;
    let input = source.native().ok_or_else(changed)?;
    source.charge(Work::Allocate(512))?;
    let materializer =
        crate::direct_preparation::canonical_materializer_digest().map_err(str::to_owned)?;
    let unitizer = crate::direct_preparation::canonical_unitizer_digest().map_err(str::to_owned)?;
    for revision in source.inventory_revisions() {
        source.charge(Work::Compare(512))?;
        let binding = encode_legacy_preparation_binding(&LegacyDirectPreparationBinding {
            namespace: source.inventory_namespace(),
            source_id: digest(&revision.source_id)?,
            revision_id: digest(&revision.revision_id)?,
            content_digest: digest(&revision.content_digest)?,
            byte_length: revision.byte_length,
            materializer_digest: materializer,
            unitizer_digest: unitizer,
        });
        let key = derive_legacy_preparation_lookup_key::<PreparationRepresentationDigest>(
            &binding,
            input.protector.backend_name(),
        );
        source.charge(Work::AllocateLocator(2048))?;
        let reference_path = root
            .join(LEGACY_PREPARATION_DIRECTORY)
            .join(legacy_preparation_reference_relative_locator(&key));
        let bytes = match io::read(source, &reference_path, LEGACY_PREPARATION_MAX_OBJECT_BYTES)? {
            Ok(bytes) => bytes,
            Err(status) => {
                stream.item(
                    Family::Preparation,
                    if status == Status::AbsentProven {
                        Status::Gap
                    } else {
                        status
                    },
                    28,
                    [key, digest(&revision.revision_id)?, ZERO],
                    [0; 3],
                )?;
                continue;
            }
        };
        let Ok(reference) = decode_legacy_preparation_reference(&bytes, &key) else {
            stream.item(
                Family::Preparation,
                Status::Corrupt,
                28,
                [key, io::fingerprint(source, &bytes)?, ZERO],
                [bytes.len() as u64, 0, 0],
            )?;
            continue;
        };
        let object = derive_legacy_preparation_object_id::<PreparationRepresentationDigest>(
            &binding,
            input.protector.backend_name(),
            reference.manifest_digest(),
        );
        source.charge(Work::AllocateLocator(2048))?;
        let object_path = root.join(LEGACY_PREPARATION_DIRECTORY).join(
            legacy_preparation_object_relative_locator(&object, reference.protection()),
        );
        let encoded = match io::read(source, &object_path, LEGACY_PREPARATION_MAX_OBJECT_BYTES)? {
            Ok(bytes) => bytes,
            Err(status) => {
                stream.item(
                    Family::Preparation,
                    if status == Status::AbsentProven {
                        Status::Gap
                    } else {
                        status
                    },
                    29,
                    [key, object, *reference.manifest_digest()],
                    [reference.manifest_bytes(), 0, 0],
                )?;
                continue;
            }
        };
        let decoded = match reference.protection() {
            LegacyPreparationProtection::Plaintext => encoded,
            LegacyPreparationProtection::Protected => {
                #[cfg(windows)]
                let decoded = input.protector.unprotect_with_context(
                    input.context,
                    &encoded,
                    &sha256::hex(&object),
                    &sha256::hex(reference.manifest_digest()),
                    reference.manifest_bytes(),
                );
                #[cfg(not(windows))]
                let decoded: Result<zeroize::Zeroizing<Vec<u8>>, String> = Err(changed());
                source.checkpoint()?;
                let Ok(bytes) = decoded else {
                    stream.item(
                        Family::Preparation,
                        Status::Unreadable,
                        29,
                        [key, object, *reference.manifest_digest()],
                        [reference.manifest_bytes(), 1, 0],
                    )?;
                    continue;
                };
                bytes
            }
        };
        source.charge(Work::Compare(decoded.len().saturating_mul(2)))?;
        let valid = verify_legacy_preparation_payload::<PreparationRepresentationDigest>(
            &decoded,
            reference.manifest_digest(),
            reference.manifest_bytes(),
        )
        .is_ok()
            && verify_legacy_preparation_manifest::<PreparationRepresentationDigest>(
                &decoded,
                &binding,
                crate::direct_preparation::CANONICAL_MATERIALIZER_REVISION,
                crate::direct_preparation::CANONICAL_UNITIZER_REVISION,
            )
            .is_ok();
        source.checkpoint()?;
        stream.item(
            Family::Preparation,
            if valid {
                Status::Present
            } else {
                Status::Corrupt
            },
            29,
            [key, object, *reference.manifest_digest()],
            [
                reference.manifest_bytes(),
                u64::from(reference.protection().tag()),
                0,
            ],
        )?;
    }
    Ok(())
}

fn preparation_manifest(
    stream: &mut Stream<'_, '_, '_, '_>,
    family: Family,
    subject: [u8; 32],
    evidence: [u8; 32],
    bytes: &[u8],
) -> Result<(), String> {
    let source = stream.source;
    let Some(binding) = bytes
        .get(..LEGACY_PREPARATION_BINDING_BYTES)
        .and_then(|bytes| bytes.try_into().ok())
    else {
        return stream.item(
            family,
            Status::Corrupt,
            13,
            [subject, evidence, ZERO],
            [bytes.len() as u64, 0, 0],
        );
    };
    source.charge(Work::Compare(bytes.len()))?;
    let decoded = verify_legacy_preparation_manifest::<PreparationRepresentationDigest>(
        bytes,
        binding,
        crate::direct_preparation::CANONICAL_MATERIALIZER_REVISION,
        crate::direct_preparation::CANONICAL_UNITIZER_REVISION,
    );
    source.checkpoint()?;
    match decoded {
        Ok(manifest) => {
            let binding_digest = io::fingerprint(source, binding)?;
            stream.item(
                family,
                Status::Present,
                25,
                [subject, evidence, binding_digest],
                [
                    bytes.len() as u64,
                    manifest.materializer_revision(),
                    manifest.unitizer_revision(),
                ],
            )?;
            let binding = manifest.binding();
            stream.item(
                family,
                Status::Present,
                26,
                [
                    binding.source_id,
                    binding.revision_id,
                    binding.content_digest,
                ],
                [binding.byte_length, manifest.body().len() as u64, 0],
            )
        }
        Err(
            search_materializer::api::LegacyPreparationStoreError::ProfileMismatch
            | search_materializer::api::LegacyPreparationStoreError::DigestAlgorithmMismatch,
        ) => stream.item(
            family,
            Status::Unsupported,
            25,
            [subject, evidence, ZERO],
            [bytes.len() as u64, 0, 0],
        ),
        Err(_) => stream.item(
            family,
            Status::Corrupt,
            25,
            [subject, evidence, ZERO],
            [bytes.len() as u64, 0, 0],
        ),
    }
}


// The preparation root is a known tree, but its direct children are not limited
// to the two owned tree directories. Unknown entries are accounted for here as
// content-free metadata observations with the existing classifier and statuses:
// an unknown file stays Unsupported, an unknown directory or refused locator
// stays Gap, and no unknown body is read or recursively walked. Walking an
// arbitrary unknown subtree could traverse unbounded content, so the child
// itself is the complete disposition for the residue family.
fn unknown_preparation_children(
    stream: &mut Stream<'_, '_, '_, '_>,
    root: &Path,
) -> Result<(), String> {
    let source = stream.source;
    let directory = root.join(LEGACY_PREPARATION_DIRECTORY);
    let outcome = io::visit(source, &directory, |name| {
        if name.value.to_str().is_some_and(|name| PrepTree::from_directory_name(name).is_some()) {
            return Ok(());
        }
        let subject = io::child_locator(source, &directory, &name.value)?;
        let child = directory.join(&name.value);
        let (status, bytes) = match io::metadata(source, &child)? {
            io::Observed::Missing => (Status::Gap, 0),
            io::Observed::Refused(status) => (status, 0),
            io::Observed::Object(meta) if meta.is_dir() => (Status::Gap, meta.len()),
            io::Observed::Object(meta) => (Status::Unsupported, meta.len()),
        };
        stream.item(Family::Residue, status, 34, [subject, ZERO, ZERO], [bytes, 0, 0])
    })?;
    directory_outcome(stream, Family::Residue, &directory, outcome)
}
fn residue(stream: &mut Stream<'_, '_, '_, '_>, root: &Path) -> Result<(), String> {
    revision_tree(stream, root, Family::Residue, true)?;
    preparation(stream, root, Family::Residue, true)?;
    control(stream, root, Family::Residue)?;
    unknown_preparation_children(stream, root)?;
    unknown_root_objects(stream, root)
}

// Unknown top-level entries are metadata observations, never secret/source reads.
// Known trees are accounted for by their owning family traversal above.
fn unknown_root_objects(stream: &mut Stream<'_, '_, '_, '_>, root: &Path) -> Result<(), String> {
    let source = stream.source;
    let outcome = io::visit(source, root, |name| {
        if name.value.to_str().is_some_and(|name| {
            matches!(
                name,
                "control"
                    | "revisions"
                    | "preparation"
                    | ".eliot-search-owner.lock"
                    | ".eliot-search-sealed-owner.lock"
                    | ".eliot-search-installation.v1"
                    | ".eliot-search-owner-state-a.v1"
                    | ".eliot-search-owner-state-b.v1"
            )
        }) {
            return Ok(());
        }
        let subject = io::child_locator(source, root, &name.value)?;
        let (status, bytes) = match io::metadata(source, &root.join(&name.value))? {
            io::Observed::Missing => (Status::Gap, 0),
            io::Observed::Refused(status) => (status, 0),
            io::Observed::Object(meta) => (Status::Unsupported, meta.len()),
        };
        stream.item(
            Family::Residue,
            status,
            27,
            [subject, ZERO, ZERO],
            [bytes, 0, 0],
        )
    })?;
    directory_outcome(stream, Family::Residue, root, outcome)
}

fn control(stream: &mut Stream<'_, '_, '_, '_>, root: &Path, family: Family) -> Result<(), String> {
    let source = stream.source;
    let directory = root.join("control");
    let outcome = io::visit(source, &directory, |name| {
        let text = name.value.to_str().unwrap_or("");
        let publication = text.contains("publication")
            || text.contains("manifest")
            || text.contains("shadow")
            || text.contains("epoch")
            || text == "control-cutover.v1";
        let security =
            text.contains("purge") || text.contains("quarantine") || text.contains("retention");
        let operation =
            text.contains("operation") || text.contains("receipt") || text.contains("intent");
        let residue = Path::new(text).extension().is_some_and(|extension| {
            ["tmp", "bak", "pending"]
                .iter()
                .any(|suffix| extension.eq_ignore_ascii_case(suffix))
        });
        let selected = match family {
            Family::Publications => publication,
            Family::Security => security,
            Family::Operations => operation,
            Family::Residue => residue,
            _ => false,
        };
        if !selected {
            if family == Family::Residue && !matches!(text,
                "namespace.id" | "source-events.log" | "source-roots.v1" | "directory-manifests" | "control-cutover.v1") {
                // The name does not identify an owning parser. Account for the
                // whole unknown representation without reading possible secrets.
                let subject = io::child_locator(source, &directory, &name.value)?;
                return stream.item(family, Status::Gap, 34, [subject, ZERO, ZERO], [0; 3]);
            }
            return Ok(());
        }
        let subject = io::child_locator(source, &directory, &name.value)?;
        let path = directory.join(&name.value);
        match io::metadata(source, &path)? {
            io::Observed::Object(meta) if meta.is_dir() => {
                // Every known metadata container is visited, not mistaken for a file.
                stream.item(family, Status::Gap, 35, [subject, ZERO, ZERO], [meta.len(), 0, 0])?;
                control_directory(stream, &path, family, 0)
            }
            _ => control_file(stream, &path, family, subject, residue),
        }
    })?;
    directory_outcome(stream, family, &directory, outcome)?;
    if family == Family::Publications || family == Family::Security {
        // A legacy layout has no complete authoritative H5 reference denominator.
        stream.item(family, Status::Gap, 24, [ZERO; 3], [0; 3])?;
    } else if stream.statuses.iter().all(|count| *count == 0) {
        stream.item(family, Status::AbsentProven, 14, [ZERO; 3], [0; 3])?;
    }
    Ok(())
}

fn control_directory(
    stream: &mut Stream<'_, '_, '_, '_>,
    directory: &Path,
    family: Family,
    depth: usize,
) -> Result<(), String> {
    if depth > 3 {
        return stream.item(
            family,
            Status::Unsupported,
            14,
            [io::locator(stream.source, directory)?, ZERO, ZERO],
            [depth as u64, 0, 0],
        );
    }
    let source = stream.source;
    let outcome = io::visit(source, directory, |name| {
        let path = directory.join(&name.value);
        let subject = io::locator(source, &path)?;
        match io::metadata(source, &path)? {
            io::Observed::Object(meta) if meta.is_dir() => {
                stream.item(family, Status::Gap, 35, [subject, ZERO, ZERO], [meta.len(), depth as u64 + 1, 0])?;
                control_directory(stream, &path, family, depth + 1)
            }
            _ => control_file(stream, &path, family, subject, family == Family::Residue),
        }
    })?;
    directory_outcome(stream, family, directory, outcome)
}

fn control_file(
    stream: &mut Stream<'_, '_, '_, '_>, path: &Path, family: Family,
    subject: [u8; 32], orphan: bool,
) -> Result<(), String> {
    let source = stream.source;
    let supported = path.strip_prefix(source.inventory_root()).is_ok_and(|relative| {
        let mut parts = relative.components();
        parts.next() == Some(std::path::Component::Normal(std::ffi::OsStr::new("control")))
            && parts.next() == Some(std::path::Component::Normal(std::ffi::OsStr::new("control-cutover.v1")))
            && parts.next().is_none()
    });
    if !supported {
        // Only the canonical root-relative locator admits the owning decoder.
        // Parent visits retain no-follow pins; unknown/nested bodies stay unread.
        let (status, size) = match io::metadata(source, path)? {
            io::Observed::Missing => (Status::Gap, 0),
            io::Observed::Refused(status) => (status, 0),
            io::Observed::Object(meta) => (Status::Unsupported, meta.len()),
        };
        return stream.item(family, status, 14, [subject, ZERO, ZERO], [size, 0, u64::from(orphan)]);
    }
    match io::read(source, path, io::CONTROL_BYTES)? {
        Ok(bytes) => {
            let fingerprint = io::fingerprint(source, &bytes)?;
            match search_control_redb::migration::ControlCutoverMarker::decode(&bytes) {
                Ok(marker) => stream.item(family, Status::Present, 15,
                    [subject, fingerprint, marker.catalog_snapshot], [marker.epoch, bytes.len() as u64, 0]),
                Err(_) => stream.item(family, Status::Corrupt, 15,
                    [subject, fingerprint, ZERO], [bytes.len() as u64, 0, 0]),
            }
        }
        Err(status) => stream.item(family,
            if status == Status::AbsentProven { Status::Gap } else { status },
            14, [subject, ZERO, ZERO], [0; 3]),
    }
}
fn directory_outcome(
    stream: &mut Stream<'_, '_, '_, '_>,
    family: Family,
    directory: &Path,
    outcome: Option<Status>,
) -> Result<(), String> {
    let subject = io::locator(stream.source, directory)?;
    if let Some(status) = outcome {
        stream.item(family, status, 16, [subject, ZERO, ZERO], [0; 3])?;
    } else {
        stream.item(family, Status::Present, 36, [subject, ZERO, ZERO], [0; 3])?;
    }
    Ok(())
}

pub(super) fn digest(value: &str) -> Result<[u8; 32], String> {
    sha256::decode_digest(value).ok_or_else(changed)
}
pub(super) fn changed() -> String {
    "DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()
}
