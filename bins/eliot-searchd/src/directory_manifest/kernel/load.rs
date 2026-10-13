//! Exact manifest readback, parsing and generation selection.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::codec::{
    build_manifest, validate_digest, validate_entry, validate_filename,
};
use super::model::{
    DirectoryEntry, DirectoryManifest, DirectoryManifestVerification,
};
use super::paths::{
    ensure_regular_file, existing_manifest_root, is_reparse, manifest_files,
};
use super::spec::{
    MANIFEST_HEADER, MAX_MANIFEST_BYTES, MAX_MANIFEST_ENTRIES,
    MAX_MANIFEST_LINE_BYTES,
};

#[cfg(test)]
pub(super) fn load_latest_manifest(
    root: &Path,
    namespace_id: &str,
    directory_digest: &str,
) -> Result<Option<DirectoryManifest>, String> {
    validate_digest(namespace_id, "DIRECT_MANIFEST_NAMESPACE_INVALID")?;
    validate_digest(
        directory_digest,
        "DIRECT_MANIFEST_DIRECTORY_INVALID",
    )?;
    let mut latest: Option<DirectoryManifest> = None;
    let mut seen_generation = BTreeMap::<u64, String>::new();
    for path in manifest_files(root)? {
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            return Err("DIRECT_MANIFEST_FILENAME_INVALID".to_owned());
        };
        if !file_name.starts_with(directory_digest) {
            continue;
        }
        let manifest = load_manifest_file(&path)?;
        if manifest.namespace_id != namespace_id
            || manifest.directory_digest != directory_digest
        {
            return Err("DIRECT_MANIFEST_BINDING_MISMATCH".to_owned());
        }
        if let Some(existing) = seen_generation.insert(
            manifest.generation,
            manifest.manifest_digest.clone(),
        ) && existing != manifest.manifest_digest
        {
            return Err("DIRECT_MANIFEST_GENERATION_AMBIGUOUS".to_owned());
        }
        match &latest {
            Some(existing) if existing.generation > manifest.generation => {}
            Some(existing)
                if existing.generation == manifest.generation
                    && existing.manifest_digest != manifest.manifest_digest =>
            {
                return Err("DIRECT_MANIFEST_GENERATION_AMBIGUOUS".to_owned());
            }
            _ => latest = Some(manifest),
        }
    }
    Ok(latest)
}

pub(super) fn load_manifest_file(
    path: &Path,
) -> Result<DirectoryManifest, String> {
    load_manifest_input(path, MAX_MANIFEST_BYTES).map(|(manifest, _)| manifest)
}

