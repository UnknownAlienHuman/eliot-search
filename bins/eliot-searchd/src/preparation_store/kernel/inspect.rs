//! Read-only migration inspection of one preparation reference/object pair.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Instant;

use search_materializer::api::{
    LEGACY_PREPARATION_DIRECTORY, LEGACY_PREPARATION_OBJECTS_DIRECTORY,
    LEGACY_PREPARATION_REFERENCES_DIRECTORY,
    legacy_preparation_reference_file_name, legacy_preparation_shard,
};
use zeroize::Zeroizing;

use super::codec::{
    binding, decode_object, decode_reference, extension, lookup_key,
    object_file_name, object_id, object_shard, verify_manifest,
};
use super::spec::{MAX_OBJECT_BYTES, REF_BYTES};
use super::super::super::storage_io::{
    ensure_directory, read_regular_file,
};
use super::super::super::{
    RevisionMetadata, RevisionProtector, verify_plaintext,
};
use crate::sha256;

/// One reference/object observation. Payload bytes never escape into migration
/// output.
pub(crate) struct PreparationEvidence {
    pub(crate) present: bool,
    pub(crate) stored_bytes: u64,
    pub(crate) json: String,
}

/// Recomputes the exact profile layout only during explicit migration
/// inspection. Absence is a typed missing derivative; malformed or
/// contradictory state is an error and is never repaired here.
pub(crate) fn inspect(
    root: &Path,
    protector: &RevisionProtector,
    namespace: &str,
    metadata: &RevisionMetadata,
    source: &[u8],
    deadline: Instant,
) -> Result<PreparationEvidence, String> {
    let check = || {
        if Instant::now() >= deadline {
            Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())
        } else {
            Ok(())
        }
    };
    check()?;
    verify_plaintext(metadata, source)?;
    let binding = binding(namespace, metadata)?;
    let key = lookup_key(&binding, protector);
    let hex = sha256::hex(&key);
    let missing = || PreparationEvidence {
        present: false,
        stored_bytes: 0,
        json: format!(
            concat!(
                "{{\"status\":\"missing_reference\",",
                "\"reference_key_sha256\":\"{}\",",
                "\"record_verified\":false,",
                "\"layout_available\":false}}"
            ),
            hex
        ),
    };

    ensure_directory(root)?;
    let base = root.join(LEGACY_PREPARATION_DIRECTORY);
    let refs = base.join(LEGACY_PREPARATION_REFERENCES_DIRECTORY);
    let shard = refs.join(legacy_preparation_shard(&key));
    for path in [&base, &refs, &shard] {
        check()?;
        match fs::symlink_metadata(path) {
            Ok(_) => ensure_directory(path)?,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(missing());
            }
            Err(_) => {
                return Err(
                    "DIRECT_PREPARATION_REFERENCE_READ_FAILED".to_owned(),
                );
            }
        }
    }

    let ref_path = shard.join(legacy_preparation_reference_file_name(&key));
    match fs::symlink_metadata(&ref_path) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(missing());
        }
        Err(_) => {
            return Err(
                "DIRECT_PREPARATION_REFERENCE_READ_FAILED".to_owned(),
            );
        }
    }
    let saved = read_regular_file(
        &ref_path,
        REF_BYTES,
        "DIRECT_PREPARATION_REFERENCE_READ_FAILED",
    )?;
    let (digest, length) =
        decode_reference(&saved, &key, protector).map_err(str::to_owned)?;
    let id = object_id(&binding, protector, &digest);
    let objects = base.join(LEGACY_PREPARATION_OBJECTS_DIRECTORY);
    ensure_directory(&objects)?;
    let object_shard_path = objects.join(object_shard(&id)?);
    ensure_directory(&object_shard_path)?;
    let object_path =
        object_shard_path.join(object_file_name(&id, protector)?);

    check()?;
    let encoded = Zeroizing::new(read_regular_file(
        &object_path,
        MAX_OBJECT_BYTES,
        "DIRECT_PREPARATION_OBJECT_READ_FAILED",
    )?);
    let raw_digest = sha256::digest(&encoded);
    let raw_length = encoded.len() as u64;
    let manifest = decode_object(&encoded, protector, &id, digest, length)?;
    drop(encoded);
    let (
        body,
        stored_representation,
        materializer_digest,
        unitizer_digest,
        materializer_revision,
        unitizer_revision,
    ) = {
        let verified =
            verify_manifest(&manifest, &binding).map_err(str::to_owned)?;
        (
            verified.body().to_vec(),
            *verified.representation_id(),
            verified.binding().materializer_digest,
            verified.binding().unitizer_digest,
            verified.materializer_revision(),
            verified.unitizer_revision(),
        )
    };

    check()?;
    let expected = {
        use crate::direct_preparation::encode_canonical_preparation;

        let namespace_bytes = sha256::decode_digest(namespace)
            .ok_or_else(|| {
                "DIRECT_PREPARATION_BINDING_INVALID".to_owned()
            })?;
        let source_bytes = sha256::decode_digest(&metadata.source_id)
            .ok_or_else(|| {
                "DIRECT_PREPARATION_BINDING_INVALID".to_owned()
            })?;
        let revision_bytes = sha256::decode_digest(&metadata.revision_id)
            .ok_or_else(|| {
                "DIRECT_PREPARATION_BINDING_INVALID".to_owned()
            })?;
        let content_bytes = sha256::decode_digest(&metadata.content_digest)
            .ok_or_else(|| {
                "DIRECT_PREPARATION_BINDING_INVALID".to_owned()
            })?;
        let (_, recomputed) = encode_canonical_preparation(
            source,
            &namespace_bytes,
            &source_bytes,
            &revision_bytes,
            &content_bytes,
            metadata.byte_length,
        )
        .map_err(str::to_owned)?;
        Zeroizing::new(recomputed)
    };
    if body.as_slice() != expected.as_slice() {
        return Err("DIRECT_PREPARATION_DERIVATION_MISMATCH".to_owned());
    }
    let gap = crate::direct_preparation::preparation_gap(&expected)
        .map_err(str::to_owned)?;
    drop(expected);

    let representation_hex = sha256::hex(&stored_representation);
    let materializer_hex = sha256::hex(&materializer_digest);
    let unitizer_hex = sha256::hex(&unitizer_digest);
    drop(manifest);

    check()?;
    let reread = Zeroizing::new(read_regular_file(
        &object_path,
        MAX_OBJECT_BYTES,
        "DIRECT_PREPARATION_OBJECT_READ_FAILED",
    )?);
    if reread.len() as u64 != raw_length
        || sha256::digest(&reread) != raw_digest
        || read_regular_file(
            &ref_path,
            REF_BYTES,
            "DIRECT_PREPARATION_REFERENCE_READ_FAILED",
        )? != saved
    {
        return Err("DIRECT_MIGRATION_PREPARATION_CHANGED".to_owned());
    }
    check()?;

    Ok(PreparationEvidence {
        present: true,
        stored_bytes: raw_length + REF_BYTES as u64,
        json: format!(
            concat!(
                "{{\"status\":\"verified_record\",",
                "\"reference_key_sha256\":\"{}\",",
                "\"reference_file_sha256\":\"{}\",",
                "\"reference_bytes\":{},",
                "\"object_id\":\"{}\",",
                "\"object_format\":\"{}\",",
                "\"manifest_sha256\":\"{}\",",
                "\"manifest_bytes\":{},",
                "\"encoded_sha256\":\"{}\",",
                "\"encoded_bytes\":{},",
                "\"record_verified\":true,",
                "\"layout_available\":{},",
                "\"preparation_gap\":{},",
                "\"representation_id\":\"{}\",",
                "\"materializer_profile_digest\":\"{}\",",
                "\"unitizer_profile_digest\":\"{}\",",
                "\"materializer_profile_revision\":{},",
                "\"unitizer_profile_revision\":{},",
                "\"content_digest_algorithm\":\"sha256\",",
                "\"representation_digest_algorithm\":",
                "\"blake3_256\",",
                "\"manifest_digest_algorithm\":\"sha256\"}}"
            ),
            hex,
            sha256::hex(&sha256::digest(&saved)),
            REF_BYTES,
            id,
            extension(protector),
            sha256::hex(&digest),
            length,
            sha256::hex(&raw_digest),
            raw_length,
            gap.is_none(),
            gap.map_or_else(
                || "null".to_owned(),
                crate::service_output::json_string,
            ),
            representation_hex,
            materializer_hex,
            unitizer_hex,
            materializer_revision,
            unitizer_revision,
        ),
    })
}
