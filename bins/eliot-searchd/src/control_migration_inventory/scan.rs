//! The twelve concrete inventory denominators; no whole-inventory collection.

use std::path::Path;
use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{legacy_inventory_profile_digest, LegacyInventoryLimits};
use search_materializer::api::{LegacyPreparationInventoryTree as PrepTree,
    classify_legacy_preparation_inventory_name, decode_legacy_preparation_reference,
    LEGACY_PREPARATION_DIRECTORY, LEGACY_PREPARATION_REFERENCE_BYTES};
use search_revision_store::{classify_legacy_revision_inventory_name,
    is_legacy_revision_inventory_shard, LegacyRevisionPhysicalKind,
    LEGACY_REVISION_DIRECTORY, LEGACY_REVISION_MAX_OBJECT_BYTES};
use super::{Family, Status, Stream, NativeMigrationInput, Work, sha256, io};

const ZERO: [u8; 32] = [0; 32];

pub(super) fn family(stream: &mut Stream<'_, '_, '_, '_>, family: Family, root: &Path,
    target: SourceNamespaceId, input: &NativeMigrationInput<'_, '_, '_>) -> Result<(), String> {
    match family {
        Family::Owner => stream.item(family, Status::Present, 1, [ZERO; 3],
            [input.inventory_owner.epoch, input.inventory_owner.generation, 1]),
        Family::Sources => sources(stream, target),
        Family::Roots => roots(stream, root, input),
        Family::DirectoryManifests => manifests(stream, root, target),
        Family::Revisions => revisions(stream, root, input),
        Family::Preparation => preparation(stream, root),
        Family::Residue => residue(stream, root),
        Family::Operations => {
            control(stream, root, family)?;
            // V1 effect IDs are catalog-derived, not original invocation receipts.
            stream.item(family, Status::Gap, 348, [ZERO; 3], [348, 0, 0])
        }
        Family::Publications | Family::Security => control(stream, root, family),
        Family::Credentials => {
            let binding = input.protector.initialization_key_binding()
                .map_err(|error| error.code().to_owned())?;
            stream.item(family, Status::Present, 20, [binding, ZERO, ZERO], [0, 0, 0])?;
            stream.item(family, Status::Present, 21, [crate::direct_preparation::profile_digest(), ZERO, ZERO], [1, 1, 0])
        }
        Family::Limits => stream.item(family, Status::Present, 22,
            [*legacy_inventory_profile_digest().as_bytes(), ZERO, ZERO],
            [LegacyInventoryLimits::BASELINE.records, LegacyInventoryLimits::BASELINE.encoded_bytes,
                LegacyInventoryLimits::BASELINE.page_names as u64]),
    }
}

fn sources(stream: &mut Stream<'_, '_, '_, '_>, target: SourceNamespaceId) -> Result<(), String> {
    let source = stream.source;
    let header = source.source_mapping_header(target)?;
    stream.item(Family::Sources, Status::Present, 2,
        [header.legacy_namespace, header.catalog_snapshot, ZERO], [header.events, header.sources, 0])?;
    source.compile_source_mapping_with_rows(target, source.native().ok_or_else(changed)?.context.deadline(),
        |_| source.checkpoint(), |row| {
            stream.reserve_row()?;
            let encoded = stream.encoder.source_event(&row).map_err(super::codec_reason)?;
            stream.statuses[Status::Present as usize] += 1;
            stream.send(&encoded)?;
            if row.lifecycle.opens_revision() {
                stream.item(Family::Sources, Status::Present, 3,
                    [*row.legacy_source.as_bytes(), *row.legacy_revision.as_bytes(), *row.event.as_bytes()],
                    [row.occurrence, row.sequence, row.source_bytes])?;
            }
            Ok(())
        })?;
    for head in source.inventory_heads() {
        source.charge(Work::Compare(192))?;
        stream.item(Family::Sources, Status::Present, 4,
            [digest(&head.source_id)?, digest(&head.revision_id)?, digest(&head.record_digest)?],
            [head.sequence, head.byte_length, u64::from(head.state.tag() == "A")])?;
    }
    Ok(())
}

