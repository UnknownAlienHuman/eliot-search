//! Fixed-order runtime command dispatch and quarantine admission.

use std::io::Write;

#[cfg(test)]
use crate::catalog_quarantine;
use crate::config_composition::EffectiveDaemonConfig;
#[cfg(test)]
use crate::direct_store::PreparationCursor;
use crate::direct_store::{DirectRead, ReadOnlyStore};
use crate::service_output::write_line;

#[cfg(test)]
use super::codec::decode_path;
use super::diagnostics::{
    cmd_health, cmd_list_sources, cmd_read_revision, cmd_status, cmd_verify_manifests, cmd_version,
    refresh_storage,
};
#[cfg(test)]
use super::mutation::{
    cmd_gc, cmd_index_directory, cmd_index_file, cmd_migration_plan, cmd_prepare_revision,
    cmd_prepare_root, cmd_retire, cmd_sync_directory,
};
use super::query::{cmd_continue, cmd_expand_handle, cmd_search_page, cmd_streaming_search};
use super::reporting::emit_verification;
use super::session::ServiceControl;
#[cfg(test)]
use super::state::CommandState;
use super::state::ReadCommandState;

pub(super) fn execute_command(
    command: &str,
    store: &ReadOnlyStore<'_>,
    continuations: &mut crate::continuation::ContinuationCatalog,
    handles: &mut crate::result_handles::ResultHandleCatalog,
    storage: &mut crate::storage_security::StorageSecurityStatus,
    writer: &mut impl Write,
    config: &EffectiveDaemonConfig,
) -> Result<ServiceControl, String> {
    with_read_command(command, |name| {
        store.checkpoint()?;
        let root = store.root();
        let fields = command.split('\t').take(6).collect::<Vec<_>>();
        match (name, fields.as_slice()) {
            ("health", [_]) => cmd_health(
                writer,
                store,
                continuations,
                handles,
                &root,
                storage,
                config,
            )?,
            ("status", [_]) => {
                refresh_storage(storage, &root)?;
                cmd_status(writer, store, storage, config)?;
            }
            ("version", [_]) => cmd_version(writer)?,
            ("shutdown", [_]) => {
                write_line(writer, "{\"event\":\"draining\",\"accepted\":true}")?;
                return Ok(ServiceControl::Stop);
            }
            ("verify", [_]) => {
                refresh_storage(storage, &root)?;
                emit_verification(writer, store, &root, storage)?;
            }
            ("verify-directory-manifests", [_]) => {
                cmd_verify_manifests(writer, store, &root, storage)?;
            }
            ("list-sources", [_]) => cmd_list_sources(writer, store, &root, storage)?,
            _ => {
                let mut state = ReadCommandState {
                    writer,
                    store,
                    continuations,
                    handles,
                    canonical_root: &root,
                    storage,
                };
                execute_read_command(name, fields.as_slice(), &mut state)?;
            }
        }
        Ok(ServiceControl::Continue)
    })
}
fn with_read_command(
    command: &str,
    route: impl FnOnce(&str) -> Result<ServiceControl, String>,
) -> Result<ServiceControl, String> {
    if let Some(code) = command_refusal(command) {
        return Err(code.to_owned());
    }
    let name = command
        .split('\t')
        .next()
        .ok_or_else(|| "SERVICE_COMMAND_EMPTY".to_owned())?;
    route(name)
}

pub(super) fn command_refusal(command: &str) -> Option<&'static str> {
    let name = command.split('\t').next().unwrap_or_default();
    if name.is_empty() {
        return Some("SERVICE_COMMAND_INVALID");
    }
    if matches!(
        name,
        "prepare-root"
            | "prepare-revision"
            | "index-file"
            | "index-directory"
            | "sync-directory"
            | "retire"
            | "gc"
            | "control-migration-plan"
            | "control-migration-revisions"
            | "control-migration-page"
            | "control-migration-directories"
    ) {
        return Some("SERVICE_MUTATION_UNAVAILABLE");
    }
    None
}

fn execute_read_command<W: Write, S: DirectRead>(
    name: &str,
    fields: &[&str],
    state: &mut ReadCommandState<'_, W, S>,
) -> Result<(), String> {
    match (name, fields) {
        ("search", [_, mode, query_hex]) => cmd_streaming_search(state, mode, query_hex)?,
        ("search-page", [_, mode, page_size, query_hex]) => {
            cmd_search_page(state, mode, page_size, query_hex)?;
        }
        ("continue", [_, token, page_size]) => cmd_continue(state, token, page_size)?,
        ("expand-handle", [_, token, start, end]) => cmd_expand_handle(state, token, start, end)?,
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
        _ => return Err("SERVICE_COMMAND_INVALID".to_owned()),
    }
    Ok(())
}

#[cfg(test)]
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
            cmd_streaming_search(&mut reborrow_read(state), mode, query_hex)?;
        }
        ("search-page", [_, mode, page_size, query_hex]) => {
            cmd_search_page(&mut reborrow_read(state), mode, page_size, query_hex)?;
        }
        ("continue", [_, token, page_size]) => {
            cmd_continue(&mut reborrow_read(state), token, page_size)?;
        }
        ("expand-handle", [_, token, start, end]) => {
            cmd_expand_handle(&mut reborrow_read(state), token, start, end)?;
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

#[cfg(test)]
fn reborrow_read<'borrow, W: Write>(
    state: &'borrow mut CommandState<'_, W>,
) -> ReadCommandState<'borrow, W, crate::direct_store::DirectStore> {
    ReadCommandState {
        writer: &mut *state.writer,
        store: &*state.store,
        continuations: &mut *state.continuations,
        handles: &mut *state.handles,
        canonical_root: state.canonical_root,
        storage: &mut *state.storage,
    }
}

#[cfg(test)]
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

#[cfg(test)]
mod availability_tests {
    use super::*;

    #[test]
    fn unavailable_commands_refuse_before_any_routing_or_argument_work() {
        for name in [
            "prepare-root",
            "prepare-revision",
            "index-file",
            "index-directory",
            "sync-directory",
            "retire",
            "gc",
            "control-migration-plan",
            "control-migration-revisions",
            "control-migration-page",
            "control-migration-directories",
        ] {
            for command in [name.to_owned(), format!("{name}\tbad\tbad\textra")] {
                let mut calls = 0;
                let result = with_read_command(&command, |_| {
                    calls += 1;
                    Ok(ServiceControl::Continue)
                });
                assert_eq!(result, Err("SERVICE_MUTATION_UNAVAILABLE".to_owned()));
                assert_eq!(calls, 0, "{command}");
            }
        }
        let mut calls = 0;
        let result = with_read_command("health", |name| {
            assert_eq!(name, "health");
            calls += 1;
            Ok(ServiceControl::Continue)
        });
        assert_eq!(result, Ok(ServiceControl::Continue));
        assert_eq!(calls, 1);
    }
}
