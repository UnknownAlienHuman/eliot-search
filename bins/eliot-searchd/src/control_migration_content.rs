//! Persist canonical content fingerprints for the existing source-mapping plan.
//! Read retained objects, never live paths. No source bodies enter the artifact.
//!
//! `search-control-redb::migration` owns the frozen manifest line schema,
//! canonical record chain, immutable artifact lifecycle and accounting state
//! machine. This adapter owns only retained-byte readback and BLAKE3 computation.

use std::path::Path;
use std::time::Instant;

use search_contracts::{
    Blake3Digest32, Sha256Digest32, SourceNamespaceId,
};
use search_control_redb::migration::{
    SourceContentManifestEncoder, SourceContentManifestEncodingError,
    SourceContentManifestHeader, SourceContentManifestSummary,
    SourceContentObjectReadback, SourceImportRecordArtifact,
    SourceImportRecordArtifactError, SourceImportRecordArtifactPublication, source_content_profile_digest,
    MAX_SOURCE_IMPORT_ROW_BYTES,
};
use zeroize::Zeroizing;

use super::super::super::migration_context::MigrationSource;
use search_revision_store::LegacyRevisionObjectWork as Work;
use crate::revision_protection::RevisionProtector;

use super::super::{read_import_revision, read_import_revision_with_context};
use super::{
    ensure_directory, sha256, temporary_staging_name,
};

pub(super) struct ContentArtifact {
    pub(super) name: String,
    pub(super) chain: [u8; 32],
    pub(super) records: u64,
    pub(super) source_bytes: u64,
    pub(super) encoded_bytes: u64,
    pub(super) profile: [u8; 32],
}

// Constructed only after both independent source-backed artifact passes finish.
pub(super) struct PreparedContent {
    pub(super) publication: SourceImportRecordArtifactPublication<super::redb_import::DaemonImportOutputPlatform>,
}

impl PreparedContent {
    pub(super) fn from_verified(
        publication: SourceImportRecordArtifactPublication<super::redb_import::DaemonImportOutputPlatform>,
        header: SourceContentManifestHeader,
        summary: SourceContentManifestSummary,
    ) -> Result<Self, String> {
        if summary.objects != header.expected_objects {
            return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
        }
        Ok(Self { publication })
    }
}