fn roots(stream: &mut Stream<'_, '_, '_, '_>, root: &Path,
    input: &NativeMigrationInput<'_, '_, '_>) -> Result<(), String> {
    let source = stream.source;
    let current = io::read(source, &root.join("control/source-roots.v1"), 64 * 1024)?;
    match (current, input.registration) {
        (Ok(current), Some(original)) if current.as_slice() == original => {
            let raw = io::fingerprint(source, original)?;
            stream.item(Family::Roots, Status::Present, 5, [raw, ZERO, ZERO], [original.len() as u64, 0, 0])?;
            source.charge(Work::Allocate(original.len().saturating_mul(2)))?;
            source.charge(Work::Compare(original.len()))?;
            let decoded = crate::source_roots::migration_input_from_bytes(root, Some(original))
                .map_err(|_| changed())?;
            for path in &decoded.paths {
                source.charge(Work::Allocate(2048))?;
                let native = io::native_name(path.as_os_str());
                let reference = io::fingerprint(source, &native)?;
                stream.item(Family::Roots, Status::Present, 6, [reference, raw, ZERO], [0, 0, 0])?;
            }
        }
        (Err(Status::AbsentProven), None) => {
            stream.item(Family::Roots, Status::AbsentProven, 5, [ZERO; 3], [0; 3])?;
        }
        _ => return Err(changed()),
    }
    Ok(())
}

fn manifests(stream: &mut Stream<'_, '_, '_, '_>, root: &Path,
    target: SourceNamespaceId) -> Result<(), String> {
    let source = stream.source;
    let directory = root.join("control/directory-manifests");
    let namespace = source.source_mapping_header(target)?.legacy_namespace;
    let outcome = io::visit(source, &directory, |name| {
        let path = directory.join(&name.value);
        let subject = io::fingerprint(source, &name.key)?;
        match io::read(source, &path, io::CONTROL_BYTES)? {
            Ok(bytes) => {
                let evidence = io::fingerprint(source, &bytes)?;
                source.charge(Work::Compare(bytes.len().saturating_mul(2)))?;
                source.charge(Work::Allocate(8192))?;
                // Validate the complete digest/order before emitting any bindings.
                match crate::directory_manifest::inventory_manifest(&path, &bytes, &namespace, |_| Ok(())) {
                    Ok((generation, count, directory_id, manifest)) => {
                        stream.item(Family::DirectoryManifests, Status::Present, 7,
                            [subject, evidence, manifest], [generation, count, bytes.len() as u64])?;
                        crate::directory_manifest::inventory_manifest(&path, &bytes, &namespace, |refs| {
                            stream.item(Family::DirectoryManifests, Status::Present, 8, refs, [generation, 0, 0])
                        })?;
                        let _ = directory_id;
                    }
                    Err(_) => stream.item(Family::DirectoryManifests, Status::Corrupt, 7,
                        [subject, evidence, ZERO], [bytes.len() as u64, 0, 0])?,
                }
            }
            Err(status) => stream.item(Family::DirectoryManifests, status, 7, [subject, ZERO, ZERO], [0; 3])?,
        }
        Ok(())
    })?;
    directory_outcome(stream, Family::DirectoryManifests, outcome)?;
    // Physical exhaustion cannot prove an entirely lost historical generation.
    stream.item(Family::DirectoryManifests, Status::Gap, 23, [ZERO; 3], [0; 3])
}

fn revisions(stream: &mut Stream<'_, '_, '_, '_>, root: &Path,
    input: &NativeMigrationInput<'_, '_, '_>) -> Result<(), String> {
    let source = stream.source;
    for revision in source.retained_revisions() {
        let id = digest(&revision.revision_id)?;
        let verified = super::super::super::read_import_revision_with_context(
            root, Some(input.protector), &revision, input.context);
        // A native/context refusal is terminal, not a row claiming corrupt bytes.
        source.checkpoint()?;
        let status = match verified {
            Ok(bytes) => { drop(bytes); Status::Present }
            Err(ref reason) if reason.contains("READ") || reason.contains("UNAVAILABLE") => Status::Unreadable,
            Err(ref reason) if reason.contains("MISSING") => Status::Gap,
            Err(_) => Status::Corrupt,
        };
        stream.item(Family::Revisions, status, 9,
            [id, digest(&revision.content_digest)?, digest(&revision.source_id)?], [revision.byte_length, 0, 0])?;
    }
    revision_tree(stream, root, Family::Revisions, false)?;
    if stream.statuses.iter().all(|count| *count == 0) {
        stream.item(Family::Revisions, Status::AbsentProven, 9, [ZERO; 3], [0; 3])?;
    }
    Ok(())
}

