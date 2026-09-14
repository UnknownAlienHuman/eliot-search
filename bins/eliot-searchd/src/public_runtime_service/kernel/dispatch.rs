//! Fixed-order runtime command dispatch and quarantine admission.

use std::io::Write;

use crate::catalog_quarantine;
use crate::direct_store::PreparationCursor;
use crate::service_output::write_line;

use super::codec::decode_path;
use super::diagnostics::{
    cmd_health, cmd_list_sources, cmd_read_revision, cmd_status, cmd_verify_manifests,
    cmd_version, refresh_storage,
};
use super::mutation::{
    cmd_gc, cmd_index_directory, cmd_index_file, cmd_migration_plan,
    cmd_prepare_revision, cmd_prepare_root, cmd_retire, cmd_sync_directory,
};
use super::query::{
    cmd_continue, cmd_expand_handle, cmd_search_page, cmd_streaming_search,
};
use super::reporting::emit_verification;
use super::session::{MutationAttempt, ServiceControl};
use super::state::{CommandState, invalidate_search_state};

pub(super) fn execute_command(
    command: &str,
    store: &mut crate::direct_store::DirectStore,
    continuations: &mut crate::continuation::ContinuationCatalog,
    handles: &mut crate::result_handles::ResultHandleCatalog,
    owner: &crate::development::DataRootGuard,
    storage: &mut crate::storage_security::StorageSecurityStatus,
    operation: (&mut impl Write, &mut MutationAttempt),
) -> Result<ServiceControl, String> {
    let (writer, attempt) = operation;
    let canonical_root = owner.canonical_root();
    if let Err(quarantined) = catalog_quarantine::check(canonical_root) {
        let _ = invalidate_search_state(continuations, handles);
        return Err(quarantined);
    }
    let fields = command.split('\t').collect::<Vec<_>>();
    let Some(name) = fields.first().copied() else {
        return Err("SERVICE_COMMAND_EMPTY".to_owned());
    };
    match (name, fields.as_slice()) {
        ("health", [_]) => cmd_health(
            writer,
            store,
            continuations,
            handles,
            canonical_root,
            storage,
        )?,
        ("status", [_]) => {
            refresh_storage(storage, canonical_root)?;
            cmd_status(writer, store, storage)?;
        }
        ("version", [_]) => cmd_version(writer)?,
        ("shutdown", [_]) => {
            write_line(writer, "{\"event\":\"draining\",\"accepted\":true}")?;
            return Ok(ServiceControl::Stop);
        }
        ("verify", [_]) => {
            refresh_storage(storage, canonical_root)?;
            emit_verification(writer, store, canonical_root, storage)?;
        }
        ("verify-directory-manifests", [_]) => {
            cmd_verify_manifests(writer, store, canonical_root, storage)?;
        }
        ("list-sources", [_]) => {
            cmd_list_sources(writer, store, canonical_root, storage)?;
        }
        ("control-migration-plan", [_, target_namespace]) => {
            catalog_quarantine::arm(canonical_root)?;
            cmd_migration_plan(writer, store, owner, attempt, target_namespace)?;
            catalog_quarantine::clear(canonical_root)?;
        }
        ("control-migration-revisions", [_] | [_, _]) => {
            let page = store.inspect_migration_revisions(fields.get(1).copied())?;
            write_line(writer, &page)?;
        }
        ("control-migration-page", [_] | [_, _]) => {
            let page = crate::plaintext_direct_store::DirectStore::inspect_control_history(
                canonical_root,
                &store.namespace_id(),
                fields.get(1).copied(),
            )?;
            write_line(writer, &page)?;
        }
        ("control-migration-directories", [_] | [_, _]) => {
            let page = store.inspect_migration_directories(owner, fields.get(1).copied())?;
            write_line(writer, &page)?;
        }
        _ => {
            let mut state = CommandState {
                writer,
                store,
                continuations,
                handles,
                canonical_root,
                storage,
                attempt,
            };
            execute_mutating_command(name, fields.as_slice(), &mut state)?;
        }
    }
    Ok(ServiceControl::Continue)
}

