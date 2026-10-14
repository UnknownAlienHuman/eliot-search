//! Complete streamed migration evidence, with no candidate/recovery authority.

use super::super::super::migration_context::{MigrationSource, NativeMigrationInput};
use super::{redb_import::DaemonImportOutputPlatform, sha256, temporary_staging_name};
use search_contracts::{Sha256Digest32, SourceNamespaceId};
use search_control_redb::migration::{
    LegacyInventoryBinding, LegacyInventoryEncoder, LegacyInventoryFamily as Family,
    LegacyInventoryLimits, LegacyInventoryProfile, LegacyInventoryStatus as Status, LegacyInventorySummary,
    MAX_SOURCE_IMPORT_ROW_BYTES, SourceImportRecordArtifact, SourceImportRecordArtifactError,
    SourceImportRecordArtifactPublication,
};
use search_revision_store::LegacyRevisionObjectWork as Work;
use std::path::Path;
use std::cell::RefCell;
use search_control_redb::{ControlError, MigrationBootstrapWork};

// This adapter borrows the admitted operation; it owns neither a deadline nor
// a renewed work meter. Keep the first refusal available to the native caller.
pub(super) struct ArtifactContext<'a, 'view, 'operation, 'owner> {
    source: &'a MigrationSource<'view, 'operation, 'owner>,
    first: RefCell<Option<String>>,
}
impl<'a, 'view, 'operation, 'owner> ArtifactContext<'a, 'view, 'operation, 'owner> {
    pub(super) const fn new(source: &'a MigrationSource<'view, 'operation, 'owner>) -> Self {
        Self { source, first: RefCell::new(None) }
    }
    pub(super) fn reserve(&self, work: MigrationBootstrapWork) -> Result<(), ControlError> {
        if self.first.borrow().is_some() { return Err(ControlError::ReadCancelled); }
        super::bootstrap::reserve(self.source, work).map_err(|cause| {
            self.first.borrow_mut().get_or_insert(cause);
            ControlError::ReadCancelled
        })
    }
    pub(super) fn reason(&self, error: SourceImportRecordArtifactError<String>) -> String {
        if matches!(error, SourceImportRecordArtifactError::PublishOutcomeUnknown) {
            return artifact_reason(error);
        }
        self.first.borrow().clone().unwrap_or_else(|| artifact_reason(error))
    }
}

#[path = "control_migration_inventory/io.rs"]
mod io;
#[path = "control_migration_inventory/scan.rs"]
mod scan;
#[path = "control_migration_inventory/preparation_links.rs"]
mod preparation_links;

type BootstrapDispatch<'a> = (&'a mut search_control_redb::MigrationBootstrap,
    &'a dyn Fn(search_control_redb::MigrationBootstrapWork) -> Result<(), String>);

pub(super) struct StagedInventory {
    pub(super) receipt: search_control_redb::MigrationBootstrapReceipt,
    pub(super) publication: SourceImportRecordArtifactPublication<DaemonImportOutputPlatform>,
}

pub(super) fn stage(
    source: &MigrationSource<'_, '_, '_>,
    root: &Path,
    target: SourceNamespaceId,
    directory: &Path,
    input: &NativeMigrationInput<'_, '_, '_>,
) -> Result<(), String> {
    stage_inner(source, root, target, directory, input, None).map(|_| ())
}

pub(super) fn stage_owned<F: Fn(search_control_redb::MigrationBootstrapWork) -> Result<(), String>>(
    source: &MigrationSource<'_, '_, '_>, root: &Path, target: SourceNamespaceId,
    directory: &Path, input: &NativeMigrationInput<'_, '_, '_>,
    journal: &mut search_control_redb::MigrationBootstrap, checkpoint: F,
) -> Result<StagedInventory, String> {
    stage_inner(source, root, target, directory, input, Some((journal, &checkpoint)))?
        .ok_or_else(|| "DIRECT_MIGRATION_BOOTSTRAP_INVALID".to_owned())
}

pub(super) struct JointArtifactInputs<'mapping, 'bootstrap> {
    pub(super) source_plan: [u8; 32],
    pub(super) bootstrap: BootstrapDispatch<'bootstrap>,
    pub(super) mapping: &'mapping mut super::source_stage::PreparedSourceMap,
}