/// Publication is inert until the complete importer consumes its exact bindings.
/// Each verification pass rereads every retained object with the selected protector;
/// no digest cache can hide a missing or changed source during the second pass.
pub(super) fn stage(
    source: &MigrationSource<'_, '_, '_>,
    root: &Path,
    target: SourceNamespaceId,
    source_plan: [u8; 32],
    directory: &Path,
    deadline: Instant,
) -> Result<ContentArtifact, String> {
    source.checkpoint()?;
    source.charge(Work::Observe)?;
    ensure_directory(directory)?;
    source.checkpoint()?;
    let temporary_name = temporary_staging_name()?;
    source.checkpoint()?;
    source.charge(Work::Open)?;
    let mut artifact = SourceImportRecordArtifact::create(
        directory,
        &temporary_name,
        super::redb_import::DaemonImportOutputPlatform,
        deadline,
    )
    .map_err(content_artifact_reason)?;
    source.checkpoint()?;
    let counts = compile(
        source,
        root,
        target,
        source_plan,
        deadline,
        |row| {
            source.charge(Work::Write(row.len()))?;
            artifact
                .push(row, deadline)
                .map_err(content_artifact_reason)?;
            source.checkpoint()
        },
    )?;
    source.checkpoint()?;
    let frozen = artifact
        .freeze(deadline)
        .map_err(content_artifact_reason)?;
    source.checkpoint()?;
    let digest = *frozen.chain();
    let length = frozen.encoded_bytes();

    // Recompute every digest and compare the exact generated rows with the
    // package-owned temporary artifact before publishing it.
    let mut readback = frozen
        .begin_readback(deadline)
        .map_err(content_artifact_reason)?;
    source.checkpoint()?;
    let observed = compile(
        source,
        root,
        target,
        source_plan,
        deadline,
        |row| {
            source.charge(Work::Compare(row.len()))?;
            readback
                .compare(row, deadline)
                .map_err(content_artifact_reason)?;
            source.checkpoint()
        },
    )?;
    if observed != counts {
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    source.checkpoint()?;
    let verified = readback
        .finish(deadline)
        .map_err(content_artifact_reason)?;
    source.checkpoint()?;
    let name = format!("{}.source-content.v1", sha256::hex(&digest));
    source.checkpoint()?;
    let published = verified
        .publish(&name, deadline)
        .map_err(content_artifact_reason)?;
    source.checkpoint()?;
    if published.chain() != &digest
        || published.encoded_bytes() != length
        || published.name() != name.as_str()
    {
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    source.checkpoint()?;
    Ok(ContentArtifact {
        name,
        chain: digest,
        records: counts.objects,
        source_bytes: counts.source_bytes,
        encoded_bytes: length,
        profile: *source_content_profile_digest().as_bytes(),
    })
}

pub(super) fn compile(
    source: &MigrationSource<'_, '_, '_>,
    root: &Path,
    target: SourceNamespaceId,
    source_plan: [u8; 32],
    deadline: Instant,
    mut emit: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<SourceContentManifestSummary, String> {
    source.checkpoint()?;
    let source_header = source.source_mapping_header(target)?;
    if source.verify_migration_snapshot(deadline)?
        != source_header.catalog_snapshot
    {
        return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
    }

    #[cfg(windows)]
    let legacy_protector = if source.native().is_none() {
        RevisionProtector::open_existing(source_header.legacy_namespace)?
    } else { None };
    #[cfg(not(windows))]
    let legacy_protector = if source.native().is_none() { Some(RevisionProtector::open(
        source_header.legacy_namespace,
        &root.join("revisions"),
    )?) } else { None };
    let protector = source.native().map(|input| input.protector).or(legacy_protector.as_ref());
    source.checkpoint()?;

    let expected = source.retained_revisions().len() as u64;
    source.charge(Work::Allocate(search_control_redb::migration::MAX_SOURCE_IMPORT_ROW_BYTES))?;
    source.charge(Work::Compare(search_control_redb::migration::MAX_SOURCE_IMPORT_ROW_BYTES))?;
    let mut encoder = SourceContentManifestEncoder::new(
        SourceContentManifestHeader {
            target_namespace: target,
            legacy_namespace: Sha256Digest32::from_bytes(
                source_header.legacy_namespace,
            ),
            catalog_snapshot: Sha256Digest32::from_bytes(
                source_header.catalog_snapshot,
            ),
            source_plan: Sha256Digest32::from_bytes(source_plan),
            expected_objects: expected,
        },
    )
    .map_err(encoding_reason)?;
    let header_row = encoder.header_row().map_err(encoding_reason)?;
    emit(&header_row)?;

    for metadata in source.retained_revisions() {
        source.checkpoint()?;
        let bytes = source.native().map_or_else(
            || read_import_revision(root, protector, &metadata, deadline),
            |input| read_import_revision_with_context(root, protector, &metadata, input.context),
        )?;
        source.checkpoint()?;
        source.charge(Work::Allocate(search_control_redb::migration::MAX_SOURCE_IMPORT_ROW_BYTES))?;
        source.charge(Work::Compare(search_control_redb::migration::MAX_SOURCE_IMPORT_ROW_BYTES))?;
        let row = encoder
            .object_row(object_observation(source, &metadata, &bytes)?)
            .map_err(encoding_reason)?;
        emit(&row)?;
    }

    if source.verify_migration_snapshot(deadline)?
        != source_header.catalog_snapshot
    {
        return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
    }
    source.charge(Work::Allocate(search_control_redb::migration::MAX_SOURCE_IMPORT_ROW_BYTES))?;
    source.charge(Work::Compare(search_control_redb::migration::MAX_SOURCE_IMPORT_ROW_BYTES))?;
    let (end_row, summary) = encoder.finish().map_err(encoding_reason)?;
    emit(&end_row)?;
    source.checkpoint()?;
    Ok(summary)
}

/// The existing v1 encoder driven by the inventory's pinned source read.
/// The sink streams rows into the original artifact or its independent compare.
pub(super) struct ContentPass<'sink> {
    encoder: SourceContentManifestEncoder,
    header: SourceContentManifestHeader,
    sink: &'sink mut dyn FnMut(&[u8]) -> Result<(), String>,
}

impl<'sink> ContentPass<'sink> {
    pub(super) fn new(
        source: &MigrationSource<'_, '_, '_>, target: SourceNamespaceId,
        source_plan: [u8; 32], deadline: Instant,
        sink: &'sink mut dyn FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<Self, String> {
        let source_header = source.source_mapping_header(target)?;
        if source.verify_migration_snapshot(deadline)? != source_header.catalog_snapshot {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        let header = SourceContentManifestHeader {
            target_namespace: target,
            legacy_namespace: Sha256Digest32::from_bytes(source_header.legacy_namespace),
            catalog_snapshot: Sha256Digest32::from_bytes(source_header.catalog_snapshot),
            source_plan: Sha256Digest32::from_bytes(source_plan),
            expected_objects: source.retained_revisions().len() as u64,
        };
        source.charge(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES))?;
        source.charge(Work::Compare(MAX_SOURCE_IMPORT_ROW_BYTES))?;
        let mut encoder = SourceContentManifestEncoder::new(header).map_err(encoding_reason)?;
        sink(&encoder.header_row().map_err(encoding_reason)?)?;
        source.checkpoint()?;
        Ok(Self { encoder, header, sink })
    }

    pub(super) const fn header(&self) -> SourceContentManifestHeader { self.header }

    pub(super) fn observe(
        &mut self, source: &MigrationSource<'_, '_, '_>,
        metadata: &crate::plaintext_direct_store::RevisionMetadata,
        readback: &super::super::RevisionReadback,
    ) -> Result<(), String> {
        let object = object_observation(source, metadata, readback.bytes())?;
        source.charge(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES))?;
        source.charge(Work::Compare(MAX_SOURCE_IMPORT_ROW_BYTES))?;
        (self.sink)(&self.encoder.object_row(object).map_err(encoding_reason)?)?;
        source.checkpoint()
    }

    pub(super) fn finish(
        mut self, source: &MigrationSource<'_, '_, '_>, deadline: Instant,
    ) -> Result<SourceContentManifestSummary, String> {
        if source.verify_migration_snapshot(deadline)? != *self.header.catalog_snapshot.as_bytes() {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        source.charge(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES))?;
        source.charge(Work::Compare(MAX_SOURCE_IMPORT_ROW_BYTES))?;
        let (row, summary) = self.encoder.finish().map_err(encoding_reason)?;
        (self.sink)(&row)?;
        source.checkpoint()?;
        Ok(summary)
    }
}

fn object_observation(
    source: &MigrationSource<'_, '_, '_>, metadata: &crate::plaintext_direct_store::RevisionMetadata,
    bytes: &[u8],
) -> Result<SourceContentObjectReadback, String> {
    let mut hasher = Zeroizing::new(blake3::Hasher::new());
    for chunk in bytes.chunks(256 * 1024) {
        source.charge(Work::Compare(chunk.len()))?;
        hasher.update(chunk);
        source.checkpoint()?;
    }
    Ok(SourceContentObjectReadback {
        legacy_source_id: legacy_digest(&metadata.source_id)?,
        legacy_revision_id: legacy_digest(&metadata.revision_id)?,
        content_sha256: legacy_digest(&metadata.content_digest)?,
        byte_length: metadata.byte_length,
        content_blake3: Blake3Digest32::from_bytes(*hasher.finalize().as_bytes()),
    })
}

fn legacy_digest(value: &str) -> Result<Sha256Digest32, String> {
    Sha256Digest32::parse_hex(value)
        .map_err(|_| "DIRECT_CONTROL_READBACK_MISMATCH".to_owned())
}

pub(super) fn owned_artifact_reason(
    context: &super::inventory::ArtifactContext<'_, '_, '_, '_>,
    error: SourceImportRecordArtifactError<String>,
) -> String {
    let reason = context.reason(error);
    reason.strip_prefix("DIRECT_MIGRATION_INVENTORY_")
        .map_or_else(|| reason.clone(), |suffix| format!("DIRECT_MIGRATION_CONTENT_{suffix}"))
}

fn encoding_reason(error: SourceContentManifestEncodingError) -> String {
    error.code().to_owned()
}

fn content_artifact_reason(
    error: SourceImportRecordArtifactError<String>,
) -> String {
    match error {
        SourceImportRecordArtifactError::Context(_) => "DIRECT_MIGRATION_UNAVAILABLE".to_owned(),
        SourceImportRecordArtifactError::Platform(reason) => reason,
        SourceImportRecordArtifactError::DeadlineExceeded => {
            "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned()
        }
        SourceImportRecordArtifactError::TemporaryNameInvalid
        | SourceImportRecordArtifactError::CreateFailed => {
            "DIRECT_MIGRATION_PLAN_CREATE_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::FinalNameInvalid
        | SourceImportRecordArtifactError::ObjectInvalid => {
            "DIRECT_MIGRATION_PLAN_OBJECT_INVALID".to_owned()
        }
        SourceImportRecordArtifactError::Closed => {
            "DIRECT_MIGRATION_PLAN_CLOSED".to_owned()
        }
        SourceImportRecordArtifactError::WriteFailed => {
            "DIRECT_MIGRATION_CONTENT_WRITE_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::SyncFailed => {
            "DIRECT_MIGRATION_CONTENT_SYNC_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::ReadFailed => {
            "DIRECT_MIGRATION_PLAN_READ_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::ComparisonReadFailed => {
            "DIRECT_MIGRATION_CONTENT_READBACK_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::ReadbackMismatch => {
            "DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned()
        }
        SourceImportRecordArtifactError::PublishOutcomeUnknown => {
            "DIRECT_MIGRATION_CONTENT_PUBLISH_OUTCOME_UNKNOWN".to_owned()
        }
        SourceImportRecordArtifactError::ImmutableConflict => {
            "DIRECT_MIGRATION_CONTENT_IMMUTABLE_CONFLICT".to_owned()
        }
        SourceImportRecordArtifactError::IdentityChanged => {
            "DIRECT_MIGRATION_PLAN_CLEANUP_IDENTITY_CHANGED".to_owned()
        }
        SourceImportRecordArtifactError::CleanupFailed => {
            "DIRECT_MIGRATION_PLAN_CLEANUP_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::RecordChain(error) => {
            error.code().to_owned()
        }
    }
}

#[cfg(test)]
mod co_production_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct Scratch { root: PathBuf }
    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let root = std::env::temp_dir().join(format!("search-content-parity-{}-{stamp}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir_all(root.join("data")).unwrap();
            Self { root }
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
            assert!(self.root.file_name().unwrap().to_str().unwrap().starts_with("search-content-parity-"));
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn co_production_content_v1_byte_parity_and_incomplete_stream_refusal() {
        let scratch = Scratch::new();
        let input = scratch.root.join("input.rs");
        fs::write(&input, b"fn bounded_fixture() {}\n").unwrap();
        let mut store = crate::plaintext_direct_store::DirectStore::open(&scratch.root.join("data")).unwrap();
        store.index_file(&input).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let source = MigrationSource::new(&store, deadline, None);
        let target = SourceNamespaceId::from_bytes([17; 16]);
        let plan = [23; 32];
        let mut baseline = Vec::new();
        let expected = compile(&source, source.inventory_root(), target, plan, deadline, |row| {
            baseline.push(row.to_vec()); Ok(())
        }).unwrap();
        let mut produced = Vec::new();
        let mut sink = |row: &[u8]| { produced.push(row.to_vec()); Ok(()) };
        let mut pass = ContentPass::new(&source, target, plan, deadline, &mut sink).unwrap();
        for metadata in source.retained_revisions() {
            // A real legacy readback verifies plaintext SHA/length/identity;
            // this fixture proves wire parity, not NativeCtx co-production.
            let readback = super::super::super::read_revision_objects_at(
                source.inventory_root(), None, &metadata, false, Some(deadline),
                super::super::super::RevisionReadbackMode::RawEvidence,
            ).unwrap();
            pass.observe(&source, &metadata, &readback).unwrap();
        }
        assert_eq!(pass.finish(&source, deadline).unwrap(), expected);
        assert_eq!(produced, baseline);
        let mut discard = |_: &[u8]| Ok(());
        let incomplete = ContentPass::new(&source, target, plan, deadline, &mut discard).unwrap();
        assert!(incomplete.finish(&source, deadline).is_err());
    }
}
