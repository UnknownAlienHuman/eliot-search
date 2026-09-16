//! Exact bounded read-only preparation lookup.

use std::path::Path;

use zeroize::Zeroizing;

use super::codec::{
    binding, decode_reference, lookup_key, object_file_name, object_id,
    object_shard, read_object, verify_manifest,
};
use super::paths::directories;
use super::spec::REF_BYTES;
use super::super::super::storage_io::{
    ensure_directory, read_regular_file,
};
use super::super::super::{RevisionMetadata, RevisionProtector};

/// Returns only the canonical preparation body bound to the requested source
/// revision, profile and active storage-protection profile. Query lookup never
/// creates or repairs durable state.
pub(crate) fn load(
    root: &Path,
    protector: &RevisionProtector,
    namespace: &str,
    metadata: &RevisionMetadata,
) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    let binding = binding(namespace, metadata)
        .map_err(|_| "DIRECT_PREPARATION_BINDING_INVALID")?;
    let key = lookup_key(&binding, protector);
    let (reference_path, objects) = directories(root, &key, false)
        .map_err(|_| "DIRECT_PREPARATION_UNAVAILABLE")?;
    let saved = read_regular_file(
        &reference_path,
        REF_BYTES,
        "DIRECT_PREPARATION_REFERENCE_READ_FAILED",
    )
    .map_err(|_| "DIRECT_PREPARATION_UNAVAILABLE")?;
    let (digest, length) = decode_reference(&saved, &key, protector)?;
    let id = object_id(&binding, protector, &digest);
    let object_shard_path = objects.join(
        object_shard(&id)
            .map_err(|_| "DIRECT_PREPARATION_OBJECT_INVALID")?,
    );
    ensure_directory(&object_shard_path)
        .map_err(|_| "DIRECT_PREPARATION_OBJECT_INVALID")?;
    let path = object_shard_path.join(
        object_file_name(&id, protector)
            .map_err(|_| "DIRECT_PREPARATION_OBJECT_INVALID")?,
    );
    let manifest = read_object(&path, protector, &id, digest, length)
        .map_err(|_| "DIRECT_PREPARATION_OBJECT_INVALID")?;
    let verified = verify_manifest(&manifest, &binding)?;
    Ok(Zeroizing::new(verified.body().to_vec()))
}
