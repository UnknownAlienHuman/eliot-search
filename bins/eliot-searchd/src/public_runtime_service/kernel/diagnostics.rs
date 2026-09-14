//! Read-only runtime status and diagnostic commands.

use std::env;
use std::io::Write;
use std::path::Path;

use crate::continuation::ContinuationCatalog;
use crate::development::Health;
use crate::direct_store::DirectStore;
use crate::directory_manifest::verify_directory_manifests;
use crate::result_handles::ResultHandleCatalog;
use crate::service_output::{emit_provider_status, json_string, write_line};
use crate::sha256;
use crate::storage_security::StorageSecurityStatus;

use super::codec::parse_u64;
use super::reporting::emit_source_list;
use super::spec::{MAX_DIAGNOSTIC_REVISION_SLICE_BYTES, PROTOCOL_VERSION};

pub(super) fn cmd_version(writer: &mut impl Write) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            "{{\"event\":\"version\",\"binary\":\"eliot-searchd\",\"version\":\"{}\",\"protocol_version\":{}}}",
            env!("CARGO_PKG_VERSION"),
            PROTOCOL_VERSION,
        ),
    )
}

pub(super) fn cmd_list_sources(
    writer: &mut impl Write,
    store: &DirectStore,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
) -> Result<(), String> {
    refresh_storage(storage, canonical_root)?;
    emit_source_list(writer, store, storage)
}

pub(super) fn cmd_status(
    writer: &mut impl Write,
    store: &DirectStore,
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    let readiness = current_readiness()?;
    emit_provider_status(
        writer,
        &store.namespace_id(),
        readiness.capabilities.search_available,
        readiness.capabilities.indexed_search_available,
        readiness.capabilities.source_backed_search_available,
        &readiness.blockers,
        storage,
    )
}

fn current_readiness(
) -> Result<crate::config_composition::ReadinessReport, String> {
    let cli_args: Vec<String> = env::args().skip(1).collect();
    let (_, cli) = crate::config_composition::parse_cli_config_args(&cli_args)?;
    let effective = crate::config_composition::effective_from_process(&cli)?;
    Ok(crate::config_composition::derive_readiness(
        &effective,
        crate::config_composition::direct_dependencies(),
        crate::config_composition::AcceptedReceipts::default(),
    ))
}

pub(super) fn cmd_health(
    writer: &mut impl Write,
    store: &DirectStore,
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
) -> Result<(), String> {
    let verification = store.verify()?;
    let manifests =
        verify_directory_manifests(canonical_root, &store.namespace_id())?;
    refresh_storage(storage, canonical_root)?;
    let readiness = current_readiness()?;
    let health = Health::from_readiness(&readiness);
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"health\",\"namespace_id\":\"{}\",",
                "\"registered_sources\":{},\"active_sources\":{},",
                "\"verified_revisions\":{},\"directory_manifests\":{},",
                "\"live_continuations\":{},",
                "\"retained_continuation_matches\":{},",
                "\"live_source_handles\":{},\"health\":{},",
                "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
            ),
            store.namespace_id(),
            verification.registered_sources,
            verification.active_sources,
            verification.verified_revisions,
            manifests.manifest_files,
            continuations.live_count(),
            continuations.retained_matches(),
            handles.live_count(),
            health.json(),
            storage.json(),
            storage.encrypted_at_rest,
        ),
    )
}

pub(super) fn cmd_verify_manifests(
    writer: &mut impl Write,
    store: &DirectStore,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
) -> Result<(), String> {
    let manifests =
        verify_directory_manifests(canonical_root, &store.namespace_id())?;
    refresh_storage(storage, canonical_root)?;
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"directory_manifests_verified\",",
                "\"namespace_id\":\"{}\",\"manifest_files\":{},",
                "\"directories\":{},\"current_entries\":{},",
                "\"highest_generation\":{},\"source_backed\":true,",
                "\"storage_backend\":{},\"encrypted_at_rest\":{}}}"
            ),
            store.namespace_id(),
            manifests.manifest_files,
            manifests.directories,
            manifests.current_entries,
            manifests.highest_generation,
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )
}

pub(super) fn cmd_read_revision(
    writer: &mut impl Write,
    store: &DirectStore,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
    revision_id: &str,
    start: &str,
    end: &str,
) -> Result<(), String> {
    let start = parse_u64(start, "SERVICE_START_OFFSET_INVALID")?;
    let end = parse_u64(end, "SERVICE_END_OFFSET_INVALID")?;
    if end.saturating_sub(start) > MAX_DIAGNOSTIC_REVISION_SLICE_BYTES {
        return Err("SERVICE_REVISION_SLICE_TOO_LARGE".to_owned());
    }
    let slice = store.read_revision_range(revision_id, start, end)?;
    refresh_storage(storage, canonical_root)?;
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"revision_slice\",",
                "\"revision_id\":\"{}\",\"content_digest\":\"{}\",",
                "\"byte_start\":{},\"byte_end\":{},",
                "\"encoding\":\"hex\",\"bytes\":\"{}\",",
                "\"diagnostic_internal_identifiers\":true,",
                "\"source_backed\":true,\"storage_backend\":{},",
                "\"encrypted_at_rest\":{}}}"
            ),
            slice.revision_id,
            slice.content_digest,
            slice.byte_start,
            slice.byte_end,
            sha256::hex(&slice.bytes),
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )
}

pub(super) fn refresh_storage(
    storage: &mut StorageSecurityStatus,
    canonical_root: &Path,
) -> Result<(), String> {
    *storage = StorageSecurityStatus::inspect(canonical_root)?;
    Ok(())
}
