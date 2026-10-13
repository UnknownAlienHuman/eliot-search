//! Complete streamed migration evidence, with no candidate/recovery authority.

use super::super::super::migration_context::{MigrationSource, NativeMigrationInput};
use super::{redb_import::DaemonImportOutputPlatform, sha256, temporary_staging_name};
use search_contracts::{Sha256Digest32, SourceNamespaceId};
use search_control_redb::migration::{
    LegacyInventoryBinding, LegacyInventoryEncoder, LegacyInventoryFamily as Family,
    LegacyInventoryLimits, LegacyInventoryStatus as Status, LegacyInventorySummary,
    MAX_SOURCE_IMPORT_ROW_BYTES, SourceImportRecordArtifact, SourceImportRecordArtifactError,
};
use search_revision_store::LegacyRevisionObjectWork as Work;
use std::path::Path;

#[path = "control_migration_inventory/io.rs"]
mod io;
#[path = "control_migration_inventory/scan.rs"]
mod scan;

pub(super) fn stage(
    source: &MigrationSource<'_, '_, '_>,
    root: &Path,
    target: SourceNamespaceId,
    directory: &Path,
    input: &NativeMigrationInput<'_, '_, '_>,
) -> Result<(), String> {
    input.context.inventory_reobserve(input)?;
    source.charge(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES))?;
    let temporary = temporary_staging_name()?;
    source.charge(Work::Open)?;
    let mut artifact = SourceImportRecordArtifact::create(
        directory,
        &temporary,
        DaemonImportOutputPlatform,
        input.context.deadline(),
    )
    .map_err(artifact_reason)?;
    let first = compile(source, root, target, input, &mut |row| {
        source.charge(Work::Write(row.len()))?;
        artifact
            .push(row, input.context.deadline())
            .map_err(artifact_reason)?;
        source.checkpoint()
    })?;
    input.context.inventory_reobserve(input)?;
    source.charge(Work::Observe)?;
    let frozen = artifact
        .freeze(input.context.deadline())
        .map_err(artifact_reason)?;
    let chain = *frozen.chain();
    let encoded_bytes = frozen.encoded_bytes();
    source.charge(Work::Open)?;
    let mut compare = frozen
        .begin_readback(input.context.deadline())
        .map_err(artifact_reason)?;
    let second = compile(source, root, target, input, &mut |row| {
        source.charge(Work::Compare(row.len()))?;
        compare
            .compare(row, input.context.deadline())
            .map_err(artifact_reason)?;
        source.checkpoint()
    })?;
    if first != second {
        return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned());
    }
    input.context.inventory_reobserve(input)?;
    source.charge(Work::Observe)?;
    let verified = compare
        .finish(input.context.deadline())
        .map_err(artifact_reason)?;
    source.charge(Work::Allocate(128))?;
    let name = format!("{}.legacy-inventory.v1", sha256::hex(&chain));
    source.charge(Work::Observe)?;
    let published = verified
        .publish(&name, input.context.deadline())
        .map_err(artifact_reason)?;
    // This is already after a possible effect: a late refusal remains unknown.
    source
        .checkpoint()
        .map_err(|_| "DIRECT_MIGRATION_INVENTORY_PUBLISH_OUTCOME_UNKNOWN".to_owned())?;
    if published.chain() != &chain || published.encoded_bytes() != encoded_bytes {
        return Err("DIRECT_MIGRATION_INVENTORY_PUBLISH_OUTCOME_UNKNOWN".to_owned());
    }
    input
        .context
        .inventory_reobserve(input)
        .map_err(|_| "DIRECT_MIGRATION_INVENTORY_PUBLISH_OUTCOME_UNKNOWN".to_owned())
}

fn artifact_reason(error: SourceImportRecordArtifactError<String>) -> String {
    match error {
        SourceImportRecordArtifactError::PublishOutcomeUnknown => {
            "DIRECT_MIGRATION_INVENTORY_PUBLISH_OUTCOME_UNKNOWN".to_owned()
        }
        SourceImportRecordArtifactError::RecordChain(_) => {
            "DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned()
        }
        SourceImportRecordArtifactError::Platform(reason) => {
            drop(reason);
            "DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()
        }
        _ => "DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned(),
    }
}

