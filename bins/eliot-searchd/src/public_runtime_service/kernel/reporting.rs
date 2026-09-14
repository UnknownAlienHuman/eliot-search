//! Read-only verification and source-list event rendering.

use std::io::Write;
use std::path::Path;

use crate::direct_store::DirectStore;
use crate::directory_manifest::verify_directory_manifests;
use crate::service_output::{json_string, write_line};
use crate::storage_security::StorageSecurityStatus;

pub(super) fn emit_verification(
    writer: &mut impl Write,
    store: &DirectStore,
    canonical_root: &Path,
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    let verification = store.verify()?;
    let manifests =
        verify_directory_manifests(canonical_root, &store.namespace_id())?;
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"direct_store_verified\",",
                "\"namespace_id\":\"{}\",\"source_events\":{},",
                "\"registered_sources\":{},\"active_sources\":{},",
                "\"referenced_revisions\":{},\"verified_revisions\":{},",
                "\"total_revision_bytes\":{},\"manifest_files\":{},",
                "\"manifest_directories\":{},\"source_backed\":true,",
                "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
            ),
            store.namespace_id(),
            verification.source_events,
            verification.registered_sources,
            verification.active_sources,
            verification.referenced_revisions,
            verification.verified_revisions,
            verification.total_revision_bytes,
            manifests.manifest_files,
            manifests.directories,
            storage.json(),
            storage.encrypted_at_rest,
        ),
    )
}

pub(super) fn emit_source_list(
    writer: &mut impl Write,
    store: &DirectStore,
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    let sources = store.list_sources();
    for source in &sources {
        write_line(
            writer,
            &format!(
                concat!(
                    "{{\"event\":\"source\",\"source_id\":\"{}\",",
                    "\"revision_id\":\"{}\",\"content_digest\":\"{}\",",
                    "\"path_digest\":\"{}\",\"byte_length\":{},",
                    "\"identity_strength\":\"{}\",\"active\":{},",
                    "\"sequence\":{},\"diagnostic_internal_identifiers\":true}}"
                ),
                source.source_id,
                source.revision_id,
                source.content_digest,
                source.path_digest,
                source.byte_length,
                source.identity_strength,
                source.active,
                source.sequence,
            ),
        )?;
    }
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"source_list_complete\",",
                "\"namespace_id\":\"{}\",\"sources\":{},",
                "\"diagnostic_internal_identifiers\":true,",
                "\"storage_backend\":{},\"encrypted_at_rest\":{}}}"
            ),
            store.namespace_id(),
            sources.len(),
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )
}
