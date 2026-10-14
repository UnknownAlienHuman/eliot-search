//! Original source-map publication and in-place row staging, not full candidate authority.
use super::super::super::migration_context::{MigrationSource, NativeMigrationInput};
use super::{bootstrap, inventory, redb_import::DaemonImportOutputPlatform,
    temporary_staging_name, sha256};
use search_contracts::SourceNamespaceId;
use search_control_redb::{ControlError, MigrationBootstrapFailure, MigrationBootstrapWork};
use search_control_redb::migration::{SourceImportRecordArtifact,
    SourceImportRecordArtifactPublication, SourceMappingSummary, SourceMappingHeader,
    SourceMappingManifestDecoder, SourceMappingDecodedRow, SourceImportRow,
    MAX_SOURCE_IMPORT_ROW_BYTES};
use search_revision_store::LegacyRevisionObjectWork as Work;
use std::path::Path;

pub(super) struct PreparedSourceMap {
    pub(super) publication: SourceImportRecordArtifactPublication<DaemonImportOutputPlatform>,
    header: SourceMappingHeader,
    summary: SourceMappingSummary,
}

impl PreparedSourceMap {
    pub(super) fn visit(
        &mut self, source: &MigrationSource<'_, '_, '_>, input: &NativeMigrationInput<'_, '_, '_>,
        mut consume: impl FnMut(SourceImportRow) -> Result<(), String>,
    ) -> Result<(), String> {
        source.charge(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES
            + std::mem::size_of::<SourceMappingManifestDecoder>()))?;
        let limit = usize::try_from(self.header.expected_events).map_err(|_| invalid())?;
        let mut decoder = SourceMappingManifestDecoder::new(self.header, limit)
            .map_err(|error| error.code().to_owned())?;
        transfer_rows(source, &mut self.publication, input, |row| {
            source.charge(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES + 2048))?;
            source.charge(Work::Compare(MAX_SOURCE_IMPORT_ROW_BYTES))?;
            if let SourceMappingDecodedRow::Event(event) = decoder.push(row).map_err(|error| error.code().to_owned())? {
                consume(event)?;
            }
            Ok(())
        })?;
        if decoder.finish().map_err(|error| error.code().to_owned())? != self.summary { return Err(unknown()); }
        Ok(())
    }
}