/// Produce both existing artifacts from the same two independent source passes.
/// Neither original is published until both streams have completed comparison.
pub(super) fn stage_owned_joint(
    source: &MigrationSource<'_, '_, '_>, root: &Path, target: SourceNamespaceId,
    directory: &Path, input: &NativeMigrationInput<'_, '_, '_>,
    inputs: JointArtifactInputs<'_, '_>,
) -> Result<(StagedInventory, super::content_readback::PreparedContent), String> {
    let JointArtifactInputs { source_plan, bootstrap, mapping } = inputs;
    input.context.inventory_reobserve(input)?;
    let inventory_context = ArtifactContext::new(source);
    let inventory_work = |unit| inventory_context.reserve(unit);
    let content_context = ArtifactContext::new(source);
    let content_work = |unit| content_context.reserve(unit);
    source.charge(Work::Allocate(256))?;
    let inventory_temporary = temporary_staging_name()?;
    let mut inventory_writer = SourceImportRecordArtifact::create_with_context(
        directory, &inventory_temporary, DaemonImportOutputPlatform, input.context.deadline(), &inventory_work,
    ).map_err(|error| inventory_context.reason(error))?;
    source.charge(Work::Allocate(256))?;
    let content_temporary = temporary_staging_name()?;
    let mut content_writer = SourceImportRecordArtifact::create_with_context(
        directory, &content_temporary, DaemonImportOutputPlatform, input.context.deadline(), &content_work,
    ).map_err(|error| super::content_readback::owned_artifact_reason(&content_context, error))?;
    let (first_inventory, first_content, content_header) = {
        let mut content_sink = |row: &[u8]| {
            content_writer.push_with_context(row, input.context.deadline(), &content_work)
                .map_err(|error| super::content_readback::owned_artifact_reason(&content_context, error))?;
            source.checkpoint()
        };
        let mut content = super::content_readback::ContentPass::new(
            source, target, source_plan, input.context.deadline(), &mut content_sink,
        )?;
        let header = content.header();
        let inventory = compile_inventory(source, root, target, input, &mut |row| {
            inventory_writer.push_with_context(row, input.context.deadline(), &inventory_work)
                .map_err(|error| inventory_context.reason(error))?;
            source.checkpoint()
        }, Some(&mut *mapping), Some(&mut content))?;
        (inventory, content.finish(source, input.context.deadline())?, header)
    };
    input.context.inventory_reobserve(input)?;
    let inventory_frozen = inventory_writer.freeze_with_context(input.context.deadline(), &inventory_work)
        .map_err(|error| inventory_context.reason(error))?;
    let content_frozen = content_writer.freeze_with_context(input.context.deadline(), &content_work)
        .map_err(|error| super::content_readback::owned_artifact_reason(&content_context, error))?;
    let inventory_chain = *inventory_frozen.chain();
    let content_chain = *content_frozen.chain();
    let mut inventory_compare = inventory_frozen.begin_readback_with_context(input.context.deadline(), &inventory_work)
        .map_err(|error| inventory_context.reason(error))?;
    let mut content_compare = content_frozen.begin_readback_with_context(input.context.deadline(), &content_work)
        .map_err(|error| super::content_readback::owned_artifact_reason(&content_context, error))?;
    let (second_inventory, second_content) = {
        let mut content_sink = |row: &[u8]| {
            content_compare.compare_with_context(row, input.context.deadline(), &content_work)
                .map_err(|error| super::content_readback::owned_artifact_reason(&content_context, error))?;
            source.checkpoint()
        };
        let mut content = super::content_readback::ContentPass::new(
            source, target, source_plan, input.context.deadline(), &mut content_sink,
        )?;
        let inventory = compile_inventory(source, root, target, input, &mut |row| {
            inventory_compare.compare_with_context(row, input.context.deadline(), &inventory_work)
                .map_err(|error| inventory_context.reason(error))?;
            source.checkpoint()
        }, Some(&mut *mapping), Some(&mut content))?;
        (inventory, content.finish(source, input.context.deadline())?)
    };
    if first_inventory != second_inventory || first_content != second_content {
        return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned());
    }
    input.context.inventory_reobserve(input)?;
    let inventory_verified = inventory_compare.finish_with_context(input.context.deadline(), &inventory_work)
        .map_err(|error| inventory_context.reason(error))?;
    let content_verified = content_compare.finish_with_context(input.context.deadline(), &content_work)
        .map_err(|error| super::content_readback::owned_artifact_reason(&content_context, error))?;
    source.charge(Work::Allocate(256))?;
    let content_name = format!("{}.source-content.v1", sha256::hex(&content_chain));
    let content_publication = content_verified.prepare_publication_with_context(
        &content_name, input.context.deadline(), &content_work,
    ).map_err(|error| super::content_readback::owned_artifact_reason(&content_context, error))?;
    let content = super::content_readback::PreparedContent::from_verified(
        content_publication, content_header, first_content,
    )?;
    let inventory_name = format!("{}.{}", sha256::hex(&inventory_chain), LegacyInventoryProfile::LocationV2.artifact_suffix());
    let mut inventory_publication = inventory_verified.prepare_publication_with_context(
        &inventory_name, input.context.deadline(), &inventory_work,
    ).map_err(|error| inventory_context.reason(error))?;
    input.context.inventory_reobserve(input)?;
    let (journal, checkpoint) = bootstrap;
    let receipt = journal.publish_inventory(&mut inventory_publication, &DaemonImportOutputPlatform,
        input.context.deadline(), checkpoint).map_err(super::bootstrap::reason)?;
    input.context.inventory_reobserve(input)
        .map_err(|_| "DIRECT_MIGRATION_BOOTSTRAP_OUTCOME_UNKNOWN".to_owned())?;
    Ok((StagedInventory { receipt, publication: inventory_publication }, content))
}