/// Shared parser and exact-file observation used by verification and migration.
pub(super) fn load_manifest_input(
    path: &Path,
    max_bytes: usize,
) -> Result<(DirectoryManifest, String), String> {
    ensure_regular_file(path)?;
    let mut file = File::open(path)
        .map_err(|_| "DIRECT_MANIFEST_READ_ERROR".to_owned())?;
    let before = file
        .metadata()
        .map_err(|_| "DIRECT_MANIFEST_METADATA_ERROR".to_owned())?;
    if !before.is_file() || is_reparse(&before) {
        return Err("DIRECT_MANIFEST_FILE_INVALID".to_owned());
    }
    let max_bytes = max_bytes.min(MAX_MANIFEST_BYTES);
    if before.len() > max_bytes as u64 {
        return Err("DIRECT_MANIFEST_TOO_LARGE".to_owned());
    }
    let mut text = String::new();
    (&mut file)
        .take(max_bytes as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|_| "DIRECT_MANIFEST_READ_ERROR".to_owned())?;
    let after = file
        .metadata()
        .map_err(|_| "DIRECT_MANIFEST_METADATA_ERROR".to_owned())?;
    if text.len() > max_bytes || !text.ends_with('\n') {
        return Err("DIRECT_MANIFEST_TRUNCATED".to_owned());
    }
    if text.len() as u64 != before.len()
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err("DIRECT_MANIFEST_CHANGED_DURING_READ".to_owned());
    }

    let mut lines = text.split_terminator('\n');
    let header = lines
        .next()
        .ok_or_else(|| "DIRECT_MANIFEST_HEADER_MISSING".to_owned())?;
    let header_fields = header.splitn(6, '\t').collect::<Vec<_>>();
    if header_fields.len() != 5 || header_fields[0] != MANIFEST_HEADER {
        return Err("DIRECT_MANIFEST_HEADER_INVALID".to_owned());
    }
    let namespace_id = header_fields[1].to_owned();
    let directory_digest = header_fields[2].to_owned();
    let generation = header_fields[3]
        .parse::<u64>()
        .map_err(|_| "DIRECT_MANIFEST_GENERATION_INVALID".to_owned())?;
    let expected_digest = header_fields[4].to_owned();
    validate_digest(&namespace_id, "DIRECT_MANIFEST_NAMESPACE_INVALID")?;
    validate_digest(
        &directory_digest,
        "DIRECT_MANIFEST_DIRECTORY_INVALID",
    )?;
    validate_digest(
        &expected_digest,
        "DIRECT_MANIFEST_DIGEST_INVALID",
    )?;
    if generation == 0 {
        return Err("DIRECT_MANIFEST_GENERATION_INVALID".to_owned());
    }

    let mut entries = BTreeMap::new();
    for line in lines {
        if line.is_empty() || line.len() > MAX_MANIFEST_LINE_BYTES {
            return Err("DIRECT_MANIFEST_LINE_INVALID".to_owned());
        }
        let fields = line.splitn(5, '\t').collect::<Vec<_>>();
        if fields.len() != 4 || fields[0] != "V1" {
            return Err("DIRECT_MANIFEST_LINE_INVALID".to_owned());
        }
        let entry = DirectoryEntry {
            source_id: fields[1].to_owned(),
            path_digest: fields[2].to_owned(),
            revision_id: fields[3].to_owned(),
        };
        validate_entry(&entry)?;
        if entries.insert(entry.source_id.clone(), entry).is_some() {
            return Err("DIRECT_MANIFEST_SOURCE_DUPLICATE".to_owned());
        }
        if entries.len() > MAX_MANIFEST_ENTRIES {
            return Err("DIRECT_MANIFEST_ENTRY_LIMIT_EXCEEDED".to_owned());
        }
    }
    let manifest = build_manifest(
        namespace_id,
        directory_digest,
        generation,
        entries,
    )?;
    if manifest.manifest_digest != expected_digest {
        return Err("DIRECT_MANIFEST_DIGEST_MISMATCH".to_owned());
    }
    validate_filename(path, &manifest)?;
    Ok((manifest, text))
}

