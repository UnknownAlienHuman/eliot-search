//! Explicit inert source-stage profile, preserving the old bootstrap/import bytes.
use crate::migration::{SourceImportBinding, SourceImportRecordArtifactPublication};
use super::super::{CompletedInventory, MigrationBootstrapIdentity, MigrationBootstrapPlatform};
use crate::ControlError;

pub(super) const PROFILE: &[u8] = b"eliot.owner-source-stage.v1\0control3+source4\0same-live-original-database\0source-map-only\0no-serving-candidate-restart-cleanup\0batch=256;row=371;rows=2000000;bytes=536870912;owner-marker=4096\0";

pub(super) fn marker(binding: SourceImportBinding, proof: &CompletedInventory) -> Vec<u8> {
    let mut out = Vec::with_capacity(1024);
    out.extend_from_slice(PROFILE);
    out.extend_from_slice(proof.original.as_bytes());
    out.extend_from_slice(&proof.chain);
    out.extend_from_slice(&proof.rows.to_le_bytes());
    out.extend_from_slice(&proof.bytes.to_le_bytes());
    out.extend_from_slice(binding.target_namespace.as_bytes());
    for digest in [binding.legacy_namespace, binding.catalog_snapshot,
        binding.mapping_profile, binding.plan_chain] { out.extend_from_slice(digest.as_bytes()); }
    out.extend_from_slice(&binding.events.to_le_bytes());
    out.extend_from_slice(&binding.sources.to_le_bytes());
    out
}

pub(super) fn attempt<P: MigrationBootstrapPlatform>(profile: &[u8],
    binding: SourceImportBinding, publication: &SourceImportRecordArtifactPublication<P>,
    identity: MigrationBootstrapIdentity, phase: u8,
) -> Result<Vec<u8>, ControlError> {
    let name = publication.final_name();
    let stage = publication.staging_name().to_str().ok_or(ControlError::MigrationUnverified)?;
    super::super::codec::local_name(name)?;
    super::super::codec::local_name(stage)?;
    if phase > 1 || publication.chain() != binding.plan_chain.as_bytes()
        || name != format!("{}.source-map.v1", super::super::codec::hex(publication.chain()))
        || publication.rows() == 0 || publication.encoded_bytes() == 0
        || identity.file == 0
        || (identity.profile == crate::MigrationBootstrapIdentityProfile::WindowsFileIndex64
            && identity.volume > u64::from(u32::MAX))
    { return Err(ControlError::MigrationUnverified); }
    let mut out = Vec::with_capacity(2048);
    out.extend_from_slice(b"eliot.owner-source-map-attempt.v1\0");
    out.extend_from_slice(profile);
    out.push(phase);
    out.push(identity.profile as u8);
    out.extend_from_slice(&identity.volume.to_le_bytes());
    out.extend_from_slice(&identity.file.to_le_bytes());
    for value in [stage, name] {
        let length = u16::try_from(value.len()).map_err(|_| ControlError::BudgetExceeded)?;
        out.extend_from_slice(&length.to_le_bytes()); out.extend_from_slice(value.as_bytes());
    }
    out.extend_from_slice(publication.chain());
    out.extend_from_slice(&publication.rows().to_le_bytes());
    out.extend_from_slice(&publication.encoded_bytes().to_le_bytes());
    Ok(out)
}
