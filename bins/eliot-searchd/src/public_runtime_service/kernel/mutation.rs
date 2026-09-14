//! Durable DIRECT mutations and their exact readback reporting.

use std::io::Write;
use std::path::Path;

use crate::continuation::ContinuationCatalog;
use crate::development::DataRootGuard;
use crate::direct_store::{DirectStore, PreparationCursor};
use crate::directory_manifest::{sync_directory, verify_directory_manifests};
use crate::maintenance_guard::guarded_collect_orphan_revisions;
use crate::result_handles::ResultHandleCatalog;
use crate::service_output::{emit_indexed_source, json_string, write_line};
use crate::storage_security::StorageSecurityStatus;

use super::codec::decode_path;
use super::diagnostics::refresh_storage;
use super::session::MutationAttempt;
use super::state::{CommandState, invalidate_search_state};

pub(super) fn cmd_migration_plan(
    writer: &mut impl Write,
    store: &DirectStore,
    owner: &DataRootGuard,
    attempt: &mut MutationAttempt,
    target_namespace: &str,
) -> Result<(), String> {
    let target = search_contracts::SourceNamespaceId::parse(target_namespace)
        .map_err(|_| "DIRECT_MIGRATION_TARGET_NAMESPACE_INVALID".to_owned())?;
    if target.as_bytes() == &[0; 16] {
        return Err("DIRECT_MIGRATION_TARGET_NAMESPACE_INVALID".to_owned());
    }
    attempt.arm();
    let result = store.stage_source_migration_plan(owner, target)?;
    write_line(writer, &result)
}

pub(super) fn cmd_index_directory(
    writer: &mut impl Write,
    store: &mut DirectStore,
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
    directory: &Path,
) -> Result<(), String> {
    let (invalidated_continuations, invalidated_handles) =
        invalidate_search_state(continuations, handles);
    let indexed = store.index_directory(directory)?;
    let changed = indexed.iter().filter(|source| source.changed).count();
    refresh_storage(storage, canonical_root)?;
    for source in &indexed {
        emit_indexed_source(writer, source, 0, 0, storage)?;
    }
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"directory_index_complete\",",
                "\"namespace_id\":\"{}\",\"sources\":{},",
                "\"changed\":{},\"invalidated_continuations\":{},",
                "\"invalidated_handles\":{},\"source_backed\":true,",
                "\"storage_backend\":{},\"encrypted_at_rest\":{}}}"
            ),
            store.namespace_id(),
            indexed.len(),
            changed,
            invalidated_continuations,
            invalidated_handles,
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )
}

pub(super) fn cmd_prepare_root(
    writer: &mut impl Write,
    store: &DirectStore,
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
    cursor: Option<&PreparationCursor>,
) -> Result<(), String> {
    let invalidated = invalidate_search_state(continuations, handles);
    let batch = store.prepare_root(cursor)?;
    refresh_storage(storage, canonical_root)?;
    crate::preparation_composition::emit_batch(writer, &batch, invalidated)
}

pub(super) fn cmd_prepare_revision(
    writer: &mut impl Write,
    store: &DirectStore,
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
    revision_id: &str,
) -> Result<(), String> {
    let invalidated = invalidate_search_state(continuations, handles);
    let gap = store.prepare_revision(revision_id)?;
    refresh_storage(storage, canonical_root)?;
    crate::preparation_composition::emit_prepared(
        writer,
        revision_id,
        invalidated,
        gap,
    )
}

pub(super) fn cmd_index_file(
    writer: &mut impl Write,
    store: &mut DirectStore,
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
    path: &Path,
) -> Result<(), String> {
    let (invalidated_continuations, invalidated_handles) =
        invalidate_search_state(continuations, handles);
    let indexed = store.index_file(path)?;
    refresh_storage(storage, canonical_root)?;
    emit_indexed_source(
        writer,
        &indexed,
        invalidated_continuations,
        invalidated_handles,
        storage,
    )
}