/// Canonical migration visitor: fixed header plus one borrowed entry at a time.
pub(super) fn visit_manifest_bytes(
    path: &Path, bytes: &[u8], namespace: &[u8; 32],
    mut visit: impl FnMut([[u8; 32]; 3]) -> Result<(), String>,
) -> Result<(u64, u64, [u8; 32], [u8; 32]), String> {
    let invalid = || "DIRECT_MANIFEST_INVENTORY_INVALID".to_owned();
    if bytes.len() > MAX_MANIFEST_BYTES || bytes.last() != Some(&b'\n') { return Err(invalid()); }
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    let (header, body) = text.split_once('\n').ok_or_else(invalid)?;
    let mut fields = header.split('\t');
    if fields.next() != Some(MANIFEST_HEADER) { return Err(invalid()); }
    let namespace_text = fields.next().ok_or_else(invalid)?;
    let directory = fields.next().ok_or_else(invalid)?;
    let generation_text = fields.next().ok_or_else(invalid)?;
    let expected = fields.next().ok_or_else(invalid)?;
    if fields.next().is_some() { return Err(invalid()); }
    for value in [namespace_text, directory, expected] { validate_digest(value, "DIRECT_MANIFEST_INVENTORY_INVALID")?; }
    let generation = generation_text.parse::<u64>().map_err(|_| invalid())?;
    if generation == 0 || generation.to_string() != generation_text
        || crate::sha256::decode_digest(namespace_text).as_ref() != Some(namespace) { return Err(invalid()); }
    let mut previous: Option<&str> = None;
    let mut count = 0_u64;
    for line in body.split_terminator('\n') {
        if line.is_empty() || line.len() > MAX_MANIFEST_LINE_BYTES { return Err(invalid()); }
        let mut fields = line.split('\t');
        if fields.next() != Some("V1") { return Err(invalid()); }
        let mut references = [[0; 32]; 3];
        let source = fields.next().ok_or_else(invalid)?;
        let path_digest = fields.next().ok_or_else(invalid)?;
        let revision = fields.next().ok_or_else(invalid)?;
        if fields.next().is_some() || previous.is_some_and(|value| value >= source) { return Err(invalid()); }
        for (slot, value) in references.iter_mut().zip([source, path_digest, revision]) {
            validate_digest(value, "DIRECT_MANIFEST_INVENTORY_INVALID")?;
            *slot = crate::sha256::decode_digest(value).ok_or_else(invalid)?;
        }
        count += 1;
        if count > MAX_MANIFEST_ENTRIES as u64 { return Err(invalid()); }
        visit(references)?;
        previous = Some(source);
    }
    let digest = crate::sha256::digest_parts(b"eliot-search/direct-directory-manifest/v1",
        &[namespace_text.as_bytes(), directory.as_bytes(), &generation.to_be_bytes(), body.as_bytes()]);
    if crate::sha256::hex(&digest) != expected { return Err(invalid()); }
    let manifest = DirectoryManifest { namespace_id: namespace_text.to_owned(),
        directory_digest: directory.to_owned(), generation, entries: BTreeMap::new(),
        manifest_digest: expected.to_owned() };
    validate_filename(path, &manifest)?;
    Ok((generation, count, crate::sha256::decode_digest(directory).ok_or_else(invalid)?, digest))
}

/// Verifies every immutable directory manifest and selects one unambiguous
/// highest generation per directory.
pub fn verify_directory_manifests(
    data_root: &Path,
    namespace_id: &str,
) -> Result<DirectoryManifestVerification, String> {
    validate_digest(namespace_id, "DIRECT_MANIFEST_NAMESPACE_INVALID")?;
    let canonical_root = std::fs::canonicalize(data_root)
        .map_err(|error| format!("DIRECT_MANIFEST_ROOT_ERROR:{error}"))?;
    let files = existing_manifest_root(&canonical_root)?
        .map(|root| manifest_files(&root))
        .transpose()?
        .unwrap_or_default();
    let mut current = BTreeMap::<String, DirectoryManifest>::new();
    let mut generations = BTreeMap::<(String, u64), String>::new();
    let mut highest_generation = 0_u64;

    for path in &files {
        let manifest = load_manifest_file(path)?;
        if manifest.namespace_id != namespace_id {
            return Err("DIRECT_MANIFEST_NAMESPACE_MISMATCH".to_owned());
        }
        let key = (manifest.directory_digest.clone(), manifest.generation);
        if let Some(existing) =
            generations.insert(key, manifest.manifest_digest.clone())
            && existing != manifest.manifest_digest
        {
            return Err("DIRECT_MANIFEST_GENERATION_AMBIGUOUS".to_owned());
        }
        highest_generation = highest_generation.max(manifest.generation);
        match current.get(&manifest.directory_digest) {
            Some(existing) if existing.generation > manifest.generation => {}
            Some(existing)
                if existing.generation == manifest.generation
                    && existing.manifest_digest != manifest.manifest_digest =>
            {
                return Err("DIRECT_MANIFEST_GENERATION_AMBIGUOUS".to_owned());
            }
            _ => {
                current.insert(manifest.directory_digest.clone(), manifest);
            }
        }
    }

    Ok(DirectoryManifestVerification {
        manifest_files: files.len(),
        directories: current.len(),
        current_entries: current
            .values()
            .map(|manifest| manifest.entries.len())
            .sum(),
        highest_generation,
    })
}
