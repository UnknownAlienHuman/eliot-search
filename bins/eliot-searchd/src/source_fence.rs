//! Shared source-state fingerprint for DIRECT handles and continuation windows.
//!
//! Preserve the exact length-delimited preimage previously duplicated in both
//! callers. The unused alternate fold is not a compatible replacement for it.
//! This is a session invalidation check, not live access/currentness authority.

use crate::direct_store::DirectStore;
use crate::sha256;

pub fn digest(store: &DirectStore) -> String {
    let namespace = store.namespace_id();
    let sources = store.list_sources();
    let mut encoded = Vec::new();
    append(&mut encoded, namespace.as_bytes());
    for source in sources {
        append(&mut encoded, source.source_id.as_bytes());
        append(&mut encoded, source.revision_id.as_bytes());
        append(&mut encoded, source.content_digest.as_bytes());
        append(&mut encoded, source.path_digest.as_bytes());
        encoded.extend_from_slice(&source.byte_length.to_be_bytes());
        encoded.push(u8::from(source.active));
        encoded.extend_from_slice(&source.sequence.to_be_bytes());
    }
    sha256::hex(&sha256::digest_parts(
        b"eliot-search/direct-source-fence/v1",
        &[&encoded],
    ))
}

fn append(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(
        &u64::try_from(value.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    output.extend_from_slice(value);
}