pub(super) fn cmd_sync_directory<W: Write>(
    state: &mut CommandState<'_, W>,
    path_hex: &str,
) -> Result<(), String> {
    let directory = decode_path(path_hex)?;
    let CommandState {
        writer,
        store,
        continuations,
        handles,
        canonical_root,
        storage,
        attempt,
    } = &mut *state;
    attempt.arm();
    let (invalidated_continuations, invalidated_handles) =
        invalidate_search_state(continuations, handles);
    let result = sync_directory(store, canonical_root, &directory)?;
    store.verify()?;
    let manifests =
        verify_directory_manifests(canonical_root, &store.namespace_id())?;
    refresh_storage(storage, canonical_root)?;
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"directory_sync_complete\",",
                "\"namespace_id\":\"{}\",\"directory_digest\":\"{}\",",
                "\"previous_generation\":{},\"generation\":{},",
                "\"previous_sources\":{},\"indexed_sources\":{},",
                "\"changed_sources\":{},\"missing_sources\":{},",
                "\"retired_sources\":{},\"moved_or_rebound_sources\":{},",
                "\"manifest_digest\":\"{}\",\"manifest_files\":{},",
                "\"invalidated_continuations\":{},",
                "\"invalidated_handles\":{},\"source_backed\":true,",
                "\"storage_backend\":{},\"encrypted_at_rest\":{}}}"
            ),
            result.namespace_id,
            result.directory_digest,
            result
                .previous_generation
                .map_or_else(|| "null".to_owned(), |value| value.to_string()),
            result.generation,
            result.previous_sources,
            result.indexed_sources,
            result.changed_sources,
            result.missing_sources,
            result.retired_sources,
            result.moved_or_rebound_sources,
            result.manifest_digest,
            manifests.manifest_files,
            invalidated_continuations,
            invalidated_handles,
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )
}

pub(super) fn cmd_gc(
    writer: &mut impl Write,
    store: &DirectStore,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
    apply: bool,
) -> Result<(), String> {
    let result = guarded_collect_orphan_revisions(canonical_root, apply)?;
    refresh_storage(storage, canonical_root)?;
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"direct_store_gc_complete\",",
                "\"namespace_id\":\"{}\",\"applied\":{},",
                "\"referenced_revisions\":{},\"scanned_objects\":{},",
                "\"plaintext_objects\":{},\"protected_objects\":{},",
                "\"temporary_objects\":{},",
                "\"referenced_plaintext_objects\":{},",
                "\"referenced_protected_objects\":{},",
                "\"orphan_objects\":{},\"orphan_bytes\":{},",
                "\"deleted_objects\":{},\"deleted_bytes\":{},",
                "\"unexpected_objects\":{},\"storage_security\":{},",
                "\"encrypted_at_rest\":{}}}"
            ),
            store.namespace_id(),
            result.applied,
            result.referenced_revisions,
            result.scanned_objects,
            result.plaintext_objects,
            result.protected_objects,
            result.temporary_objects,
            result.referenced_plaintext_objects,
            result.referenced_protected_objects,
            result.orphan_objects,
            result.orphan_bytes,
            result.deleted_objects,
            result.deleted_bytes,
            result.unexpected_objects,
            storage.json(),
            storage.encrypted_at_rest,
        ),
    )
}

pub(super) fn cmd_retire<W: Write>(
    state: &mut CommandState<'_, W>,
    source_id: &str,
) -> Result<(), String> {
    let source = state.store.retire_source(source_id)?;
    let (invalidated_continuations, invalidated_handles) =
        invalidate_search_state(state.continuations, state.handles);
    refresh_storage(state.storage, state.canonical_root)?;
    write_line(
        state.writer,
        &format!(
            concat!(
                "{{\"event\":\"source_retired\",",
                "\"source_id\":\"{}\",\"revision_id\":\"{}\",",
                "\"sequence\":{},\"active\":false,",
                "\"invalidated_continuations\":{},",
                "\"invalidated_handles\":{},\"storage_backend\":{},",
                "\"encrypted_at_rest\":{}}}"
            ),
            source.source_id,
            source.revision_id,
            source.sequence,
            invalidated_continuations,
            invalidated_handles,
            json_string(state.storage.backend),
            state.storage.encrypted_at_rest,
        ),
    )
}
