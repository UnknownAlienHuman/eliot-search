//! Immutable preparation object and lookup-reference publication.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use search_materializer::api::encode_legacy_preparation_manifest;
use zeroize::Zeroizing;

use super::codec::{
    binding, lookup_key, object_file_name, object_id, object_shard, read_object,
    reference,
};
use super::paths::directories;
use super::spec::REF_BYTES;
use super::super::super::storage_io::{
    ensure_child_directory, persist_immutable_object, read_regular_file,
    sync_directory,
};
use super::super::super::{
    IndexedSource, RevisionMetadata, RevisionProtector, verify_plaintext,
};
use crate::direct_preparation::{
    CANONICAL_MATERIALIZER_REVISION, CANONICAL_UNITIZER_REVISION,
    CanonicalPreparationReceipt, canonical_materializer_digest,
    canonical_unitizer_digest,
};
use crate::sha256;

pub(crate) fn persist_source(
    root: &Path,
    protector: &RevisionProtector,
    namespace: &str,
    source: &IndexedSource,
    bytes: &[u8],
) -> Result<(), String> {
    super::super::super::revision_writer::persist_before_publication(
        root, protector, source, bytes,
    )?;
    let metadata = RevisionMetadata {
        source_id: source.source_id.clone(),
        revision_id: source.revision_id.clone(),
        content_digest: source.content_digest.clone(),
        byte_length: source.byte_length,
    };
    persist(root, protector, namespace, &metadata, bytes).map(|_| ())
}

/// Object publication and exact readback precede its reference, which in turn
/// precedes the source-catalog event. Conflicting immutable bytes are never
/// replaced.
pub(crate) fn persist(
    root: &Path,
    protector: &RevisionProtector,
    namespace: &str,
    metadata: &RevisionMetadata,
    source: &[u8],
) -> Result<Option<&'static str>, String> {
    persist_canonical(root, protector, namespace, metadata, source)
        .map(|receipt| receipt.gap)
}

/// Canonical publication returning representation and profile provenance.
pub(crate) fn persist_canonical(
    root: &Path,
    protector: &RevisionProtector,
    namespace: &str,
    metadata: &RevisionMetadata,
    source: &[u8],
) -> Result<CanonicalPreparationReceipt, String> {
    use crate::direct_preparation::{
        encode_canonical_preparation, preparation_gap as gap_of,
    };

    verify_plaintext(metadata, source)?;
    let binding = binding(namespace, metadata)?;
    let namespace_bytes = sha256::decode_digest(namespace)
        .ok_or_else(|| "DIRECT_PREPARATION_BINDING_INVALID".to_owned())?;
    let source_bytes = sha256::decode_digest(&metadata.source_id)
        .ok_or_else(|| "DIRECT_PREPARATION_BINDING_INVALID".to_owned())?;
    let revision_bytes = sha256::decode_digest(&metadata.revision_id)
        .ok_or_else(|| "DIRECT_PREPARATION_BINDING_INVALID".to_owned())?;
    let content_bytes = sha256::decode_digest(&metadata.content_digest)
        .ok_or_else(|| "DIRECT_PREPARATION_BINDING_INVALID".to_owned())?;
    let materializer_digest =
        canonical_materializer_digest().map_err(str::to_owned)?;
    let unitizer_digest =
        canonical_unitizer_digest().map_err(str::to_owned)?;
    let (representation, body) = encode_canonical_preparation(
        source,
        &namespace_bytes,
        &source_bytes,
        &revision_bytes,
        &content_bytes,
        metadata.byte_length,
    )
    .map_err(str::to_owned)?;
    let gap = gap_of(&body).map_err(str::to_owned)?;

    let manifest = Zeroizing::new(
        encode_legacy_preparation_manifest(
            &binding,
            &representation,
            CANONICAL_MATERIALIZER_REVISION,
            CANONICAL_UNITIZER_REVISION,
            &body,
        )
        .map_err(|error| error.code().to_owned())?,
    );

    let digest = sha256::digest(&manifest);
    let key = lookup_key(&binding, protector);
    let (reference_path, objects) = directories(root, &key, true)?;
    let object_id = object_id(&binding, protector, &digest);
    let object_shard_path = objects.join(object_shard(&object_id)?);
    ensure_child_directory(&object_shard_path)?;
    #[cfg(unix)]
    sync_directory(&objects)?;
    #[cfg(not(unix))]
    sync_directory(&objects);
    let object_path = object_shard_path.join(object_file_name(
        &object_id,
        protector,
    )?);
    let expected_ref = reference(
        &key,
        digest,
        manifest.len() as u64,
        protector,
    )?;

    match fs::symlink_metadata(&reference_path) {
        Ok(_) => {
            let observed = read_regular_file(
                &reference_path,
                REF_BYTES,
                "DIRECT_PREPARATION_REFERENCE_READ_FAILED",
            )?;
            if observed != expected_ref {
                return Err(
                    "DIRECT_PREPARATION_REFERENCE_CONFLICT".to_owned(),
                );
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => {
            return Err(
                "DIRECT_PREPARATION_REFERENCE_READ_FAILED".to_owned(),
            );
        }
    }

    match fs::symlink_metadata(&object_path) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let encoded = Zeroizing::new(if protector.encrypts_new_objects() {
                protector.protect(
                    &object_id,
                    &sha256::hex(&digest),
                    &manifest,
                )?
            } else {
                manifest.to_vec()
            });
            persist_immutable_object(&object_path, &encoded)?;
        }
        Err(_) => {
            return Err("DIRECT_PREPARATION_OBJECT_READ_FAILED".to_owned());
        }
    }

    let observed = read_object(
        &object_path,
        protector,
        &object_id,
        digest,
        manifest.len() as u64,
    )?;
    if observed.as_slice() != manifest.as_slice() {
        return Err("DIRECT_PREPARATION_OBJECT_CONFLICT".to_owned());
    }
    persist_immutable_object(&reference_path, &expected_ref)?;

    Ok(CanonicalPreparationReceipt {
        representation_id: representation,
        materializer_digest,
        unitizer_digest,
        gap,
    })
}
