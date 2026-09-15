//! Native physical-root/executable observation and acquisition token minting.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use search_contracts::DataRootId;
use search_runtime_owner::OwnerError;

use super::codec::{
    canonical_path_bytes, domain_digest, is_reparse, native_volume_material,
};
use super::spec::{MAX_EXECUTABLE_BYTES, READ_CHUNK_BYTES};

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(0);

/// Fresh native observation of one already-locked canonical root.
pub(super) struct ObservedRoot {
    pub(super) canonical_path_digest: [u8; 32],
    pub(super) volume_identity_digest: [u8; 32],
    pub(super) data_root_id: DataRootId,
}

/// Observes the physical root behind an already-held exclusion.
pub(super) fn observe_physical_root(canonical_root: &Path) -> Result<ObservedRoot, OwnerError> {
    let fresh = fs::canonicalize(canonical_root).map_err(|_| OwnerError::DataRootInvalid)?;
    if fresh != canonical_root {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    let metadata = fs::symlink_metadata(&fresh).map_err(|_| OwnerError::DataRootInvalid)?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err(OwnerError::DataRootInvalid);
    }
    let path_bytes = canonical_path_bytes(&fresh);
    let volume_material = native_volume_material(&fresh)?;
    let canonical_path_digest =
        domain_digest(b"eliot-search/owner-canonical-path/v1\0", &[&path_bytes]);
    let volume_identity_digest = domain_digest(
        b"eliot-search/owner-volume-identity/v1\0",
        &[&volume_material],
    );
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-search/owner-data-root-id/v1\0");
    hasher.update(&volume_material);
    hasher.update(&path_bytes);
    let digest = hasher.finalize();
    let id: [u8; 16] = digest.as_bytes()[..16]
        .try_into()
        .map_err(|_| OwnerError::DataRootInvalid)?;
    Ok(ObservedRoot {
        canonical_path_digest,
        volume_identity_digest,
        data_root_id: DataRootId::from_bytes(id),
    })
}

/// Hashes the running executable image with a finite budget.
pub(super) fn observe_executable() -> Result<[u8; 32], OwnerError> {
    let path = std::env::current_exe().map_err(|_| OwnerError::DataRootInvalid)?;
    let canonical = fs::canonicalize(&path).map_err(|_| OwnerError::DataRootInvalid)?;
    let metadata = fs::metadata(&canonical).map_err(|_| OwnerError::DataRootInvalid)?;
    if !metadata.is_file() {
        return Err(OwnerError::DataRootInvalid);
    }
    let mut file = fs::File::open(&canonical).map_err(|_| OwnerError::DataRootInvalid)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-search/owner-executable/v1\0");
    let mut chunk = vec![0_u8; READ_CHUNK_BYTES];
    let mut total: u64 = 0;
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|_| OwnerError::DataRootInvalid)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| OwnerError::DataRootInvalid)?)
            .ok_or(OwnerError::DataRootInvalid)?;
        if total > MAX_EXECUTABLE_BYTES {
            return Err(OwnerError::DataRootInvalid);
        }
        hasher.update(&chunk[..read]);
    }
    let digest = hasher.finalize();
    let mut output = [0_u8; 32];
    output.copy_from_slice(digest.as_bytes());
    Ok(output)
}

/// Mints the two stable installation identifiers using the original shared
/// process-local counter and exact digest transcript.
pub(super) fn mint_installation_ids(
    observed: &ObservedRoot,
    executable: &[u8; 32],
) -> Result<([u8; 16], [u8; 16]), OwnerError> {
    let stamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| OwnerError::DataRootInvalid)?
        .as_nanos();
    let counter = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-search/owner-installation/v1\0");
    hasher.update(&stamp_nanos.to_be_bytes());
    hasher.update(&std::process::id().to_be_bytes());
    hasher.update(&counter.to_be_bytes());
    hasher.update(executable);
    hasher.update(&observed.canonical_path_digest);
    hasher.update(&observed.volume_identity_digest);
    let digest = hasher.finalize();
    let bytes = digest.as_bytes();
    let installation_id: [u8; 16] = bytes[..16]
        .try_into()
        .map_err(|_| OwnerError::DataRootInvalid)?;
    let installation_incarnation_id: [u8; 16] = bytes[16..]
        .try_into()
        .map_err(|_| OwnerError::DataRootInvalid)?;
    Ok((installation_id, installation_incarnation_id))
}

/// Mints a reuse-resistant process-creation token for one acquisition.
pub(super) fn mint_owner_token(
    observed: &ObservedRoot,
    executable: &[u8; 32],
) -> Result<[u8; 16], OwnerError> {
    let stamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| OwnerError::DataRootInvalid)?
        .as_nanos();
    let counter = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-search/owner-token/v1\0");
    hasher.update(&stamp_nanos.to_be_bytes());
    hasher.update(&std::process::id().to_be_bytes());
    hasher.update(&counter.to_be_bytes());
    hasher.update(executable);
    hasher.update(&observed.canonical_path_digest);
    hasher.update(&observed.volume_identity_digest);
    let digest = hasher.finalize();
    digest.as_bytes()[..16]
        .try_into()
        .map_err(|_| OwnerError::DataRootInvalid)
}