fn revision_tree(stream: &mut Stream<'_, '_, '_, '_>, root: &Path,
    family: Family, residue_only: bool) -> Result<(), String> {
    let source = stream.source;
    let directory = root.join(LEGACY_REVISION_DIRECTORY);
    let outcome = io::visit(source, &directory, |shard| {
        let path = directory.join(&shard.value);
        if !shard.value.to_str().is_some_and(is_legacy_revision_inventory_shard) {
            return stream.item(family, Status::Unsupported, 10,
                [io::fingerprint(source, &shard.key)?, ZERO, ZERO], [0; 3]);
        }
        let outcome = io::visit(source, &path, |name| {
            let subject = io::fingerprint(source, &name.key)?;
            let classified = name.value.to_str().and_then(classify_legacy_revision_inventory_name);
            let (status, kind, reference) = match classified {
                Some(value) => {
                    let referenced = source.retained_revision(value.id()).is_some();
                    let temporary = value.physical_kind() == LegacyRevisionPhysicalKind::Temporary;
                    if residue_only && referenced && !temporary { return Ok(()); }
                    (if temporary || !referenced { Status::Orphan } else { Status::Present },
                        if temporary { 11 } else { 10 }, digest(value.id())?)
                }
                None => (Status::Unsupported, 10, ZERO),
            };
            match io::read(source, &path.join(&name.value), LEGACY_REVISION_MAX_OBJECT_BYTES)? {
                Ok(bytes) => stream.item(family, status, kind,
                    [subject, io::fingerprint(source, &bytes)?, reference], [bytes.len() as u64, 0, 0]),
                Err(observed) => stream.item(family, if observed == Status::AbsentProven { Status::Gap } else { observed },
                    kind, [subject, ZERO, reference], [0; 3]),
            }
        })?;
        directory_outcome(stream, family, outcome)
    })?;
    directory_outcome(stream, family, outcome)
}

fn preparation(stream: &mut Stream<'_, '_, '_, '_>, root: &Path) -> Result<(), String> {
    let source = stream.source;
    let base = root.join(LEGACY_PREPARATION_DIRECTORY);
    for tree in [PrepTree::References, PrepTree::Objects] {
        let directory = base.join(tree.directory_name());
        let outcome = io::visit(source, &directory, |shard| {
            let path = directory.join(&shard.value);
            if !shard.value.to_str().is_some_and(is_legacy_revision_inventory_shard) {
                return stream.item(Family::Preparation, Status::Unsupported, 12,
                    [io::fingerprint(source, &shard.key)?, ZERO, ZERO], [0; 3]);
            }
            let outcome = io::visit(source, &path, |name| {
                let subject = io::fingerprint(source, &name.key)?;
                let classified = name.value.to_str().and_then(|name|
                    classify_legacy_preparation_inventory_name(tree, name));
                let status = classified.map_or(Status::Unsupported, |value|
                    if value.kind().tag() == "uncommitted_temporary_object" { Status::Orphan } else { Status::Present });
                match io::read(source, &path.join(&name.value), LEGACY_REVISION_MAX_OBJECT_BYTES)? {
                    Ok(bytes) => {
                        let evidence = io::fingerprint(source, &bytes)?;
                        if tree == PrepTree::References && status == Status::Present {
                            let key = classified.map(|value| digest(value.id())).transpose()?.ok_or_else(changed)?;
                            match decode_legacy_preparation_reference(&bytes, &key) {
                                Ok(reference) => stream.item(Family::Preparation, Status::Present, 12,
                                    [key, *reference.manifest_digest(), evidence],
                                    [reference.manifest_bytes(), u64::from(reference.protection().tag()), LEGACY_PREPARATION_REFERENCE_BYTES as u64]),
                                Err(_) => stream.item(Family::Preparation, Status::Corrupt, 12,
                                    [subject, evidence, ZERO], [bytes.len() as u64, 0, 0]),
                            }
                        } else { stream.item(Family::Preparation, status, 13,
                            [subject, evidence, ZERO], [bytes.len() as u64, 0, 0]) }
                    }
                    Err(status) => stream.item(Family::Preparation, if status == Status::AbsentProven { Status::Gap } else { status },
                        12, [subject, ZERO, ZERO], [0; 3]),
                }
            })?;
            directory_outcome(stream, Family::Preparation, outcome)
        })?;
        directory_outcome(stream, Family::Preparation, outcome)?;
    }
    stream.item(Family::Preparation, Status::Present, 21,
        [crate::direct_preparation::profile_digest(), ZERO, ZERO], [1, 1, 0])
}

