//! The twelve concrete inventory denominators; no whole-inventory collection.

use super::{Family, NativeMigrationInput, Status, Stream, Work, io, sha256};
use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{LegacyInventoryLimits, LegacyInventoryProfile};
use search_materializer::api::{
    LEGACY_PREPARATION_BINDING_BYTES, LEGACY_PREPARATION_DIRECTORY,
    LEGACY_PREPARATION_MAX_OBJECT_BYTES, LEGACY_PREPARATION_REFERENCE_BYTES,
    LegacyPreparationReference, LegacyDirectRepresentationDigest,
    LegacyPreparationInventoryTree as PrepTree, LegacyPreparationProtection,
    LegacyPreparationStoreDigest, classify_legacy_preparation_inventory_name,
    decode_legacy_preparation_reference,
    verify_legacy_preparation_manifest, verify_legacy_preparation_payload,
};
use search_revision_store::{
    LEGACY_REVISION_DIRECTORY, LEGACY_REVISION_MAX_OBJECT_BYTES, LegacyRevisionPhysicalKind,
    LegacyRevisionProtection,
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
        Family::Operations | Family::Publications | Family::Security => control(stream, root, family),
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
    revision_tree(stream, root, Family::Revisions, false, None)?;
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

pub(super) fn revisions_with_content(
    stream: &mut Stream<'_, '_, '_, '_>,
    root: &Path,
    input: &NativeMigrationInput<'_, '_, '_>,
    content: &mut super::super::content_readback::ContentPass<'_>,
) -> Result<(), String> {
    let source = stream.source;
    // Keep the accepted inventory ordering: all logical kind-9 rows precede
    // physical rows. They remain inert until every referenced body is verified
    // and both complete artifacts finish their independent second pass.
    for revision in source.inventory_revisions() {
        source.charge(Work::Compare(512))?;
        stream.item(Family::Revisions, Status::Present, 9,
            [digest(&revision.revision_id)?, digest(&revision.content_digest)?, digest(&revision.source_id)?],
            [revision.byte_length, 0, 0])?;
    }
    let mut revisions = source.retained_revisions();
    let mut current: Option<CurrentRevision<'_, '_, '_, '_>> = None;
    revision_tree(stream, root, Family::Revisions, false, Some(&mut |stream, id, protection, subject| {
        if current.as_ref().is_none_or(|held| held.id != id) {
            if let Some(held) = current.take() { held.finish()?; }
            source.charge(Work::Allocate(512))?;
            let metadata = revisions.next().ok_or_else(changed)?;
            if metadata.revision_id != id { return Err(changed()); }
            let pins = match io::PinnedRevisionRepresentations::open(source, id)? {
                Ok(pins) => pins,
                Err(status) => {
                    stream.item(Family::Revisions,
                        if status == Status::AbsentProven { Status::Gap } else { status },
                        10, [subject, ZERO, digest(id)?], [0; 3])?;
                    return Err(changed());
                }
            };
            let readback = super::super::super::read_import_revision_detailed_with_context(
                root, Some(input.protector), &metadata, input.context,
            )?;
            source.checkpoint()?;
            let raw = [readback.plaintext().map(CurrentRaw::capture), readback.protected().map(CurrentRaw::capture)];
            for (index, kind) in [LegacyRevisionProtection::Plaintext, LegacyRevisionProtection::Protected]
                .into_iter().enumerate()
            {
                pins.verify_readback(kind, raw[index].map(|fact| (fact.identity, fact.length)))?;
            }
            pins.verify_same_original()?.map_err(|_| changed())?;
            content.observe(source, &metadata, &readback)?;
            // Only two fixed-size raw observations survive into the next name;
            // the current plaintext is released before another revision starts.
            drop(readback);
            current = Some(CurrentRevision { id: metadata.revision_id, pins, raw, seen: [false; 2] });
        }
        let held = current.as_mut().ok_or_else(changed)?;
        let index = protection_index(protection);
        if held.seen[index] { return Err(changed()); }
        let raw = held.raw[index].ok_or_else(changed)?;
        held.pins.verify_same_original()?.map_err(|_| changed())?;
        let (refs, counts) = raw.physical_row(subject, digest(id)?);
        stream.item(Family::Revisions, Status::Present, 10, refs, counts)?;
        held.seen[index] = true;
        held.pins.verify_same_original()?.map_err(|_| changed())?;
        Ok(())
    }))?;
    if let Some(held) = current.take() { held.finish()?; }
    if revisions.next().is_some() { return Err(changed()); }
    if stream.statuses.iter().all(|count| *count == 0) {
        stream.item(Family::Revisions, Status::AbsentProven, 9, [ZERO; 3], [0; 3])?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct CurrentRaw {
    identity: Option<(u64, u64)>,
    length: u64,
    sha256: [u8; 32],
}

impl CurrentRaw {
    const fn capture(raw: &super::super::super::RawObjectEvidence) -> Self {
        Self { identity: raw.native_identity(), length: raw.encoded_bytes(), sha256: *raw.encoded_sha256() }
    }

    const fn physical_row(self, subject: [u8; 32], reference: [u8; 32]) -> ([[u8; 32]; 3], [u64; 3]) {
        ([subject, self.sha256, reference], [self.length, 0, 0])
    }
}

struct CurrentRevision<'a, 'view, 'operation, 'owner> {
    id: String,
    pins: io::PinnedRevisionRepresentations<'a, 'view, 'operation, 'owner>,
    raw: [Option<CurrentRaw>; 2],
    seen: [bool; 2],
}

impl CurrentRevision<'_, '_, '_, '_> {
    fn finish(self) -> Result<(), String> {
        for index in 0..2 {
            if self.raw[index].is_some() != self.seen[index] { return Err(changed()); }
        }
        self.pins.verify_same_original()?.map_err(|_| changed())
    }
}

const fn protection_index(protection: LegacyRevisionProtection) -> usize {
    match protection {
        LegacyRevisionProtection::Plaintext => 0,
        LegacyRevisionProtection::Protected => 1,
    }
}

type RevisionCapture<'visitor> = dyn for<'stream, 'view, 'operation, 'owner> FnMut(
    &mut Stream<'stream, 'view, 'operation, 'owner>,
    &str,
    LegacyRevisionProtection,
    [u8; 32],
) -> Result<(), String> + 'visitor;

fn revision_tree(
    stream: &mut Stream<'_, '_, '_, '_>,
    root: &Path,
    family: Family,
    residue_only: bool,
    mut captured: Option<&mut RevisionCapture<'_>>,
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
                    if referenced && !residue_only
                        && let LegacyRevisionPhysicalKind::Final(protection) = value.physical_kind()
                        && let Some(consume) = captured.as_deref_mut()
                    {
                        return consume(stream, value.id(), protection, subject);
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
    if residue_only {
        let view = stream.preparation_links.as_ref().ok_or_else(changed)?;
        let view = std::sync::Arc::clone(view);
        for row in view.residue()? { preparation_emit(stream, Family::Residue, *row)?; }
        return Ok(());
    }
    if family != Family::Preparation || stream.preparation_links.is_some() { return Err(changed()); }
    let source = stream.source;
    let input = source.native().ok_or_else(changed)?;
    preparation_pass(stream, root, input.protector.backend_name(),
        &mut |binding, key, object, reference, encoded|
            preparation_semantic(source, input, binding, key, object, reference, encoded),
        #[cfg(test)]
        &mut || Ok(()))
}

fn preparation_pass(stream: &mut Stream<'_, '_, '_, '_>, root: &Path, backend: &str,
    semantic_read: super::preparation_links::SemanticRead<'_>,
    #[cfg(test)] before_physical: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    if stream.preparation_links.is_some() { return Err(changed()); }
    let source = stream.source;
    source.charge(Work::Allocate(std::mem::size_of::<super::preparation_links::View>()
        + 2 * std::mem::size_of::<usize>()))?;
    let mut view = super::preparation_links::observe_links(source, root, backend, semantic_read)?;
    #[cfg(test)]
    before_physical()?;
    source.checkpoint()?;
    preparation_physical(stream, root, &mut view)?;
    view.finish_physical(source)?;
    for row in view.semantic()? { preparation_emit(stream, Family::Preparation, *row)?; }
    stream.item(Family::Preparation, Status::Present, 21,
        [crate::direct_preparation::profile_digest(), ZERO, ZERO], [1, 1, 0])?;
    stream.preparation_links = Some(std::sync::Arc::new(view));
    Ok(())
}

fn preparation_item(stream: &mut Stream<'_, '_, '_, '_>,
    view: &mut super::preparation_links::View, row: super::preparation_links::Row, residue: bool,
) -> Result<(), String> {
    if residue { view.retain_residue(stream.source, row)?; }
    preparation_emit(stream, Family::Preparation, row)
}

fn preparation_emit(stream: &mut Stream<'_, '_, '_, '_>, family: Family,
    row: super::preparation_links::Row,
) -> Result<(), String> {
    stream.item(family, row.status, row.kind, row.refs, row.counts)
}

fn preparation_semantic(source: &super::MigrationSource<'_, '_, '_>,
    input: &NativeMigrationInput<'_, '_, '_>, binding: &[u8; LEGACY_PREPARATION_BINDING_BYTES],
    key: [u8; 32], object: [u8; 32], reference: LegacyPreparationReference, encoded: io::DetailedRead,
) -> Result<super::preparation_links::Row, String> {
    use super::preparation_links::Row;
    let refs = [key, object, *reference.manifest_digest()];
    let decoded = match reference.protection() {
        LegacyPreparationProtection::Plaintext => encoded.bytes,
        LegacyPreparationProtection::Protected => {
            #[cfg(windows)]
            let decoded = input.protector.unprotect_with_context(input.context, &encoded.bytes,
                &sha256::hex(&object), &sha256::hex(reference.manifest_digest()), reference.manifest_bytes());
            #[cfg(not(windows))]
            let decoded: Result<zeroize::Zeroizing<Vec<u8>>, String> = Err(changed());
            source.checkpoint()?;
            let Ok(decoded) = decoded else {
                return Ok(Row { status: Status::Unreadable, kind: 29,
                    refs, counts: [reference.manifest_bytes(), 1, 0] });
            };
            decoded
        }
    };
    preparation_decoded(source, binding, key, object, reference, &decoded)
}

fn preparation_decoded(source: &super::MigrationSource<'_, '_, '_>,
    binding: &[u8; LEGACY_PREPARATION_BINDING_BYTES], key: [u8; 32], object: [u8; 32],
    reference: LegacyPreparationReference, decoded: &[u8],
) -> Result<super::preparation_links::Row, String> {
    use super::preparation_links::Row;
    let refs = [key, object, *reference.manifest_digest()];
    source.charge(Work::Compare(decoded.len().saturating_mul(2)))?;
    let valid = verify_legacy_preparation_payload::<PreparationRepresentationDigest>(
        decoded, reference.manifest_digest(), reference.manifest_bytes()).is_ok()
        && verify_legacy_preparation_manifest::<PreparationRepresentationDigest>(decoded, binding,
            crate::direct_preparation::CANONICAL_MATERIALIZER_REVISION,
            crate::direct_preparation::CANONICAL_UNITIZER_REVISION).is_ok();
    source.checkpoint()?;
    Ok(Row { status: if valid { Status::Present } else { Status::Corrupt }, kind: 29,
        refs, counts: [reference.manifest_bytes(), u64::from(reference.protection().tag()), 0] })
}

/// Component entry over the production pass and materializer verifier. Backend
/// must be the real persisted profile; protected bytes cannot acquire a fake
/// DPAPI success here. The hook runs exactly after preliminary qualified reads.
#[cfg(test)]
fn preparation_plaintext(stream: &mut Stream<'_, '_, '_, '_>, root: &Path, backend: &str,
    before_physical: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let source = stream.source;
    preparation_pass(stream, root, backend, &mut |binding, key, object, reference, encoded| {
        if reference.protection() != LegacyPreparationProtection::Plaintext { return Err(changed()); }
        preparation_decoded(source, binding, key, object, reference, &encoded.bytes)
    }, before_physical)
}

fn preparation_directory(stream: &mut Stream<'_, '_, '_, '_>,
    view: &mut super::preparation_links::View, path: &Path, outcome: Option<Status>,
) -> Result<(), String> {
    preparation_item(stream, view, super::preparation_links::Row {
        status: outcome.unwrap_or(Status::Present), kind: if outcome.is_some() { 16 } else { 36 },
        refs: [io::locator(stream.source, path)?, ZERO, ZERO], counts: [0; 3],
    }, true)
}

/// One complete physical denominator supplies both ordered families. Only
/// bounded encoder arguments survive to Residue; every read remains qualified.
fn preparation_physical(stream: &mut Stream<'_, '_, '_, '_>, root: &Path,
    view: &mut super::preparation_links::View,
) -> Result<(), String> {
    use super::preparation_links::Row;
    let source = stream.source;
    let base = root.join(LEGACY_PREPARATION_DIRECTORY);
    for tree in [PrepTree::References, PrepTree::Objects] {
        let directory = base.join(tree.directory_name());
        let outcome = io::visit(source, &directory, |shard| {
            let path = directory.join(&shard.value);
            if !shard.value.to_str().is_some_and(is_legacy_revision_inventory_shard) {
                return preparation_item(stream, view, Row { status: Status::Unsupported, kind: 12,
                    refs: [io::locator(source, &path)?, ZERO, ZERO], counts: [0; 3] }, true);
            }
            let outcome = io::visit(source, &path, |name| {
                let subject = io::child_locator(source, &path, &name.value)?;
                let classified = name.value.to_str()
                    .and_then(|name| classify_legacy_preparation_inventory_name(tree, name));
                let status = match classified {
                    None => Status::Unsupported,
                    Some(value) if value.kind().tag() == "uncommitted_temporary_object" => Status::Orphan,
                    Some(value) if super::preparation_links::is_linked(source, view, tree,
                        &digest(value.id())?, &name.value)? => Status::Present,
                    Some(_) => Status::Gap,
                };
                if let Some(value) = classified
                    && !shard.value.to_str().is_some_and(|shard| value.id().starts_with(shard))
                {
                    view.entry(false)?;
                    return preparation_item(stream, view, Row { status: Status::Corrupt, kind: 12,
                        refs: [subject, ZERO, digest(value.id())?], counts: [0; 3] }, true);
                }
                let read = io::read_detailed(source, &path.join(&name.value), LEGACY_REVISION_MAX_OBJECT_BYTES)?;
                view.observe(source, tree, subject,
                    read.as_ref().map(|read| read.proof).map_err(|status| *status))?;
                let positive = status == Status::Present && read.is_ok();
                view.entry(positive)?;
                let rows = match read {
                    Ok(read) => {
                        let evidence = read.proof.raw_digest;
                        let bytes = &read.bytes;
                        if tree == PrepTree::References && matches!(status, Status::Present | Status::Gap) {
                            let key = classified.map(|value| digest(value.id())).transpose()?.ok_or_else(changed)?;
                            match decode_legacy_preparation_reference(bytes, &key) {
                                Ok(reference) => [Some(Row { status, kind: 37,
                                    refs: [subject, key, evidence], counts: [read.proof.bytes, 0, 0] }),
                                    Some(Row { status, kind: 12,
                                        refs: [key, *reference.manifest_digest(), evidence],
                                        counts: [reference.manifest_bytes(), u64::from(reference.protection().tag()),
                                            LEGACY_PREPARATION_REFERENCE_BYTES as u64] })],
                                Err(_) => [Some(Row { status: Status::Corrupt, kind: 12,
                                    refs: [subject, evidence, ZERO], counts: [read.proof.bytes, 0, 0] }), None],
                            }
                        } else if status == Status::Present && name.value.to_str().is_some_and(|value|
                            Path::new(value).extension().is_some_and(|extension|
                                extension == LegacyPreparationProtection::Plaintext.extension()))
                        {
                            preparation_manifest(source, subject, evidence, bytes)?
                        } else {
                            [Some(Row { status, kind: 13, refs: [subject, evidence, ZERO],
                                counts: [read.proof.bytes, 0, 0] }), None]
                        }
                    }
                    Err(status) => [Some(Row {
                        status: if status == Status::AbsentProven { Status::Gap } else { status },
                        kind: 12, refs: [subject, ZERO, ZERO], counts: [0; 3],
                    }), None],
                };
                for row in rows.into_iter().flatten() {
                    preparation_item(stream, view, row, !positive)?;
                }
                Ok(())
            })?;
            preparation_directory(stream, view, &path, outcome)
        })?;
        preparation_directory(stream, view, &directory, outcome)?;
    }
    Ok(())
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


fn preparation_manifest(
    source: &super::MigrationSource<'_, '_, '_>,
    subject: [u8; 32], evidence: [u8; 32], bytes: &[u8],
) -> Result<[Option<super::preparation_links::Row>; 2], String> {
    use super::preparation_links::Row;
    let failure = |status, kind| [Some(Row { status, kind,
        refs: [subject, evidence, ZERO], counts: [bytes.len() as u64, 0, 0] }), None];
    let Some(binding) = bytes.get(..LEGACY_PREPARATION_BINDING_BYTES)
        .and_then(|bytes| bytes.try_into().ok())
    else { return Ok(failure(Status::Corrupt, 13)); };
    source.charge(Work::Compare(bytes.len()))?;
    let decoded = verify_legacy_preparation_manifest::<PreparationRepresentationDigest>(
        bytes, binding, crate::direct_preparation::CANONICAL_MATERIALIZER_REVISION,
        crate::direct_preparation::CANONICAL_UNITIZER_REVISION);
    source.checkpoint()?;
    Ok(match decoded {
        Ok(manifest) => {
            let binding_digest = io::fingerprint(source, binding)?;
            let binding = manifest.binding();
            [Some(Row { status: Status::Present, kind: 25,
                refs: [subject, evidence, binding_digest],
                counts: [bytes.len() as u64, manifest.materializer_revision(), manifest.unitizer_revision()] }),
                Some(Row { status: Status::Present, kind: 26,
                    refs: [binding.source_id, binding.revision_id, binding.content_digest],
                    counts: [binding.byte_length, manifest.body().len() as u64, 0] })]
        }
        Err(search_materializer::api::LegacyPreparationStoreError::ProfileMismatch
            | search_materializer::api::LegacyPreparationStoreError::DigestAlgorithmMismatch)
            => failure(Status::Unsupported, 25),
        Err(_) => failure(Status::Corrupt, 25),
    })
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
    revision_tree(stream, root, Family::Residue, true, None)?;
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

pub(super) fn control(stream: &mut Stream<'_, '_, '_, '_>, root: &Path, family: Family) -> Result<(), String> {
    let source = stream.source;
    if root != source.inventory_root() { return Err(changed()); }
    if stream.control_view.is_none() {
        let captured = super::control_view::ControlView::capture(source, root, &stream.encoder, family)?;
        source.charge(Work::Allocate(std::mem::size_of::<super::control_view::ControlView>()))?;
        source.checkpoint()?;
        stream.control_view = Some(std::sync::Arc::new(captured));
    }
    let view = std::sync::Arc::clone(stream.control_view.as_ref().ok_or_else(changed)?);
    view.preflight(&stream.encoder, family)?;
    view.project(source, family, |row| stream.item(family, row.status, row.kind, row.refs, row.counts))?;
    if family == Family::Publications || family == Family::Security {
        // A legacy layout has no complete authoritative H5 reference denominator.
        stream.item(family, Status::Gap, 24, [ZERO; 3], [0; 3])?;
    } else if stream.statuses.iter().all(|count| *count == 0) {
        stream.item(family, Status::AbsentProven, 14, [ZERO; 3], [0; 3])?;
    }
    if family == Family::Operations {
        // V1 effect IDs are catalog-derived, not original invocation receipts.
        stream.item(family, Status::Gap, 348, [ZERO; 3], [348, 0, 0])?;
    }
    Ok(())
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

#[cfg(all(test, windows))]
mod co_production_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use search_contracts::Sha256Digest32;
    use search_control_redb::migration::LegacyInventoryEncoder;

    const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    struct Scratch { root: PathBuf }
    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let root = std::env::temp_dir().join(format!("search-coproduction-{}-{stamp}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir_all(root.join("revisions/aa")).unwrap();
            fs::write(root.join(format!("revisions/aa/{ID}.bin")), b"plain").unwrap();
            fs::write(root.join(format!("revisions/aa/{ID}.dpapi")), b"raw protected wire").unwrap();
            Self { root }
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
            assert!(self.root.file_name().unwrap().to_str().unwrap().starts_with("search-coproduction-"));
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn capture_physical(
        source: &super::super::MigrationSource<'_, '_, '_>,
        pins: &io::PinnedRevisionRepresentations<'_, '_, '_, '_>,
    ) -> [Option<CurrentRaw>; 2] {
        let mut facts = [None; 2];
        for (index, protection) in [LegacyRevisionProtection::Plaintext, LegacyRevisionProtection::Protected]
            .into_iter().enumerate()
        {
            let path = source.inventory_root().join(LEGACY_REVISION_DIRECTORY)
                .join(search_revision_store::legacy_revision_object_relative_locator(ID, protection).unwrap());
            let bytes = io::read(source, &path, LEGACY_REVISION_MAX_OBJECT_BYTES).unwrap().unwrap();
            let held = pins.representation(protection).unwrap();
            facts[index] = Some(CurrentRaw { identity: Some(held.native_identity()),
                length: bytes.len() as u64, sha256: io::fingerprint(source, &bytes).unwrap() });
        }
        facts
    }

    #[test]
    fn co_production_physical_rows_keep_location_v2_raw_bytes() {
        let scratch = Scratch::new();
        let store = crate::plaintext_direct_store::DirectStore::open(&scratch.root).unwrap();
        let source = super::super::MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
        let pins = io::PinnedRevisionRepresentations::open(&source, ID).unwrap().unwrap_or_else(|_| panic!("pins"));
        let raw = capture_physical(&source, &pins);
        for (index, protection) in [LegacyRevisionProtection::Plaintext, LegacyRevisionProtection::Protected]
            .into_iter().enumerate()
        {
            let path = source.inventory_root().join(LEGACY_REVISION_DIRECTORY)
                .join(search_revision_store::legacy_revision_object_relative_locator(ID, protection).unwrap());
            let subject = io::locator(&source, &path).unwrap();
            let reference = digest(ID).unwrap();
            let bytes = io::read(&source, &path, LEGACY_REVISION_MAX_OBJECT_BYTES).unwrap().unwrap();
            let baseline = ([subject, io::fingerprint(&source, &bytes).unwrap(), reference], [bytes.len() as u64, 0, 0]);
            let projected = raw[index].unwrap().physical_row(subject, reference);
            assert_eq!(projected, baseline);
            // Same accepted encoder and selected profile; neither native ID
            // nor a plaintext BLAKE substitutes for the protected raw SHA.
            let mut old = LegacyInventoryEncoder::with_profile(LegacyInventoryLimits::BASELINE, LegacyInventoryProfile::LocationV2).unwrap();
            let mut new = LegacyInventoryEncoder::with_profile(LegacyInventoryLimits::BASELINE, LegacyInventoryProfile::LocationV2).unwrap();
            let binding = sample_binding();
            assert_eq!(old.header(binding.clone()).unwrap(), new.header(binding).unwrap());
            for family in Family::ALL {
                if family == Family::Revisions { break; }
                old.item(family, Status::AbsentProven, 0, [Sha256Digest32::from_bytes(ZERO); 3], [0; 3]).unwrap();
                new.item(family, Status::AbsentProven, 0, [Sha256Digest32::from_bytes(ZERO); 3], [0; 3]).unwrap();
                old.end_family(family, Status::AbsentProven, true).unwrap();
                new.end_family(family, Status::AbsentProven, true).unwrap();
            }
            assert_eq!(old.item(Family::Revisions, Status::Present, 10, baseline.0.map(Sha256Digest32::from_bytes), baseline.1).unwrap(),
                new.item(Family::Revisions, Status::Present, 10, projected.0.map(Sha256Digest32::from_bytes), projected.1).unwrap());
        }
    }

    #[test]
    fn co_production_refuses_unconsumed_present_representation() {
        let scratch = Scratch::new();
        let store = crate::plaintext_direct_store::DirectStore::open(&scratch.root).unwrap();
        let source = super::super::MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
        let pins = io::PinnedRevisionRepresentations::open(&source, ID).unwrap().unwrap_or_else(|_| panic!("pins"));
        let raw = capture_physical(&source, &pins);
        let current = CurrentRevision { id: ID.to_owned(), pins, raw, seen: [true, false] };
        assert_eq!(current.finish(), Err(changed()));
    }

    fn sample_binding() -> search_control_redb::migration::LegacyInventoryBinding {
        search_control_redb::migration::LegacyInventoryBinding {
            owner: search_control_redb::migration::LegacyInventoryOwner {
                installation: [1; 16], incarnation: [2; 16], root: [3; 16], invocation: [4; 16],
                request_blake3: [5; 32], record_blake3: [6; 32], physical_blake3: [7; 32],
                epoch: 1, generation: 1, native_limits: [1; 6],
            },
            target: SourceNamespaceId::from_bytes([8; 16]),
            namespace: Sha256Digest32::from_bytes([9; 32]), snapshot: Sha256Digest32::from_bytes([10; 32]),
            registration: None,
        }
    }

    fn preparation_encoder() -> LegacyInventoryEncoder {
        let mut encoder = LegacyInventoryEncoder::with_profile(
            LegacyInventoryLimits::BASELINE, LegacyInventoryProfile::LocationV2).unwrap();
        encoder.header(sample_binding()).unwrap();
        for family in Family::ALL {
            if family == Family::Preparation { break; }
            encoder.end_family(family, Status::AbsentProven, true).unwrap();
        }
        encoder
    }

    #[test]
    fn preparation_observation_preserves_ordered_bytes_and_replays_residue_without_reopening() {
        use super::super::preparation_links::{Row, View};
        use search_materializer::api::{encode_legacy_preparation_reference,
            legacy_preparation_object_relative_locator, legacy_preparation_reference_relative_locator};
        use std::os::windows::fs::OpenOptionsExt;
        let scratch = Scratch::new();
        let base = scratch.root.join("preparation");
        let key = [0xaa; 32];
        let object = [0xbb; 32];
        let reference_path = base.join(legacy_preparation_reference_relative_locator(&key));
        let object_path = base.join(legacy_preparation_object_relative_locator(&object, LegacyPreparationProtection::Protected));
        fs::create_dir_all(reference_path.parent().unwrap()).unwrap();
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        let reference = encode_legacy_preparation_reference(&key, &[0x24; 32], 300,
            LegacyPreparationProtection::Protected).unwrap();
        fs::write(&reference_path, reference).unwrap();
        fs::write(&object_path, b"encoded").unwrap();
        let gap_path = base.join("objects/bb").join(format!("{}.bin", "bb".repeat(32)));
        fs::write(&gap_path, b"gap").unwrap();
        let temporary = base.join("objects/bb").join(format!(".{}.1.2.dpapi.tmp", "bb".repeat(32)));
        fs::write(&temporary, b"temporary").unwrap();
        let unsupported = base.join("objects/bb/unknown.bin");
        fs::write(&unsupported, b"unknown").unwrap();
        let corrupt = base.join("objects/bb").join(format!("{}.bin", "cc".repeat(32)));
        fs::write(&corrupt, b"misplaced").unwrap();
        let store = crate::plaintext_direct_store::DirectStore::open(&scratch.root).unwrap();
        let source = super::super::MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
        let root = source.inventory_root();
        let rebase = |path: &Path| root.join(path.strip_prefix(&scratch.root).unwrap());
        let reference_path = rebase(&reference_path);
        let object_path = rebase(&object_path);
        let gap_path = rebase(&gap_path);
        let temporary = rebase(&temporary);
        let unsupported = rebase(&unsupported);
        let corrupt = rebase(&corrupt);
        let base = root.join("preparation");
        let mut view = View::fixture_reads(&source, &[
            (PrepTree::References, key, reference_path.clone()),
            (PrepTree::Objects, object, object_path.clone()),
        ]).unwrap();
        let subject = |path: &Path| io::locator(&source, path).unwrap();
        let row = |status, kind, refs, counts| Row { status, kind, refs, counts };
        let directory = |path: &Path| row(Status::Present, 36, [subject(path), ZERO, ZERO], [0; 3]);
        let raw = |path: &Path| sha256::digest(&fs::read(path).unwrap());
        let prep = vec![
            row(Status::Present, 37, [subject(&reference_path), key, raw(&reference_path)], [reference.len() as u64, 0, 0]),
            row(Status::Present, 12, [key, [0x24; 32], raw(&reference_path)],
                [300, u64::from(LegacyPreparationProtection::Protected.tag()), LEGACY_PREPARATION_REFERENCE_BYTES as u64]),
            directory(&base.join("refs/aa")), directory(&base.join("refs")),
            row(Status::Orphan, 13, [subject(&temporary), raw(&temporary), ZERO], [9, 0, 0]),
            row(Status::Gap, 13, [subject(&gap_path), raw(&gap_path), ZERO], [3, 0, 0]),
            row(Status::Present, 13, [subject(&object_path), raw(&object_path), ZERO], [7, 0, 0]),
            row(Status::Corrupt, 12, [subject(&corrupt), ZERO, [0xcc; 32]], [0; 3]),
            row(Status::Unsupported, 13, [subject(&unsupported), raw(&unsupported), ZERO], [7, 0, 0]),
            directory(&base.join("objects/bb")), directory(&base.join("objects")),
        ];
        let residue = prep.iter().copied().enumerate()
            .filter_map(|(index, row)| (!matches!(index, 0 | 1 | 6)).then_some(row)).collect::<Vec<_>>();
        let mut actual = Vec::new();
        let mut sink = |bytes: &[u8]| { actual.extend_from_slice(bytes); Ok(()) };
        let mut stream = Stream { source: &source, sink: &mut sink, encoder: preparation_encoder(),
            statuses: [0; 7], complete: true, preparation_links: None, control_view: None };
        preparation_physical(&mut stream, root, &mut view).unwrap();
        view.finish_physical(&source).unwrap();
        let terminal = stream.terminal();
        let row = stream.encoder.end_family(Family::Preparation, terminal, stream.complete).unwrap();
        stream.send(&row).unwrap();
        stream.statuses = [0; 7];
        stream.complete = true;
        stream.preparation_links = Some(std::sync::Arc::new(view));
        // A second body read would fail on these real sharing exclusions. This
        // tests row transport only; actual native final fences remain required.
        let _exclusions = [reference_path, object_path, gap_path, temporary, unsupported, corrupt]
            .iter().map(|path| fs::OpenOptions::new().read(true).share_mode(0).open(path).unwrap())
            .collect::<Vec<_>>();
        preparation(&mut stream, root, Family::Residue, true).unwrap();
        let terminal = stream.terminal();
        let row = stream.encoder.end_family(Family::Residue, terminal, stream.complete).unwrap();
        stream.send(&row).unwrap();
        drop(stream);
        let mut expected = Vec::new();
        let mut encoder = preparation_encoder();
        for (family, rows) in [(Family::Preparation, prep), (Family::Residue, residue)] {
            for row in rows {
                expected.extend(encoder.item(family, row.status, row.kind,
                    row.refs.map(Sha256Digest32::from_bytes), row.counts).unwrap());
            }
            expected.extend(encoder.end_family(family, Status::Gap, false).unwrap());
        }
        assert_eq!(actual, expected, "complete physical and separate residue rows retain v2 bytes/order");
    }

    #[test]
    fn preparation_observation_refuses_equal_bytes_on_replaced_native_object_and_unexhausted_join() {
        use super::super::preparation_links::View;
        let scratch = Scratch::new();
        let path = scratch.root.join("preparation/objects/aa").join(format!("{ID}.bin"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"same bytes").unwrap();
        let store = crate::plaintext_direct_store::DirectStore::open(&scratch.root).unwrap();
        let source = super::super::MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
        let path = source.inventory_root().join(path.strip_prefix(&scratch.root).unwrap());
        let mut view = View::fixture_reads(&source, &[(PrepTree::Objects, [0xaa; 32], path.clone())]).unwrap();
        assert_eq!(view.finish_physical(&source), Err(changed()), "unconsumed positive cannot close");
        assert!(view.residue().is_err(), "partial physical denominator cannot escape");
        fs::rename(&path, scratch.root.join("original-held-by-name.bin")).unwrap();
        fs::write(&path, b"same bytes").unwrap();
        let observed = io::read_detailed(&source, &path, 64).unwrap().unwrap();
        assert_eq!(observed.proof.raw_digest, sha256::digest(b"same bytes"));
        assert_eq!(view.observe(&source, PrepTree::Objects, io::locator(&source, &path).unwrap(),
            Ok(observed.proof)), Err(changed()), "equal SHA/length cannot replace native identity");
    }
}

// These fixtures exercise local NTFS pin/refusal behavior only. They do not
// qualify a NativeMigrationInput, the full co-producer or installed cutover.
#[cfg(all(test, windows))]
mod revision_pin_tests {
    use super::super::io::PinnedRevisionRepresentations;
    use super::super::super::super::super::migration_context::MigrationSource;
    use search_revision_store::{LegacyRevisionProtection, LEGACY_REVISION_DIRECTORY, legacy_revision_object_relative_locator};
    use std::fs::{self, File, OpenOptions};
    use std::io::Read;
    use std::path::PathBuf;
    use search_control_redb::migration::LegacyInventoryStatus as Status;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    struct Scratch { root: PathBuf }

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
            let root = std::env::temp_dir().join(format!("search-revision-pins-{}-{stamp}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir_all(root.join("revisions/aa")).expect("scratch shard");
            Self { root }
        }

        fn path(&self, protection: LegacyRevisionProtection) -> PathBuf {
            self.root.join(LEGACY_REVISION_DIRECTORY)
                .join(legacy_revision_object_relative_locator(REVISION, protection).expect("locator"))
        }

        fn store(&self) -> crate::plaintext_direct_store::DirectStore {
            crate::plaintext_direct_store::DirectStore::open(&self.root).expect("fixture catalog")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // Only this fixture's freshly created, uniquely named temp root.
            assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
            assert!(self.root.file_name().unwrap().to_str().unwrap().starts_with("search-revision-pins-"));
            fs::remove_dir_all(&self.root).expect("drop scratch after pins");
        }
    }

    #[test]
    fn both_originals_deny_write_delete_and_match_only_their_own_readback() {
        let scratch = Scratch::new();
        fs::write(scratch.path(LegacyRevisionProtection::Plaintext), b"plain").expect("plain");
        fs::write(scratch.path(LegacyRevisionProtection::Protected), b"protected wire").expect("wire");
        let store = scratch.store();
        let source = MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
        let held = PinnedRevisionRepresentations::open(&source, REVISION).expect("open").unwrap_or_else(|_| panic!("pins refused"));
        let mut facts = Vec::new();
        for protection in [LegacyRevisionProtection::Plaintext, LegacyRevisionProtection::Protected] {
            let path = scratch.path(protection);
            assert!(OpenOptions::new().write(true).open(&path).is_err(), "write must be denied while held");
            assert!(fs::remove_file(&path).is_err(), "delete must be denied while held");
            let mut reopened = File::open(&path).expect("source owner's read-only reopen");
            let observed = eliot_searchd::native_file::observe_with_check(&reopened, &|_| Ok(())).expect("same native observer");
            let mut bytes = Vec::new();
            reopened.read_to_end(&mut bytes).expect("callback read");
            let id = (u64::from(observed.volume_serial), observed.file_index);
            held.verify_readback(protection, Some((Some(id), bytes.len() as u64))).expect("own fact");
            let pinned = held.representation(protection).expect("representation");
            assert_eq!(pinned.protection(), protection);
            assert_eq!(*pinned.snapshot(), observed);
            assert_eq!(pinned.metadata().len(), bytes.len() as u64);
            assert!(held.verify_readback(protection, Some((None, bytes.len() as u64))).is_err());
            assert!(held.verify_readback(protection, Some((Some(id), bytes.len() as u64 + 1))).is_err());
            facts.push((id, bytes.len() as u64));
        }
        assert!(held.verify_readback(LegacyRevisionProtection::Plaintext, Some((Some(facts[1].0), facts[0].1))).is_err());
        held.verify_same_original().expect("stable originals").unwrap_or_else(|_| panic!("verification refused"));
    }

    #[test]
    fn missing_both_is_explicit_and_missing_counterpart_is_rechecked() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let source = MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
        assert!(matches!(PinnedRevisionRepresentations::open(&source, REVISION), Ok(Err(Status::AbsentProven))));
        assert!(PinnedRevisionRepresentations::open(&source, "../outside").is_err());
        fs::write(scratch.path(LegacyRevisionProtection::Plaintext), b"plain").expect("plain");
        let held = PinnedRevisionRepresentations::open(&source, REVISION).expect("open").unwrap_or_else(|_| panic!("pins refused"));
        assert!(held.representation(LegacyRevisionProtection::Protected).is_none());
        held.verify_readback(LegacyRevisionProtection::Protected, None).expect("explicit absence");
        assert!(held.verify_readback(LegacyRevisionProtection::Plaintext, None).is_err());
        fs::write(scratch.path(LegacyRevisionProtection::Protected), b"late wire").expect("new counterpart");
        assert!(held.verify_same_original().is_err(), "late presence cannot match prior absence");
    }

    #[test]
    fn hardlink_and_second_representation_refusal_release_all_leaf_pins() {
        let scratch = Scratch::new();
        let plain = scratch.path(LegacyRevisionProtection::Plaintext);
        fs::write(&plain, b"plain").expect("plain");
        let alias = scratch.root.join("alias.bin");
        fs::hard_link(&plain, &alias).expect("hardlink fixture");
        let store = scratch.store();
        let source = MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
        assert!(matches!(PinnedRevisionRepresentations::open(&source, REVISION), Ok(Err(Status::Gap))));
        fs::remove_file(alias).expect("remove alias");
        fs::create_dir(scratch.path(LegacyRevisionProtection::Protected)).expect("wrong-kind counterpart");
        assert!(matches!(PinnedRevisionRepresentations::open(&source, REVISION), Ok(Err(Status::Corrupt))));
        assert!(OpenOptions::new().write(true).open(plain).is_ok(), "refusal must release the first leaf pin");
    }
}

#[cfg(all(test, windows))]
#[path = "preparation_dedup_tests.rs"]
mod preparation_dedup_tests;
