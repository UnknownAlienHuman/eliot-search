//! Import the typed source mapping into an inactive redb artifact. The source
//! mapper is replayed for exact verification; no JSON parser or live catalog is added.

use std::fs::{self, File};
use std::path::Path;
use std::time::Instant;

use search_contracts::{Sha256Digest32, SourceNamespaceId};
use search_control_redb::migration::{
    SourceContentManifest, SourceImportBinding, SourceImportCounts,
    SourceImportOutputArtifact, SourceImportOutputArtifactError,
    SourceImportOutputArtifactPlatform, SourceImportOutputLockPlatform,
    SourceImportRecordChain, SourceImportRecordChainError,
    SourceMappingImport, SourceMappingReadback,
};

use super::content_readback::ContentArtifact;
use super::{
    check_deadline, ensure_directory, fingerprint, regular, sha256,
    sync_directory,
};
use crate::plaintext_direct_store::DirectStore;

type ImportOutput = SourceImportOutputArtifact<DaemonImportOutputPlatform>;

/// The caller retains source exclusion and has already verified the text plan.
/// A complete existing database is rechecked, not overwritten or treated as live control.
pub(super) fn store(
    source: &DirectStore,
    target: SourceNamespaceId,
    directory: &Path,
    plan_chain: [u8; 32],
    expected: SourceImportCounts,
    content: &ContentArtifact,
    deadline: Instant,
) -> Result<(String, bool), String> {
    check_deadline(Some(deadline))?;
    ensure_directory(directory)?;
    let header = source.source_mapping_header(target)?;
    let binding = header.import_binding(plan_chain);
    if content.records != source.retained_revisions().len() as u64 {
        return Err("DIRECT_MIGRATION_CONTENT_COUNT_MISMATCH".to_owned());
    }
    let content_binding = SourceContentManifest {
        target_namespace: target,
        legacy_namespace: Sha256Digest32::from_bytes(header.legacy_namespace),
        catalog_snapshot: Sha256Digest32::from_bytes(header.catalog_snapshot),
        source_plan: Sha256Digest32::from_bytes(plan_chain),
        profile: Sha256Digest32::from_bytes(content.profile),
        manifest_chain: Sha256Digest32::from_bytes(content.chain),
        manifest_bytes: content.encoded_bytes,
        objects: content.records,
        source_bytes: content.source_bytes,
    };
    verify_content(directory, content, deadline)?;

    let target_digest = sha256::digest_parts(
        b"eliot-search/source-content-import/v2",
        &[&plan_chain, &content.chain],
    );
    let name = format!("{}.source-map.v2.redb", sha256::hex(&target_digest));
    let output = ImportOutput::acquire(
        directory,
        &name,
        DaemonImportOutputPlatform,
        deadline,
    )
    .map_err(output_artifact_reason)?;

    if let Some(final_artifact) = output
        .open_final(deadline)
        .map_err(output_artifact_reason)?
    {
        let final_identity = *final_artifact.identity();
        verify_mapping(
            source,
            final_artifact.into_file(),
            binding,
            content_binding,
            expected,
            deadline,
        )?;
        verify_content(directory, content, deadline)?;
        output
            .cleanup_verified_alias(&final_identity, deadline)
            .map_err(output_artifact_reason)?;
        return Ok((name, true));
    }

    let pending = output
        .open_or_create_pending(deadline)
        .map_err(output_artifact_reason)?;
    let pending_identity = *pending.identity();
    let created = pending.created();
    let mut writer = if created {
        SourceMappingImport::create_with_content(
            pending.into_file(),
            binding,
            content_binding,
            deadline,
        )
    } else {
        SourceMappingImport::resume_with_content(
            pending.into_file(),
            binding,
            content_binding,
            deadline,
        )
    }
    .map_err(|error| error.code().to_owned())?;
    if created {
        output
            .sync_pending_creation(deadline)
            .map_err(output_artifact_reason)?;
    }

    let mut hash = SourceImportRecordChain::new();
    let summary = source.compile_source_mapping_with_rows(
        target,
        deadline,
        |encoded| hash.push(encoded).map_err(record_chain_reason),
        |row| {
            writer
                .push(row, deadline)
                .map_err(|error| error.code().to_owned())
        },
    )?;
    if summary.import_counts() != expected || hash.finish() != plan_chain {
        return Err("DIRECT_MIGRATION_IMPORT_SOURCE_CHANGED".to_owned());
    }
    writer
        .finish(expected, deadline)
        .map_err(|error| error.code().to_owned())?;

    let pending = output
        .open_pending_matching(&pending_identity, deadline)
        .map_err(output_artifact_reason)?;
    verify_mapping(
        source,
        pending.into_file(),
        binding,
        content_binding,
        expected,
        deadline,
    )?;

    let published = output
        .publish_pending(&pending_identity, deadline)
        .map_err(output_artifact_reason)?;
    let reused = published.reused();
    let final_identity = *published.identity();
    verify_mapping(
        source,
        published.into_file(),
        binding,
        content_binding,
        expected,
        deadline,
    )?;
    verify_content(directory, content, deadline)?;
    output
        .cleanup_verified_alias(&final_identity, deadline)
        .map_err(output_artifact_reason)?;
    Ok((name, reused))
}

