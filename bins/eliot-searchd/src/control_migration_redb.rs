//! Import the typed source mapping into an inactive redb artifact. The source
//! mapper is replayed for exact verification; no JSON parser or live catalog is added.

use std::fs::{self, File};
use std::path::Path;
use std::time::Instant;

use search_contracts::{Sha256Digest32, SourceNamespaceId};
use search_control_redb::migration::{
    SourceContentManifest, SourceImportBinding, SourceImportCounts, SourceImportOutputArtifact,
    SourceImportOutputArtifactError, SourceImportOutputArtifactPlatform,
    SourceImportOutputLockPlatform, SourceImportRecordArtifactError, SourceImportRecordChain,
    SourceImportRecordChainError, SourceMappingImport, SourceMappingReadback,
    inspect_source_import_record_artifact,
};

use super::content_readback::ContentArtifact;
use super::{check_deadline, ensure_directory, sha256, sync_directory};
use crate::plaintext_direct_store::DirectStore;

type ImportOutput = SourceImportOutputArtifact<DaemonImportOutputPlatform>;

struct OriginalCheck<'a> {
    source: &'a DirectStore,
    deadline: Instant,
}

impl OriginalCheck<'_> {
    fn run(&self) -> Result<(), String> {
        self.source.check_operation()?;
        check_deadline(Some(self.deadline))
    }
}

/// Captures an API result, checks the original operation, then propagates.
///
/// This is the single precedence rule for every `store`/`verify_mapping` call:
/// bind, check, then `?`.
fn checked<T>(original: &OriginalCheck<'_>, result: Result<T, String>) -> Result<T, String> {
    original.run()?;
    result
}

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
    let original = OriginalCheck { source, deadline };
    original.run()?;
    checked(&original, ensure_directory(directory))?;
    let header = checked(&original, source.source_mapping_header(target))?;
    let binding = header.import_binding(plan_chain);
    original.run()?;
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
    verify_content(directory, content, &original)?;

    let target_digest = sha256::digest_parts(
        b"eliot-search/source-content-import/v2",
        &[&plan_chain, &content.chain],
    );
    let name = format!("{}.source-map.v2.redb", sha256::hex(&target_digest));
    original.run()?;
    let output = checked(
        &original,
        ImportOutput::acquire(
            directory,
            &name,
            DaemonImportOutputPlatform,
            original.deadline,
        )
        .map_err(output_artifact_reason),
    )?;

    if let Some(final_artifact) = checked(
        &original,
        output
            .open_final(original.deadline)
            .map_err(output_artifact_reason),
    )? {
        let final_identity = *final_artifact.identity();
        verify_mapping(
            source,
            final_artifact.into_file(),
            binding,
            content_binding,
            expected,
            &original,
        )?;
        verify_content(directory, content, &original)?;
        checked(
            &original,
            output
                .cleanup_verified_alias(&final_identity, original.deadline)
                .map_err(output_artifact_reason),
        )?;
        original.run()?;
        return Ok((name, true));
    }

    let pending = checked(
        &original,
        output
            .open_or_create_pending(original.deadline)
            .map_err(output_artifact_reason),
    )?;
    let pending_identity = *pending.identity();
    let created = pending.created();
    original.run()?;
    let opened = if created {
        SourceMappingImport::create_with_content(
            pending.into_file(),
            binding,
            content_binding,
            original.deadline,
        )
    } else {
        SourceMappingImport::resume_with_content(
            pending.into_file(),
            binding,
            content_binding,
            original.deadline,
        )
    };
    let mut writer = checked(&original, opened.map_err(|error| error.code().to_owned()))?;
    if created {
        checked(
            &original,
            output
                .sync_pending_creation(original.deadline)
                .map_err(output_artifact_reason),
        )?;
    }

    let mut hash = SourceImportRecordChain::new();
    let summary = checked(
        &original,
        source.compile_source_mapping_with_rows(
            target,
            original.deadline,
            |encoded| {
                original.run()?;
                checked(&original, hash.push(encoded).map_err(record_chain_reason))
            },
            |row| {
                original.run()?;
                let pushed = writer.push(row, original.deadline);
                checked(&original, pushed.map_err(|error| error.code().to_owned()))
            },
        ),
    )?;
    if summary.import_counts() != expected || hash.finish() != plan_chain {
        original.run()?;
        return Err("DIRECT_MIGRATION_IMPORT_SOURCE_CHANGED".to_owned());
    }
    original.run()?;
    checked(
        &original,
        writer
            .finish(expected, original.deadline)
            .map_err(|error| error.code().to_owned()),
    )?;

    let pending = checked(
        &original,
        output
            .open_pending_matching(&pending_identity, original.deadline)
            .map_err(output_artifact_reason),
    )?;
    verify_mapping(
        source,
        pending.into_file(),
        binding,
        content_binding,
        expected,
        &original,
    )?;

    let published = checked(
        &original,
        output
            .publish_pending(&pending_identity, original.deadline)
            .map_err(output_artifact_reason),
    )?;
    let reused = published.reused();
    let final_identity = *published.identity();
    verify_mapping(
        source,
        published.into_file(),
        binding,
        content_binding,
        expected,
        &original,
    )?;
    verify_content(directory, content, &original)?;
    checked(
        &original,
        output
            .cleanup_verified_alias(&final_identity, original.deadline)
            .map_err(output_artifact_reason),
    )?;
    original.run()?;
    Ok((name, reused))
}