fn execute_mutating_command<W: Write>(
    name: &str,
    fields: &[&str],
    state: &mut CommandState<'_, W>,
) -> Result<(), String> {
    if execute_quarantined_writes(name, fields, state)? {
        return Ok(());
    }
    match (name, fields) {
        ("search", [_, mode, query_hex]) => {
            cmd_streaming_search(state, mode, query_hex)?;
        }
        ("search-page", [_, mode, page_size, query_hex]) => {
            cmd_search_page(state, mode, page_size, query_hex)?;
        }
        ("continue", [_, token, page_size]) => {
            cmd_continue(state, token, page_size)?;
        }
        ("expand-handle", [_, token, start, end]) => {
            cmd_expand_handle(state, token, start, end)?;
        }
        ("read-revision", [_, revision_id, start, end]) => {
            cmd_read_revision(
                state.writer,
                state.store,
                state.canonical_root,
                state.storage,
                revision_id,
                start,
                end,
            )?;
        }
        ("gc", [_, mode]) => {
            let apply = match *mode {
                "dry-run" => false,
                "apply" => true,
                _ => return Err("SERVICE_GC_MODE_INVALID".to_owned()),
            };
            state.store.verify()?;
            let root = state.canonical_root;
            if apply {
                catalog_quarantine::arm(root)?;
                state.attempt.arm();
            }
            cmd_gc(
                state.writer,
                state.store,
                state.canonical_root,
                state.storage,
                apply,
            )?;
            if apply {
                catalog_quarantine::clear(root)?;
            }
        }
        _ => return Err("SERVICE_COMMAND_INVALID".to_owned()),
    }
    Ok(())
}

fn execute_quarantined_writes<W: Write>(
    name: &str,
    fields: &[&str],
    state: &mut CommandState<'_, W>,
) -> Result<bool, String> {
    match (name, fields) {
        ("prepare-root", [_] | [_, _]) => {
            let cursor = fields
                .get(1)
                .map(|value| PreparationCursor::parse(value))
                .transpose()?;
            state.store.validate_preparation_cursor(cursor.as_ref())?;
            let root = state.canonical_root;
            catalog_quarantine::arm(root)?;
            state.attempt.arm();
            cmd_prepare_root(
                state.writer,
                state.store,
                state.continuations,
                state.handles,
                state.canonical_root,
                state.storage,
                cursor.as_ref(),
            )?;
            catalog_quarantine::clear(root)?;
        }
        ("prepare-revision", [_, revision_id]) => {
            crate::preparation_composition::validate_revision(revision_id)?;
            let root = state.canonical_root;
            catalog_quarantine::arm(root)?;
            state.attempt.arm();
            cmd_prepare_revision(
                state.writer,
                state.store,
                state.continuations,
                state.handles,
                state.canonical_root,
                state.storage,
                revision_id,
            )?;
            catalog_quarantine::clear(root)?;
        }
        ("index-file", [_, path_hex]) => {
            let path = decode_path(path_hex)?;
            let root = state.canonical_root;
            catalog_quarantine::arm(root)?;
            state.attempt.arm();
            cmd_index_file(
                state.writer,
                state.store,
                state.continuations,
                state.handles,
                state.canonical_root,
                state.storage,
                &path,
            )?;
            catalog_quarantine::clear(root)?;
        }
        ("index-directory", [_, path_hex]) => {
            let directory = decode_path(path_hex)?;
            let root = state.canonical_root;
            catalog_quarantine::arm(root)?;
            state.attempt.arm();
            cmd_index_directory(
                state.writer,
                state.store,
                state.continuations,
                state.handles,
                state.canonical_root,
                state.storage,
                &directory,
            )?;
            catalog_quarantine::clear(root)?;
        }
        ("sync-directory", [_, path_hex]) => {
            let root = state.canonical_root;
            catalog_quarantine::arm(root)?;
            cmd_sync_directory(state, path_hex)?;
            catalog_quarantine::clear(root)?;
        }
        ("retire", [_, source_id]) => {
            let root = state.canonical_root;
            catalog_quarantine::arm(root)?;
            state.attempt.arm();
            cmd_retire(state, source_id)?;
            catalog_quarantine::clear(root)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