fn stage_inner(
    source: &MigrationSource<'_, '_, '_>, root: &Path, target: SourceNamespaceId,
    directory: &Path, input: &NativeMigrationInput<'_, '_, '_>,
    bootstrap: Option<BootstrapDispatch<'_>>,
) -> Result<Option<StagedInventory>, String> {
    input.context.inventory_reobserve(input)?;
    let artifact_context = ArtifactContext::new(source);
    let work = |unit| artifact_context.reserve(unit);
    source.charge(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES))?;
    let temporary = temporary_staging_name()?;
    let mut artifact = SourceImportRecordArtifact::create_with_context(
        directory,
        &temporary,
        DaemonImportOutputPlatform,
        input.context.deadline(),
        &work,
    )
    .map_err(|error| artifact_context.reason(error))?;
    let first = compile_inventory(source, root, target, input, &mut |row| {
        artifact
            .push_with_context(row, input.context.deadline(), &work)
            .map_err(|error| artifact_context.reason(error))?;
        source.checkpoint()
    }, None, None)?;
    input.context.inventory_reobserve(input)?;
    let frozen = artifact
        .freeze_with_context(input.context.deadline(), &work)
        .map_err(|error| artifact_context.reason(error))?;
    let chain = *frozen.chain();
    let encoded_bytes = frozen.encoded_bytes();
    let mut compare = frozen
        .begin_readback_with_context(input.context.deadline(), &work)
        .map_err(|error| artifact_context.reason(error))?;
    let second = compile_inventory(source, root, target, input, &mut |row| {
        compare
            .compare_with_context(row, input.context.deadline(), &work)
            .map_err(|error| artifact_context.reason(error))?;
        source.checkpoint()
    }, None, None)?;
    if first != second {
        return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned());
    }
    input.context.inventory_reobserve(input)?;
    let verified = compare
        .finish_with_context(input.context.deadline(), &work)
        .map_err(|error| artifact_context.reason(error))?;
    source.charge(Work::Allocate(128))?;
    let name = format!("{}.{}", sha256::hex(&chain), LegacyInventoryProfile::LocationV2.artifact_suffix());
    if let Some((journal, checkpoint)) = bootstrap {
        let mut original = verified.prepare_publication_with_context(&name, input.context.deadline(), &work)
            .map_err(|error| artifact_context.reason(error))?;
        input.context.inventory_reobserve(input)?;
        let receipt = journal.publish_inventory(&mut original, &DaemonImportOutputPlatform,
            input.context.deadline(), checkpoint).map_err(super::bootstrap::reason)?;
        input.context.inventory_reobserve(input)
            .map_err(|_| "DIRECT_MIGRATION_BOOTSTRAP_OUTCOME_UNKNOWN".to_owned())?;
        return Ok(Some(StagedInventory { receipt, publication: original }));
    }
    let published = verified
        .publish_with_context(&name, input.context.deadline(), &work)
        .map_err(|error| artifact_context.reason(error))?;
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
        .map(|()| None)
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

fn compile_inventory(
    source: &MigrationSource<'_, '_, '_>,
    root: &Path,
    target: SourceNamespaceId,
    input: &NativeMigrationInput<'_, '_, '_>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    mut mapping: Option<&mut super::source_stage::PreparedSourceMap>,
    mut content: Option<&mut super::content_readback::ContentPass<'_>>,
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
        encoder: LegacyInventoryEncoder::with_profile(LegacyInventoryLimits::BASELINE, LegacyInventoryProfile::LocationV2)
            .map_err(codec_reason)?,
        statuses: [0; 7],
        complete: true,
        preparation_links: None,
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
        if let Some(content) = content.as_deref_mut() && family == Family::Revisions {
            scan::revisions_with_content(&mut stream, root, input, content)?;
        } else if let Some(mapping) = mapping.as_deref_mut() && family == Family::Sources {
            scan::sources_from_mapping(&mut stream, target, input, mapping)?;
        } else {
            scan::family(&mut stream, family, root, target, input)?;
        }
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
    preparation_links: Option<std::sync::Arc<preparation_links::View>>,
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
