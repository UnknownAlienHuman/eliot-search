//! New finite metadata profile. Existing import/record artifact bytes are unchanged.
use super::{ControlError, MigrationBootstrapBinding, MigrationBootstrapIdentity,
    MigrationBootstrapIdentityProfile, MigrationBootstrapPlatform};
use crate::migration::{SourceImportRecordArtifactPublication, LegacyInventoryProfile};
use search_contracts::Blake3Digest32;

pub(super) const PROFILE: &[u8] = b"eliot.migration-bootstrap.v1\0released-native-read\0inventory-only\0no-candidate\0key=128;value=4096;records=8;live-bytes=32768;mutation-items=3;receipts=8;cache=8388608;metadata-work=8192;attempts=1;names=200;output-parents=64;utf16-path=32768\0";

pub(super) fn original(binding: &MigrationBootstrapBinding) -> Result<Vec<u8>, ControlError> {
    if binding.owner.epoch == 0 || binding.owner.generation == 0
        || binding.owner.native_limits.contains(&0)
        || binding.owner.invocation == [0; 16] || binding.owner.root == [0; 16]
        || binding.owner.installation == [0; 16] || binding.owner.incarnation == [0; 16]
    { return Err(ControlError::IdentityMismatch); }
    let mut bytes = Vec::with_capacity(768);
    bytes.extend_from_slice(PROFILE);
    for value in [&binding.owner.installation, &binding.owner.incarnation,
        &binding.owner.root, &binding.owner.invocation, binding.target.as_bytes()] {
        bytes.extend_from_slice(value);
    }
    for value in [&binding.owner.request_blake3, &binding.owner.record_blake3,
        &binding.owner.physical_blake3, binding.legacy_namespace.as_bytes(),
        binding.catalog_snapshot.as_bytes(), binding.output.path_blake3.as_bytes()] {
        bytes.extend_from_slice(value);
    }
    for value in [binding.owner.epoch, binding.owner.generation].into_iter()
        .chain(binding.owner.native_limits) { bytes.extend_from_slice(&value.to_le_bytes()); }
    match binding.registration {
        None => bytes.push(0),
        Some((digest, length)) => {
            if length == 0 || length > 64 * 1024 { return Err(ControlError::BudgetExceeded); }
            bytes.push(1); bytes.extend_from_slice(digest.as_bytes());
            bytes.extend_from_slice(&length.to_le_bytes());
        }
    }
    identity(&mut bytes, binding.output.directory)?;
    identity(&mut bytes, binding.output.target)?;
    Ok(bytes)
}

pub(super) fn attempt<P: MigrationBootstrapPlatform>(original: Blake3Digest32,
    publication: &SourceImportRecordArtifactPublication<P>, native: MigrationBootstrapIdentity,
    phase: u8,
) -> Result<Vec<u8>, ControlError> {
    if phase > 1 { return Err(ControlError::MigrationUnverified); }
    let stage = publication.staging_name().to_str().ok_or(ControlError::MigrationUnverified)?;
    let final_name = publication.final_name();
    local_name(stage)?; local_name(final_name)?;
    let profile = inventory_profile(publication)?;
    if stage == final_name
        || publication.rows() == 0 || publication.encoded_bytes() == 0
    { return Err(ControlError::MigrationUnverified); }
    let mut bytes = Vec::with_capacity(1024);
    bytes.extend_from_slice(b"eliot.migration-inventory-attempt.v1\0");
    bytes.extend_from_slice(profile.digest().as_bytes());
    bytes.extend_from_slice(original.as_bytes());
    bytes.push(phase);
    identity(&mut bytes, native)?;
    for name in [stage, final_name] {
        let length = u16::try_from(name.len()).map_err(|_| ControlError::BudgetExceeded)?;
        bytes.extend_from_slice(&length.to_le_bytes()); bytes.extend_from_slice(name.as_bytes());
    }
    bytes.extend_from_slice(publication.chain());
    bytes.extend_from_slice(&publication.rows().to_le_bytes());
    bytes.extend_from_slice(&publication.encoded_bytes().to_le_bytes());
    Ok(bytes)
}

pub(super) fn inventory_profile<P: MigrationBootstrapPlatform>(
    publication: &SourceImportRecordArtifactPublication<P>,
) -> Result<LegacyInventoryProfile, ControlError> {
    for profile in [LegacyInventoryProfile::V1, LegacyInventoryProfile::LocationV2] {
        if publication.final_name() == format!("{}.{}", hex(publication.chain()), profile.artifact_suffix()) {
            return Ok(profile);
        }
    }
    Err(ControlError::MigrationUnverified)
}

fn identity(bytes: &mut Vec<u8>, value: MigrationBootstrapIdentity) -> Result<(), ControlError> {
    if value.file == 0 || (value.profile == MigrationBootstrapIdentityProfile::WindowsFileIndex64
        && value.volume > u64::from(u32::MAX)) { return Err(ControlError::IdentityMismatch); }
    bytes.push(value.profile as u8);
    bytes.extend_from_slice(&value.volume.to_le_bytes());
    bytes.extend_from_slice(&value.file.to_le_bytes());
    Ok(())
}

pub(super) fn local_name(name: &str) -> Result<(), ControlError> {
    if name.is_empty() || name.len() > 200 || name == "." || name == ".."
        || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    { return Err(ControlError::MigrationUnverified); }
    Ok(())
}

pub(super) fn framed(domain: &[u8], bytes: &[u8]) -> Vec<u8> {
    // Native BLAKE3 remains composition-owned, as for existing journal identity.
    let mut framed = Vec::with_capacity(domain.len() + bytes.len() + 64);
    framed.extend_from_slice(b"eliot-search/migration-bootstrap/v1\0");
    framed.extend_from_slice(&(domain.len() as u64).to_le_bytes()); framed.extend_from_slice(domain);
    framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes()); framed.extend_from_slice(bytes);
    framed
}
pub(super) fn hex<const N: usize>(bytes: &[u8; N]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(N * 2);
    for byte in bytes { out.push(char::from(DIGITS[(byte >> 4) as usize])); out.push(char::from(DIGITS[(byte & 15) as usize])); }
    out
}