fn compile(
    source: &MigrationSource<'_, '_, '_>,
    root: &Path,
    target: SourceNamespaceId,
    input: &NativeMigrationInput<'_, '_, '_>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), String>,
) -> Result<LegacyInventorySummary, String> {
    let header = source.source_mapping_header(target)?;
    let registration = input.registration.map(|bytes| {
        (
            Sha256Digest32::from_bytes(sha256::digest(bytes)),
            bytes.len() as u64,
        )
    });
    source.charge(Work::Allocate(std::mem::size_of::<LegacyInventoryEncoder>()))?;
    let mut stream = Stream {
        source,
        sink,
        encoder: LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE)
            .map_err(codec_reason)?,
        statuses: [0; 7],
        complete: true,
    };
    stream.reserve_row()?;
    let row = stream
        .encoder
        .header(LegacyInventoryBinding {
            owner: input.inventory_owner,
            target,
            namespace: Sha256Digest32::from_bytes(header.legacy_namespace),
            snapshot: Sha256Digest32::from_bytes(header.catalog_snapshot),
            registration,
        })
        .map_err(codec_reason)?;
    stream.send(&row)?;
    for family in Family::ALL {
        stream.statuses = [0; 7];
        stream.complete = true;
        scan::family(&mut stream, family, root, target, input)?;
        let terminal = stream.terminal();
        stream.reserve_row()?;
        let row = stream
            .encoder
            .end_family(family, terminal, stream.complete)
            .map_err(codec_reason)?;
        stream.send(&row)?;
    }
    stream.reserve_row()?;
    let (row, summary) = stream.encoder.finish().map_err(codec_reason)?;
    sink(&row)?;
    source.checkpoint()?;
    Ok(summary)
}

fn codec_reason(error: search_control_redb::migration::LegacyInventoryError) -> String {
    use search_control_redb::migration::LegacyInventoryError;
    match error {
        LegacyInventoryError::Limit => "DIRECT_MIGRATION_INVENTORY_LIMIT",
        _ => "DIRECT_MIGRATION_INVENTORY_CHANGED",
    }
    .to_owned()
}

struct Stream<'a, 'view, 'operation, 'owner> {
    source: &'a MigrationSource<'view, 'operation, 'owner>,
    sink: &'a mut dyn FnMut(&[u8]) -> Result<(), String>,
    encoder: LegacyInventoryEncoder,
    statuses: [u64; 7],
    complete: bool,
}

impl Stream<'_, '_, '_, '_> {
    fn reserve_row(&self) -> Result<(), String> {
        self.source
            .charge(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES))?;
        self.source
            .charge(Work::Compare(MAX_SOURCE_IMPORT_ROW_BYTES))
    }
    fn send(&mut self, row: &[u8]) -> Result<(), String> {
        (self.sink)(row)?;
        self.source.checkpoint()
    }
    fn item(
        &mut self,
        family: Family,
        status: Status,
        kind: u16,
        refs: [[u8; 32]; 3],
        counts: [u64; 3],
    ) -> Result<(), String> {
        self.reserve_row()?;
        self.statuses[status as usize] += 1;
        if matches!(
            status,
            Status::Unreadable | Status::Unsupported | Status::Gap
        ) {
            self.complete = false;
        }
        let row = self
            .encoder
            .item(
                family,
                status,
                kind,
                refs.map(Sha256Digest32::from_bytes),
                counts,
            )
            .map_err(codec_reason)?;
        self.send(&row)
    }
    fn terminal(&self) -> Status {
        [
            Status::Gap,
            Status::Unreadable,
            Status::Corrupt,
            Status::Unsupported,
            Status::Orphan,
            Status::Present,
            Status::AbsentProven,
        ]
        .into_iter()
        .find(|status| self.statuses[*status as usize] != 0)
        .unwrap_or(Status::AbsentProven)
    }
}
