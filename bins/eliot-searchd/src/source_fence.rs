//! Deterministic source-state fence with constant extra memory.
//!
//! The fold commits namespace, source identity, current revision/content/path,
//! length, active state, and source-event sequence without assembling one large
//! concatenation buffer. Order comes from the DIRECT store's canonical source
//! ordering.

use crate::direct_store::DirectStore;
use crate::sha256;

/// Returns a deterministic digest of the complete current source state.
pub(crate) fn digest(store: &DirectStore) -> String {
    let namespace = store.namespace_id();
    let mut state = sha256::digest_parts(
        b"eliot-search/direct-source-fence/init/v1",
        &[namespace.as_bytes()],
    );
    for source in store.list_sources() {
        let record = sha256::digest_parts(
            b"eliot-search/direct-source-fence/record/v1",
            &[
                source.source_id.as_bytes(),
                source.revision_id.as_bytes(),
                source.content_digest.as_bytes(),
                source.path_digest.as_bytes(),
                &source.byte_length.to_be_bytes(),
                &[u8::from(source.active)],
                &source.sequence.to_be_bytes(),
            ],
        );
        state = sha256::digest_parts(
            b"eliot-search/direct-source-fence/fold/v1",
            &[&state, &record],
        );
    }
    sha256::hex(&state)
}
