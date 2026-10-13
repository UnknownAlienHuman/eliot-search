//! Read-only migration views over exact canonical manifest bytes.

use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::time::Instant;

#[cfg(test)]
use super::codec::encode_manifest;
#[cfg(test)]
use super::load::load_manifest_input;
#[cfg(test)]
use super::model::DirectoryManifest;
#[cfg(test)]
use super::paths::migration_files;
#[cfg(test)]
use crate::sha256;

/// Exact raw-file SHA is distinct from the logical manifest digest.
/// Noncanonical legacy encodings are rejected rather than rewritten.
#[cfg(test)]
pub fn migration_manifest(
    path: &Path,
    remaining_bytes: usize,
) -> Result<(DirectoryManifest, [u8; 32], usize), String> {
    let (manifest, text) = load_manifest_input(path, remaining_bytes)?;
    if encode_manifest(&manifest)? != text {
        return Err(
            "DIRECT_MIGRATION_MANIFEST_ENCODING_UNSUPPORTED".to_owned(),
        );
    }
    let digest = sha256::digest(text.as_bytes());
    Ok((manifest, digest, text.len()))
}

/// Lists bounded canonical manifest candidates without creating missing state.
#[cfg(test)]
pub fn migration_manifest_files(
    data_root: &Path,
    maximum: usize,
    deadline: Instant,
) -> Result<(bool, Vec<PathBuf>), String> {
    migration_files(data_root, maximum, deadline)
}

/// Inventory's pure visitor validates every generation/entry from bytes read
/// under original native custody. It never lists paths or collects manifests.
pub(crate) fn inventory_manifest(
    path: &Path, bytes: &[u8], namespace: &[u8; 32],
    visit: impl FnMut([[u8; 32]; 3]) -> Result<(), String>,
) -> Result<(u64, u64, [u8; 32], [u8; 32]), String> {
    super::load::visit_manifest_bytes(path, bytes, namespace, visit)
}
