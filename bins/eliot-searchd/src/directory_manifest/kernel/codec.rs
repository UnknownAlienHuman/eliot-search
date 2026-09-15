//! Canonical directory-manifest codec and digest binding.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::model::{DirectoryEntry, DirectoryManifest};
use super::spec::{
    MANIFEST_HEADER, MAX_MANIFEST_BYTES, MAX_MANIFEST_ENTRIES,
    MAX_MANIFEST_LINE_BYTES,
};
use crate::sha256;

pub(super) fn build_manifest(
    namespace_id: String,
    directory_digest: String,
    generation: u64,
    entries: BTreeMap<String, DirectoryEntry>,
) -> Result<DirectoryManifest, String> {
    validate_digest(&namespace_id, "DIRECT_MANIFEST_NAMESPACE_INVALID")?;
    validate_digest(
        &directory_digest,
        "DIRECT_MANIFEST_DIRECTORY_INVALID",
    )?;
    if generation == 0 || entries.len() > MAX_MANIFEST_ENTRIES {
        return Err("DIRECT_MANIFEST_GENERATION_INVALID".to_owned());
    }
    for (source_id, entry) in &entries {
        if source_id != &entry.source_id {
            return Err("DIRECT_MANIFEST_SOURCE_MISMATCH".to_owned());
        }
        validate_entry(entry)?;
    }
    let body = encode_body(&entries)?;
    let manifest_digest = sha256::hex(&sha256::digest_parts(
        b"eliot-search/direct-directory-manifest/v1",
        &[
            namespace_id.as_bytes(),
            directory_digest.as_bytes(),
            &generation.to_be_bytes(),
            body.as_bytes(),
        ],
    ));
    Ok(DirectoryManifest {
        namespace_id,
        directory_digest,
        generation,
        entries,
        manifest_digest,
    })
}

pub(super) fn encode_manifest(
    manifest: &DirectoryManifest,
) -> Result<String, String> {
    let body = encode_body(&manifest.entries)?;
    let encoded = format!(
        "{MANIFEST_HEADER}\t{}\t{}\t{}\t{}\n{body}",
        manifest.namespace_id,
        manifest.directory_digest,
        manifest.generation,
        manifest.manifest_digest,
    );
    if encoded.len() > MAX_MANIFEST_BYTES {
        return Err("DIRECT_MANIFEST_TOO_LARGE".to_owned());
    }
    Ok(encoded)
}

fn encode_body(
    entries: &BTreeMap<String, DirectoryEntry>,
) -> Result<String, String> {
    let mut body = String::new();
    for entry in entries.values() {
        validate_entry(entry)?;
        let line = format!(
            "V1\t{}\t{}\t{}\n",
            entry.source_id, entry.path_digest, entry.revision_id,
        );
        if line.len() > MAX_MANIFEST_LINE_BYTES {
            return Err("DIRECT_MANIFEST_LINE_TOO_LARGE".to_owned());
        }
        body.push_str(&line);
        if body.len() > MAX_MANIFEST_BYTES {
            return Err("DIRECT_MANIFEST_TOO_LARGE".to_owned());
        }
    }
    Ok(body)
}

pub(super) fn manifest_path(
    root: &Path,
    manifest: &DirectoryManifest,
) -> PathBuf {
    root.join(format!(
        "{}.{}.{}.manifest",
        manifest.directory_digest,
        manifest.generation,
        manifest.manifest_digest,
    ))
}

pub(super) fn validate_filename(
    path: &Path,
    manifest: &DirectoryManifest,
) -> Result<(), String> {
    let expected = manifest_path(
        path.parent()
            .ok_or_else(|| "DIRECT_MANIFEST_PARENT_MISSING".to_owned())?,
        manifest,
    );
    if expected.file_name() == path.file_name() {
        Ok(())
    } else {
        Err("DIRECT_MANIFEST_FILENAME_MISMATCH".to_owned())
    }
}

pub(super) fn validate_entry(entry: &DirectoryEntry) -> Result<(), String> {
    validate_digest(
        &entry.source_id,
        "DIRECT_MANIFEST_SOURCE_ID_INVALID",
    )?;
    validate_digest(
        &entry.path_digest,
        "DIRECT_MANIFEST_PATH_DIGEST_INVALID",
    )?;
    validate_digest(
        &entry.revision_id,
        "DIRECT_MANIFEST_REVISION_ID_INVALID",
    )
}

pub(super) fn validate_digest(
    value: &str,
    error: &'static str,
) -> Result<(), String> {
    if sha256::decode_digest(value).is_some() {
        Ok(())
    } else {
        Err(error.to_owned())
    }
}