fn residue(stream: &mut Stream<'_, '_, '_, '_>, root: &Path) -> Result<(), String> {
    revision_tree(stream, root, Family::Residue, true)?;
    control(stream, root, Family::Residue)
}

fn control(stream: &mut Stream<'_, '_, '_, '_>, root: &Path, family: Family) -> Result<(), String> {
    let source = stream.source;
    let directory = root.join("control");
    let outcome = io::visit(source, &directory, |name| {
        let text = name.value.to_str().unwrap_or("");
        let publication = text.contains("publication") || text.contains("manifest") || text.contains("shadow")
            || text.contains("epoch") || text == "control-cutover.v1";
        let security = text.contains("purge") || text.contains("quarantine") || text.contains("retention");
        let operation = text.contains("operation") || text.contains("receipt") || text.contains("intent");
        let residue = text.ends_with(".tmp") || text.ends_with(".bak") || text.ends_with(".pending");
        let selected = match family { Family::Publications => publication, Family::Security => security,
            Family::Operations => operation, Family::Residue => residue, _ => false };
        if !selected { return Ok(()); }
        let subject = io::fingerprint(source, &name.key)?;
        let path = directory.join(&name.value);
        match io::metadata(source, &path)? {
            io::Observed::Object(meta) if meta.is_dir() => {
                // Every known metadata container is visited, not mistaken for a file.
                control_directory(stream, &path, family, 0)
            }
            _ => control_file(stream, &path, family, subject, residue),
        }
    })?;
    directory_outcome(stream, family, outcome)?;
    if family == Family::Publications || family == Family::Security {
        // A legacy layout has no complete authoritative H5 reference denominator.
        stream.item(family, Status::Gap, 24, [ZERO; 3], [0; 3])?;
    } else if stream.statuses.iter().all(|count| *count == 0) {
        stream.item(family, Status::AbsentProven, 14, [ZERO; 3], [0; 3])?;
    }
    Ok(())
}

fn control_directory(stream: &mut Stream<'_, '_, '_, '_>, directory: &Path,
    family: Family, depth: usize) -> Result<(), String> {
    if depth > 3 { return stream.item(family, Status::Unsupported, 14, [ZERO; 3], [depth as u64, 0, 0]); }
    let source = stream.source;
    let outcome = io::visit(source, directory, |name| {
        let path = directory.join(&name.value);
        let subject = io::fingerprint(source, &name.key)?;
        match io::metadata(source, &path)? {
            io::Observed::Object(meta) if meta.is_dir() => control_directory(stream, &path, family, depth + 1),
            _ => control_file(stream, &path, family, subject, family == Family::Residue),
        }
    })?;
    directory_outcome(stream, family, outcome)
}

fn control_file(stream: &mut Stream<'_, '_, '_, '_>, path: &Path,
    family: Family, subject: [u8; 32], orphan: bool) -> Result<(), String> {
    let source = stream.source;
    match io::read(source, path, io::CONTROL_BYTES)? {
        Ok(bytes) => {
            let fingerprint = io::fingerprint(source, &bytes)?;
            if path.file_name().and_then(|value| value.to_str()) == Some("control-cutover.v1") {
                match search_control_redb::migration::ControlCutoverMarker::decode(&bytes) {
                    Ok(marker) => stream.item(family, Status::Present, 15,
                        [subject, fingerprint, marker.catalog_snapshot], [marker.epoch, bytes.len() as u64, 0]),
                    Err(_) => stream.item(family, Status::Corrupt, 15,
                        [subject, fingerprint, ZERO], [bytes.len() as u64, 0, 0]),
                }
            } else { stream.item(family, if orphan { Status::Orphan } else { Status::Present }, 14,
                [subject, fingerprint, ZERO], [bytes.len() as u64, 0, 0]) }
        }
        Err(status) => stream.item(family, if status == Status::AbsentProven { Status::Gap } else { status },
            14, [subject, ZERO, ZERO], [0; 3]),
    }
}

fn directory_outcome(stream: &mut Stream<'_, '_, '_, '_>, family: Family,
    outcome: Option<Status>) -> Result<(), String> {
    if let Some(status) = outcome { stream.item(family, status, 16, [ZERO; 3], [0; 3])?; }
    Ok(())
}

fn digest(value: &str) -> Result<[u8; 32], String> { sha256::decode_digest(value).ok_or_else(changed) }
fn changed() -> String { "DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned() }
