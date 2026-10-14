//! One inactive target precedes the original inventory publication attempt.
use super::super::super::migration_context::{MigrationSource, NativeMigrationInput};
use super::{inventory, redb_import::DaemonImportOutputPlatform, sha256};
use search_contracts::{Blake3Digest32, Sha256Digest32, SourceNamespaceId};
use search_control_redb::{MigrationBootstrap, MigrationBootstrapBinding,
    MigrationBootstrapFailure, MigrationBootstrapIdentity, MigrationBootstrapIdentityProfile,
    MigrationBootstrapPlatform, MigrationBootstrapWork};
use search_revision_store::LegacyRevisionObjectWork as Work;
use std::path::Path;

#[path = "control_migration_bootstrap/output.rs"]
pub(super) mod output;

pub(super) struct BootstrapOutput(output::Output);
impl BootstrapOutput {
    fn verify(&self, source: &MigrationSource<'_, '_, '_>) -> Result<(), String> { self.0.verify(source) }
}

pub(super) fn stage(source: &MigrationSource<'_, '_, '_>, root: &Path,
    target: SourceNamespaceId, directory: &Path, input: &NativeMigrationInput<'_, '_, '_>)
    -> Result<String, String> {
    let (mut journal, output) = initialize(source, target, directory, input)?;
    let receipt = inventory::stage_owned(source, root, target, directory, input,
        &mut journal, |work| { reserve(source, work)?; output.verify(source) })?;
    // Any refusal after publication is an unknown bounded first-phase outcome.
    input.context.inventory_reobserve(input).map_err(|_| unknown())?;
    output.verify(source).map_err(|_| unknown())?;
    source.charge(Work::Allocate(4096)).map_err(|_| unknown())?;
    Ok(receipt.receipt.render_json())
}

pub(super) fn initialize(source: &MigrationSource<'_, '_, '_>, target: SourceNamespaceId,
    directory: &Path, input: &NativeMigrationInput<'_, '_, '_>)
    -> Result<(MigrationBootstrap, BootstrapOutput), String> {
    input.context.inventory_reobserve(input)?;
    let header = source.source_mapping_header(target)?;
    source.charge(Work::Allocate(4096))?;
    let mut output = output::Output::admit(source, directory)?;
    let name = MigrationBootstrap::target_name(&input.inventory_owner);
    let (file, binding) = output.create(source, &name)?;
    let journal = MigrationBootstrap::create(file, MigrationBootstrapBinding {
        owner: input.inventory_owner, target,
        legacy_namespace: Sha256Digest32::from_bytes(header.legacy_namespace),
        catalog_snapshot: Sha256Digest32::from_bytes(header.catalog_snapshot),
        registration: input.registration.map(|bytes|
            (Sha256Digest32::from_bytes(sha256::digest(bytes)), bytes.len() as u64)),
        output: binding,
    }, input.context.deadline(), native_blake3, |work| {
        reserve(source, work)?;
        output.verify(source)
    })
        .map_err(reason)?;
    output.sync_directory(source).map_err(|_| unknown())?;
    Ok((journal, BootstrapOutput(output)))
}

fn native_blake3(bytes: &[u8]) -> Blake3Digest32 {
    Blake3Digest32::from_bytes(*blake3::hash(bytes).as_bytes())
}
pub(super) fn verify_output(output: &BootstrapOutput, source: &MigrationSource<'_, '_, '_>) -> Result<(), String> {
    output.verify(source)
}
pub(super) fn reserve(source: &MigrationSource<'_, '_, '_>, work: MigrationBootstrapWork) -> Result<(), String> {
    match work {
        MigrationBootstrapWork::Checkpoint => source.checkpoint(),
        MigrationBootstrapWork::Open => source.charge(Work::Open),
        MigrationBootstrapWork::Allocate(bytes) => source.charge(Work::Allocate(bytes)),
        MigrationBootstrapWork::Read(bytes) => source.charge(Work::Read(bytes)),
        MigrationBootstrapWork::Process(bytes) => source.charge(Work::Compare(bytes)),
        MigrationBootstrapWork::Write(bytes) => source.charge(Work::Write(bytes)),
    }
}
pub(super) fn reason(failure: MigrationBootstrapFailure<String>) -> String {
    if failure.control == search_control_redb::ControlError::CommitOutcomeUnknown {
        unknown()
    } else { failure.first_context.unwrap_or_else(|| "DIRECT_MIGRATION_BOOTSTRAP_INVALID".to_owned()) }
}
fn unknown() -> String { "DIRECT_MIGRATION_BOOTSTRAP_OUTCOME_UNKNOWN".to_owned() }

impl MigrationBootstrapPlatform for DaemonImportOutputPlatform {
    fn bootstrap_identity(&self, original: &Self::Identity) -> MigrationBootstrapIdentity {
        MigrationBootstrapIdentity { profile: MigrationBootstrapIdentityProfile::WindowsFileIndex64,
            volume: original.0, file: original.1 }
    }
}
