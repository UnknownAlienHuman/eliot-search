//! Protected revision bytes must exist and decrypt exactly before catalog publication.

use std::fs;
use std::io;
use std::path::Path;
use zeroize::Zeroizing;

use super::storage_io::{persist_immutable_object, protected_path, read_regular_file};
use super::{
    IndexedSource, MAX_REVISION_OBJECT_BYTES, RevisionMetadata, RevisionProtector,
    verify_plaintext,
};

pub(super) fn persist_before_publication(
    root: &Path,
    protector: &RevisionProtector,
    source: &IndexedSource,
    plaintext: &[u8],
) -> Result<(), String> {
    let metadata = RevisionMetadata {
        source_id: source.source_id.clone(),
        revision_id: source.revision_id.clone(),
        content_digest: source.content_digest.clone(),
        byte_length: source.byte_length,
    };
    persist_verified(root, protector, &metadata, plaintext)
}

pub(super) fn persist_verified(
    root: &Path,
    protector: &RevisionProtector,
    metadata: &RevisionMetadata,
    plaintext: &[u8],
) -> Result<(), String> {
    if !protector.encrypts_new_objects() {
        return Err("DIRECT_PROTECTED_WRITER_REQUIRES_ENCRYPTION".to_owned());
    }
    verify_plaintext(metadata, plaintext)?;
    let path = protected_path(root, &metadata.revision_id)?;
    match fs::symlink_metadata(&path) {
        // Existing objects must pass the same bounded regular-file readback.
        // Never overwrite or re-encrypt a conflicting object under the same ID.
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let protected = protector.protect(
                &metadata.revision_id,
                &metadata.content_digest,
                plaintext,
            )?;
            persist_immutable_object(&path, &protected)?;
        }
        Err(_) => return Err("DIRECT_REVISION_PROTECTED_METADATA_ERROR".to_owned()),
    }
    let object = read_regular_file(
        &path,
        MAX_REVISION_OBJECT_BYTES,
        "DIRECT_REVISION_PROTECTED_READ_ERROR",
    )?;
    let readback = Zeroizing::new(protector.unprotect(
        &object,
        &metadata.revision_id,
        &metadata.content_digest,
        metadata.byte_length,
    )?);
    verify_plaintext(metadata, &readback)?;
    if readback.as_slice() != plaintext {
        return Err("DIRECT_REVISION_PROTECTED_READBACK_MISMATCH".to_owned());
    }
    Ok(())
}