pub(super) fn stage(source: &MigrationSource<'_, '_, '_>, root: &Path,
    target: SourceNamespaceId, directory: &Path, input: &NativeMigrationInput<'_, '_, '_>)
    -> Result<String, String> {
    let (mut journal, output) = bootstrap::initialize(source, target, directory, input)?;
    let header = source.source_mapping_header(target)?;
    let mut mapping_artifact = prepare_source_map(source, target, directory, input)?;
    let expected = mapping_artifact.summary;
    let binding = header.import_binding(*mapping_artifact.publication.chain());
    let mut content_artifact = super::content_readback::prepare_owned(
        source, root, target, *binding.plan_chain.as_bytes(), directory, input,
    )?;
    let mut inventory_artifact = inventory::stage_owned_with_artifacts(source, root, target, directory, input,
        (&mut journal, &|work| { bootstrap::reserve(source, work)?; bootstrap::verify_output(&output, source) }),
        inventory::PreparedInventoryInputs { content: &mut content_artifact, mapping: &mut mapping_artifact })?;
    let reserve = |work| {
        bootstrap::reserve(source, work)?;
        if matches!(work, MigrationBootstrapWork::Open | MigrationBootstrapWork::Write(0)) {
            bootstrap::verify_output(&output, source)?;
        }
        Ok::<(), String>(())
    };
    let mut target_owner = journal.into_source_stage(binding, input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    input.context.inventory_reobserve(input)?;
    target_owner.publish_source_map(&mut mapping_artifact.publication, &DaemonImportOutputPlatform,
        input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    target_owner.initialize_mapping(input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    mapping_artifact.visit(source, input, |row| target_owner.push(row,
        input.context.deadline(), reserve).map_err(|error| reason(&error)))?;
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    let mut reader = target_owner.begin_readback(expected.import_counts(),
        input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    mapping_artifact.visit(source, input, |row| reader.compare(&row,
        input.context.deadline(), reserve).map_err(|error| reason(&error)))?;
    let receipt = reader.finish(input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    let mut inventory_owner = receipt.into_inventory_stage(input.context.deadline(), reserve)
        .map_err(|error| reason(&error))?;
    transfer_rows(source, &mut inventory_artifact.publication, input, |row| {
        inventory_owner.push(row, input.context.deadline(), reserve).map_err(|error| reason(&error))
    })?;
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    let mut inventory_reader = inventory_owner.begin_readback(input.context.deadline(), reserve)
        .map_err(|error| reason(&error))?;
    transfer_rows(source, &mut inventory_artifact.publication, input, |row| {
        inventory_reader.compare(row, input.context.deadline(), reserve).map_err(|error| reason(&error))
    })?;
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    let receipt = inventory_reader.finish(input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    let mut content_owner = receipt.into_content_stage(
        source.retained_revisions().len() as u64, input.context.deadline(), reserve,
    ).map_err(|error| content_reason(&error, "ADMISSION"))?;
    input.context.inventory_reobserve(input)?;
    content_owner.publish_content(&mut content_artifact.publication, &DaemonImportOutputPlatform,
        input.context.deadline(), reserve).map_err(|error| content_reason(&error, "PUBLISH"))?;
    transfer_rows(source, &mut content_artifact.publication, input, |row| {
        content_owner.push(row, input.context.deadline(), reserve).map_err(|error| content_reason(&error, "IMPORT"))
    })?;
    input.context.inventory_reobserve(input)?;
    let mut content_reader = content_owner.begin_readback(input.context.deadline(), reserve)
        .map_err(|error| content_reason(&error, "READBACK"))?;
    transfer_rows(source, &mut content_artifact.publication, input, |row| {
        content_reader.compare(row, input.context.deadline(), reserve).map_err(|error| content_reason(&error, "READBACK"))
    })?;
    input.context.inventory_reobserve(input)?;
    let receipt = content_reader.finish(input.context.deadline(), reserve).map_err(|error| content_reason(&error, "READBACK"))?;
    input.context.inventory_reobserve(input)?;
    bootstrap::verify_output(&output, source).map_err(|_| unknown())?;
    source.charge(Work::Allocate(16 * 1024)).map_err(|_| unknown())?;
    source.charge(Work::Compare(16 * 1024)).map_err(|_| unknown())?;
    Ok(receipt.render_json())
}

fn transfer_rows(
    source: &MigrationSource<'_, '_, '_>,
    publication: &mut SourceImportRecordArtifactPublication<DaemonImportOutputPlatform>,
    input: &NativeMigrationInput<'_, '_, '_>,
    mut consume: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let context = inventory::ArtifactContext::new(source);
    let work = |unit| context.reserve(unit);
    let mut rows = publication.begin_rows_with_context(input.context.deadline(), &work)
        .map_err(|error| context.reason(error))?;
    while let Some(row) = rows.next_with_context(input.context.deadline(), &work)
        .map_err(|error| context.reason(error))? {
        consume(row)?;
    }
    Ok(())
}

fn prepare_source_map(source: &MigrationSource<'_, '_, '_>, target: SourceNamespaceId,
    directory: &Path, input: &NativeMigrationInput<'_, '_, '_>)
    -> Result<PreparedSourceMap, String> {
    // Borrow the admitted operation only; no renewed deadline or meter.
    let artifact_context = super::inventory::ArtifactContext::new(source);
    let work = |unit| artifact_context.reserve(unit);
    source.checkpoint()?;
    source.charge(Work::Allocate(256))?;
    let temporary = temporary_staging_name()?;
    let mut artifact = SourceImportRecordArtifact::create_with_context(directory, &temporary,
        DaemonImportOutputPlatform, input.context.deadline(), &work).map_err(|error| artifact_context.reason(error))?;
    let first = source.compile_source_mapping(target, input.context.deadline(), |row| {
        artifact.push_with_context(row, input.context.deadline(), &work).map_err(|error| artifact_context.reason(error))?;
        source.checkpoint()
    })?;
    let frozen = artifact.freeze_with_context(input.context.deadline(), &work).map_err(|error| artifact_context.reason(error))?;
    let chain = *frozen.chain();
    let mut compare = frozen.begin_readback_with_context(input.context.deadline(), &work).map_err(|error| artifact_context.reason(error))?;
    let second = source.compile_source_mapping(target, input.context.deadline(), |row| {
        compare.compare_with_context(row, input.context.deadline(), &work).map_err(|error| artifact_context.reason(error))?;
        source.checkpoint()
    })?;
    if first != second { return Err("DIRECT_MIGRATION_SOURCE_STAGE_READBACK_MISMATCH".to_owned()); }
    input.context.inventory_reobserve(input)?;
    let verified = compare.finish_with_context(input.context.deadline(), &work).map_err(|error| artifact_context.reason(error))?;
    source.charge(Work::Allocate(128))?;
    let name = format!("{}.source-map.v1", sha256::hex(&chain));
    let publication = verified.prepare_publication_with_context(&name, input.context.deadline(), &work).map_err(|error| artifact_context.reason(error))?;
    Ok(PreparedSourceMap { publication, header: source.source_mapping_header(target)?, summary: first })
}

fn content_reason(error: &MigrationBootstrapFailure<String>, phase: &str) -> String {
    let outcome = if matches!(error.control, ControlError::CommitOutcomeUnknown) {
        "OUTCOME_UNKNOWN"
    } else {
        "REFUSED"
    };
    let code = format!("DIRECT_MIGRATION_CONTENT_{phase}_{outcome}");
    // Context callbacks return public reason codes only. Preserve the first
    // refusal alongside the uncertain mutation status; never report success.
    error.first_context.as_ref().map_or_else(|| code.clone(), |cause| format!("{code}:{cause}"))
}

fn reason(error: &MigrationBootstrapFailure<String>) -> String {
    if error.first_context.is_some() {
        return "DIRECT_MIGRATION_SOURCE_STAGE_CONTEXT_OUTCOME_UNKNOWN".to_owned();
    }
    match error.control {
        ControlError::InvalidValue | ControlError::InvalidKey
        | ControlError::DuplicateMutationKey | ControlError::TransactionConflict => invalid(),
        _ => unknown(),
    }
}
fn invalid() -> String { "DIRECT_MIGRATION_SOURCE_STAGE_INVALID".to_owned() }
fn unknown() -> String { "DIRECT_MIGRATION_SOURCE_STAGE_OUTCOME_UNKNOWN".to_owned() }
