//! Original source-map publication and in-place row staging, not full candidate authority.
use super::super::super::migration_context::{MigrationSource, NativeMigrationInput};
use super::{bootstrap, inventory, redb_import::DaemonImportOutputPlatform,
    temporary_staging_name, sha256};
use search_contracts::SourceNamespaceId;
use search_control_redb::{ControlError, MigrationBootstrapFailure, MigrationBootstrapWork};
use search_control_redb::migration::{SourceImportRecordArtifact,
    SourceImportRecordArtifactPublication, SourceImportRecordChain, SourceMappingSummary};
use search_revision_store::LegacyRevisionObjectWork as Work;
use std::path::Path;

pub(super) fn stage(source: &MigrationSource<'_, '_, '_>, root: &Path,
    target: SourceNamespaceId, directory: &Path, input: &NativeMigrationInput<'_, '_, '_>)
    -> Result<String, String> {
    let (mut journal, output) = bootstrap::initialize(source, target, directory, input)?;
    inventory::stage_owned(source, root, target, directory, input, &mut journal,
        |work| { bootstrap::reserve(source, work)?; bootstrap::verify_output(&output, source) })?;
    let header = source.source_mapping_header(target)?;
    let (mut publication, expected) = prepare_source_map(source, target, directory, input)?;
    let binding = header.import_binding(*publication.chain());
    let reserve = |work| {
        bootstrap::reserve(source, work)?;
        if matches!(work, MigrationBootstrapWork::Open | MigrationBootstrapWork::Write(0)) {
            bootstrap::verify_output(&output, source)?;
        }
        Ok::<(), String>(())
    };
    let mut target_owner = journal.into_source_stage(binding, input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    input.context.inventory_reobserve(input)?;
    target_owner.publish_source_map(&mut publication, &DaemonImportOutputPlatform,
        input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    target_owner.initialize_mapping(input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    let mut chain = SourceImportRecordChain::new();
    let imported = source.compile_source_mapping_with_rows(target, input.context.deadline(), |row| {
        source.charge(Work::Compare(row.len()))?;
        chain.push(row).map_err(|_| invalid())?;
        source.checkpoint()
    }, |row| target_owner.push(row, input.context.deadline(), reserve).map_err(|error| reason(&error)))?;
    if imported != expected || chain.finish() != *binding.plan_chain.as_bytes() {
        return Err(unknown());
    }
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    let mut reader = target_owner.begin_readback(expected.import_counts(),
        input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    let mut replay_chain = SourceImportRecordChain::new();
    let replayed = source.compile_source_mapping_with_rows(target, input.context.deadline(), |row| {
        source.charge(Work::Compare(row.len()))?;
        replay_chain.push(row).map_err(|_| invalid())?;
        source.checkpoint()
    }, |row| reader.compare(&row, input.context.deadline(), reserve).map_err(|error| reason(&error)))?;
    if replayed != expected || replay_chain.finish() != *binding.plan_chain.as_bytes() {
        return Err(unknown());
    }
    let receipt = reader.finish(input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    let mut inventory_owner = receipt.into_inventory_stage(input.context.deadline(), reserve)
        .map_err(|error| reason(&error))?;
    let first = inventory::compile(source, root, target, input, &mut |row| {
        inventory_owner.push(row, input.context.deadline(), reserve).map_err(|error| reason(&error))
    })?;
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    let mut inventory_reader = inventory_owner.begin_readback(input.context.deadline(), reserve)
        .map_err(|error| reason(&error))?;
    let second = inventory::compile(source, root, target, input, &mut |row| {
        inventory_reader.compare(row, input.context.deadline(), reserve).map_err(|error| reason(&error))
    })?;
    if first != second { return Err(unknown()); }
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    let receipt = inventory_reader.finish(input.context.deadline(), reserve).map_err(|error| reason(&error))?;
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    bootstrap::verify_output(&output, source).map_err(|_| unknown())?;
    source.charge(Work::Allocate(16 * 1024)).map_err(|_| unknown())?;
    source.charge(Work::Compare(16 * 1024)).map_err(|_| unknown())?;
    Ok(receipt.render_json())
}

fn prepare_source_map(source: &MigrationSource<'_, '_, '_>, target: SourceNamespaceId,
    directory: &Path, input: &NativeMigrationInput<'_, '_, '_>)
    -> Result<(SourceImportRecordArtifactPublication<DaemonImportOutputPlatform>, SourceMappingSummary), String> {
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
    Ok((publication, first))
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
