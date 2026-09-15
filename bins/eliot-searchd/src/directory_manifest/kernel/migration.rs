//! Read-only migration views over exact canonical manifest bytes.

use std::path::{Path, PathBuf};
use std::time::Instant;

use super::codec::encode_manifest;
use super::load::load_manifest_input;
use super::model::DirectoryManifest;
use super::paths::migration_files;
use crate::sha256;

/// Exact raw-file SHA is distinct from the logical manifest digest.
/// Noncanonical legacy encodings are rejected rather than rewritten.
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
pub fn migration_manifest_files(
    data_root: &Path,
    maximum: usize,
    deadline: Instant,
) -> Result<(bool, Vec<PathBuf>), String> {
    migration_files(data_root, maximum, deadline)
}