fn verify_mapping(
    source: &DirectStore,
    file: File,
    binding: SourceImportBinding,
    content: SourceContentManifest,
    expected: SourceImportCounts,
    deadline: Instant,
) -> Result<(), String> {
    let mut reader = SourceMappingReadback::open_with_content(
        file,
        binding,
        content,
        expected,
        deadline,
    )
    .map_err(|error| error.code().to_owned())?;
    let mut hash = SourceImportRecordChain::new();
    let summary = source.compile_source_mapping_with_rows(
        binding.target_namespace,
        deadline,
        |encoded| hash.push(encoded).map_err(record_chain_reason),
        |row| {
            reader
                .compare(&row, deadline)
                .map_err(|error| error.code().to_owned())
        },
    )?;
    if summary.import_counts() != expected
        || hash.finish() != *binding.plan_chain.as_bytes()
    {
        return Err("DIRECT_MIGRATION_IMPORT_SOURCE_CHANGED".to_owned());
    }
    reader
        .finish(deadline)
        .map_err(|error| error.code().to_owned())?;
    check_deadline(Some(deadline))
}

fn verify_content(
    directory: &Path,
    content: &ContentArtifact,
    deadline: Instant,
) -> Result<(), String> {
    let expected = format!("{}.source-content.v1", sha256::hex(&content.chain));
    if content.name != expected
        || fingerprint(
            &directory.join(&expected),
            content.encoded_bytes,
            deadline,
        )? != content.chain
    {
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct DaemonImportOutputPlatform;

impl SourceImportOutputLockPlatform for DaemonImportOutputPlatform {
    type Error = String;

    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
        ensure_directory(path)
    }

    fn verify_locator(
        &self,
        expected: &File,
        path: &Path,
    ) -> Result<(), Self::Error> {
        verify_locator(expected, path)
    }

    fn sync_directory(&self, path: &Path) -> Result<(), Self::Error> {
        #[cfg(unix)]
        {
            sync_directory(path)
        }
        #[cfg(not(unix))]
        {
            sync_directory(path);
            Ok(())
        }
    }
}

impl SourceImportOutputArtifactPlatform for DaemonImportOutputPlatform {
    type Identity = (u64, u64);

    fn identity(
        &self,
        file: &File,
    ) -> Result<Self::Identity, Self::Error> {
        native_identity(file)
    }
}

fn output_artifact_reason(
    error: SourceImportOutputArtifactError<String>,
) -> String {
    error.into_reason()
}

fn record_chain_reason(error: SourceImportRecordChainError) -> String {
    error.code().to_owned()
}

pub(super) fn native_identity(file: &File) -> Result<(u64, u64), String> {
    let invalid = || "DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned();
    let metadata = file.metadata().map_err(|_| invalid())?;
    if !regular(&metadata) {
        return Err(invalid());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok((metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        let observed =
            eliot_searchd::native_file::observe(file).map_err(|_| invalid())?;
        Ok((u64::from(observed.volume_serial), observed.file_index))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err("DIRECT_MIGRATION_LOCK_PLATFORM_UNSUPPORTED".to_owned())
    }
}

pub(super) fn verify_locator(
    expected: &File,
    path: &Path,
) -> Result<(), String> {
    let invalid = || "DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned();
    ensure_directory(path.parent().ok_or_else(invalid)?)?;
    if !regular(&fs::symlink_metadata(path).map_err(|_| invalid())?) {
        return Err(invalid());
    }
    let current = File::open(path).map_err(|_| invalid())?;
    if native_identity(&current)? != native_identity(expected)? {
        return Err(invalid());
    }
    Ok(())
}