fn verify_mapping(
    source: &DirectStore,
    file: File,
    binding: SourceImportBinding,
    content: SourceContentManifest,
    expected: SourceImportCounts,
    original: &OriginalCheck<'_>,
) -> Result<(), String> {
    original.run()?;
    let opened = SourceMappingReadback::open_with_content(
        file,
        binding,
        content,
        expected,
        original.deadline,
    );
    let mut reader = checked(original, opened.map_err(|error| error.code().to_owned()))?;
    let mut hash = SourceImportRecordChain::new();
    let summary = checked(
        original,
        source.compile_source_mapping_with_rows(
            binding.target_namespace,
            original.deadline,
            |encoded| {
                original.run()?;
                checked(original, hash.push(encoded).map_err(record_chain_reason))
            },
            |row| {
                original.run()?;
                let compared = reader.compare(&row, original.deadline);
                checked(original, compared.map_err(|error| error.code().to_owned()))
            },
        ),
    )?;
    if summary.import_counts() != expected || hash.finish() != *binding.plan_chain.as_bytes() {
        original.run()?;
        return Err("DIRECT_MIGRATION_IMPORT_SOURCE_CHANGED".to_owned());
    }
    original.run()?;
    checked(
        original,
        reader
            .finish(original.deadline)
            .map_err(|error| error.code().to_owned()),
    )
}

/// Private content verification now takes the original callback so both
/// calls pass the same bound source closure.
fn verify_content(
    directory: &Path,
    content: &ContentArtifact,
    original: &OriginalCheck<'_>,
) -> Result<(), String> {
    original.run()?;
    let expected = format!("{}.source-content.v1", sha256::hex(&content.chain));
    if content.name != expected {
        original.run()?;
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    let observed = inspect_source_import_record_artifact(
        &DaemonImportOutputPlatform,
        &directory.join(&expected),
        content.encoded_bytes,
        original.deadline,
    );
    let observed = checked(original, observed.map_err(record_artifact_read_reason))?;
    if observed.chain() != &content.chain {
        original.run()?;
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    original.run()
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DaemonImportOutputPlatform;

impl SourceImportOutputLockPlatform for DaemonImportOutputPlatform {
    type Error = String;

    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
        ensure_directory(path)
    }

    fn verify_locator(&self, expected: &File, path: &Path) -> Result<(), Self::Error> {
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

    fn identity(&self, file: &File) -> Result<Self::Identity, Self::Error> {
        native_identity(file)
    }
}

fn output_artifact_reason(error: SourceImportOutputArtifactError<String>) -> String {
    error.into_reason()
}

fn record_chain_reason(error: SourceImportRecordChainError) -> String {
    error.code().to_owned()
}

fn record_artifact_read_reason(error: SourceImportRecordArtifactError<String>) -> String {
    match error {
        SourceImportRecordArtifactError::Platform(reason) => reason,
        SourceImportRecordArtifactError::DeadlineExceeded => {
            "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned()
        }
        SourceImportRecordArtifactError::ObjectInvalid
        | SourceImportRecordArtifactError::FinalNameInvalid
        | SourceImportRecordArtifactError::TemporaryNameInvalid
        | SourceImportRecordArtifactError::CreateFailed => {
            "DIRECT_MIGRATION_PLAN_OBJECT_INVALID".to_owned()
        }
        SourceImportRecordArtifactError::ReadFailed => {
            "DIRECT_MIGRATION_PLAN_READ_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::ReadbackMismatch
        | SourceImportRecordArtifactError::ComparisonReadFailed => {
            "DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned()
        }
        SourceImportRecordArtifactError::IdentityChanged => {
            "DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned()
        }
        SourceImportRecordArtifactError::RecordChain(error) => error.code().to_owned(),
        SourceImportRecordArtifactError::Closed
        | SourceImportRecordArtifactError::WriteFailed
        | SourceImportRecordArtifactError::SyncFailed
        | SourceImportRecordArtifactError::PublishOutcomeUnknown
        | SourceImportRecordArtifactError::ImmutableConflict
        | SourceImportRecordArtifactError::CleanupFailed => {
            "DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned()
        }
    }
}

fn native_identity(file: &File) -> Result<(u64, u64), String> {
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
        let observed = eliot_searchd::native_file::observe(file).map_err(|_| invalid())?;
        Ok((u64::from(observed.volume_serial), observed.file_index))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err("DIRECT_MIGRATION_LOCK_PLATFORM_UNSUPPORTED".to_owned())
    }
}

fn verify_locator(expected: &File, path: &Path) -> Result<(), String> {
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

fn regular(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let reparse = false;
    metadata.is_file() && !metadata.file_type().is_symlink() && !reparse
}
