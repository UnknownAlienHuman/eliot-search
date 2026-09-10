//! Owner-fenced DIRECT runtime with paged search and opaque source handles.
//!
//! The runtime owns one data-root lock, one verified DIRECT store, finite
//! process-local continuation windows, and finite process-local source handles.
//! Public paged results expose no source, revision, path, or content identities.

use std::env;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::catalog_quarantine;
use crate::continuation::{
    ContinuationCatalog, ContinuationError, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE,
};
use crate::development::{DataRootGuard, Health, MAX_SCAN_QUERY_BYTES};
use crate::direct_store::{DirectStore, PreparationCursor};
use crate::directory_manifest::{sync_directory, verify_directory_manifests};
use crate::maintenance_guard::guarded_collect_orphan_revisions;
use crate::result_handles::{MAX_HANDLE_EXPANSION_BYTES, ResultHandleCatalog, ResultHandleError};
use crate::service_output::{
    emit_handle_expansion, emit_indexed_source, emit_search_page, emit_streaming_search,
    json_string, write_line,
};
use crate::sha256;
use crate::storage_security::StorageSecurityStatus;

#[path = "service_session.rs"]
mod session;
use session::{MutationAttempt, ServiceControl};

const PROTOCOL_VERSION: u16 = 1;
const MAX_COMMAND_BYTES: usize = 256 * 1024;
const MAX_PATH_BYTES: usize = 32 * 1024;
const MAX_DIAGNOSTIC_REVISION_SLICE_BYTES: u64 = 24 * 1024;

/// Intercepts `--serve-data-root ROOT` before one-shot command dispatch.
pub fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().and_then(|value| value.to_str()) != Some("--serve-data-root") {
        return None;
    }
    let result = match arguments.as_slice() {
        [_, root] => run_service(Path::new(root)),
        _ => Err("USAGE_ERROR".to_owned()),
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            ExitCode::from(2)
        }
    })
}

fn run_service(root: &Path) -> Result<(), String> {
    let mut guard = DataRootGuard::acquire(root)?;
    // Persistent quarantine precedes catalog open and READY. Reopen observes
    // the marker instead of inventing empty state; no implicit repair follows.
    catalog_quarantine::check(guard.canonical_root())?;
    let mut store = DirectStore::open(guard.canonical_root())?;
    let verification = store.verify()?;
    let manifests = verify_directory_manifests(guard.canonical_root(), &store.namespace_id())?;
    let mut storage = StorageSecurityStatus::inspect(guard.canonical_root())?;
    let mut continuations = ContinuationCatalog::new(&store.namespace_id());
    let mut handles = ResultHandleCatalog::new(&store.namespace_id());

    let input = io::stdin();
    let mut reader = input.lock();
    let output = io::stdout();
    let mut writer = output.lock();
    // The single live owner binding is operator-observable: epoch,
    // installation incarnation and physical-root identity travel with READY
    // so a successor incarnation is never mistaken for its predecessor.
    let (owner_incarnation, owner_root, _) = guard.journal_owner_inputs();
    let effective = crate::config_composition::build_effective_defaults()
        .map_err(|error| format!("DAEMON_CONFIG_INVALID:{error}"))?;
    let readiness = crate::config_composition::derive_readiness(
        &effective,
        &crate::config_composition::direct_dependencies(),
        &crate::config_composition::AcceptedReceipts::default(),
    );
    write_line(
        &mut writer,
        &format!(
            concat!(
                "{{\"event\":\"data_root_ready\",",
                "\"protocol_version\":{},\"namespace_id\":\"{}\",",
                "\"registered_sources\":{},\"active_sources\":{},",
                "\"directory_manifests\":{},",
                "\"runtime_owner_ready\":true,\"direct_store_ready\":true,",
                "\"owner_epoch\":{},\"recovered_previous_active\":{},",
                "\"installation_incarnation_id\":\"{}\",",
                "\"data_root_id\":\"{}\",",
                "\"source_backed_search_available\":true,",
                "\"search_available\":{},",
                "\"indexed_search_available\":{},",
                "\"paged_search_available\":true,",
                "\"opaque_source_handles_available\":true,",
                "\"default_page_size\":{},\"max_page_size\":{},",
                "\"max_handle_expansion_bytes\":{},",
                "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
            ),
            PROTOCOL_VERSION,
            store.namespace_id(),
            verification.registered_sources,
            verification.active_sources,
            manifests.manifest_files,
            guard.epoch(),
            guard.recovered_previous_active(),
            owner_incarnation,
            owner_root,
            readiness.search_available,
            readiness.indexed_search_available,
            DEFAULT_PAGE_SIZE,
            MAX_PAGE_SIZE,
            MAX_HANDLE_EXPANSION_BYTES,
            storage.json(),
            storage.encrypted_at_rest,
        ),
    )?;

    let result = session::serve(
        &mut reader,
        &mut writer,
        MAX_COMMAND_BYTES,
        |command, output, attempt| {
            execute_command(
                command,
                &mut store,
                &mut continuations,
                &mut handles,
                &guard,
                &mut storage,
                (output, attempt),
            )
        },
    );
    drop(reader);
    if result.is_err() {
        // Invalidate before unwinding the store and finally releasing its owner.
        // No queued command, clean-stop receipt or automatic retry follows.
        invalidate_search_state(&mut continuations, &mut handles);
    }
    result?;
    // Guarded succession close-out: drain intent first, then the release
    // tombstone, both before the clean claim. Dependencies already shut down
    // in reverse startup order above; the guard drop below releases
    // exclusion last. Any persistence failure here refuses the clean claim
    // instead of relabelling an unknown outcome as success. The shutdown
    // receipt (epoch, generation and BLAKE3 hex of the exact RELEASED
    // record) travels with the stopped event as audit evidence.
    guard.begin_drain(search_runtime_owner::DrainReason::Shutdown)?;
    let receipt = guard.release_cleanly()?;
    write_line(
        &mut writer,
        &format!(
            concat!(
                "{{\"event\":\"data_root_stopped\",\"clean\":true,",
                "\"owner_epoch\":{},\"owner_generation\":{},",
                "\"owner_release_digest\":\"{}\"}}"
            ),
            receipt.epoch.get(),
            receipt.generation,
            sha256::hex(&receipt.record_digest),
        ),
    )?;
    Ok(())
}

/// Mutable per-command service state shared by multi-step command handlers
/// so no handler needs more than seven arguments.
struct CommandState<'a, W: std::io::Write> {
    writer: &'a mut W,
    store: &'a mut DirectStore,
    continuations: &'a mut ContinuationCatalog,
    handles: &'a mut ResultHandleCatalog,
    canonical_root: &'a Path,
    storage: &'a mut StorageSecurityStatus,
    attempt: &'a mut MutationAttempt,
}

fn execute_command(
    command: &str,
    store: &mut DirectStore,
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
    owner: &DataRootGuard,
    storage: &mut StorageSecurityStatus,
    operation: (&mut impl std::io::Write, &mut MutationAttempt),
) -> Result<ServiceControl, String> {
    let (writer, attempt) = operation;
    let canonical_root = owner.canonical_root();
    // While quarantined no query, mutation, GC or READY-adjacent read may use
    // stale memory. Invalidate handles/continuations before refusing.
    if let Err(quarantined) = catalog_quarantine::check(canonical_root) {
        let _ = invalidate_search_state(continuations, handles);
        return Err(quarantined);
    }
    let fields = command.split('\t').collect::<Vec<_>>();
    let Some(name) = fields.first().copied() else {
        return Err("SERVICE_COMMAND_EMPTY".to_owned());
    };
    match (name, fields.as_slice()) {
        ("health", [_]) => {
            cmd_health(
                writer,
                store,
                continuations,
                handles,
                canonical_root,
                storage,
            )?;
        }
        ("version", [_]) => {
            cmd_version(writer)?;
        }
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
            // This read-only source-history input uses the already-held owner;
            // it never opens a new catalog or arms a mutation attempt.
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

/// Dispatches commands that may arm a mutation attempt or invalidate search
/// state. Read-only commands stay in [`execute_command`].
fn execute_mutating_command<W: std::io::Write>(
    name: &str,
    fields: &[&str],
    state: &mut CommandState<'_, W>,
) -> Result<(), String> {
    if execute_quarantined_writes(name, fields, state)? {
        return Ok(());
    }
    match (name, fields) {
        ("search", [_, mode, query_hex]) => {
            let query = decode_query(query_hex)?;
            let result = state.store.search(&query, parse_search_mode(mode)?)?;
            refresh_storage(state.storage, state.canonical_root)?;
            emit_streaming_search(
                state.writer,
                &state.store.namespace_id(),
                &result,
                state.storage,
            )?;
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

/// Dispatches possibly durable catalog mutations with persistent quarantine.
///
/// Each arm precedes storage effects and each clear follows exact readback.
/// Returns `Ok(true)` when the command was a quarantined write.
fn execute_quarantined_writes<W: std::io::Write>(
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

fn cmd_version(writer: &mut impl std::io::Write) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            "{{\"event\":\"version\",\"binary\":\"eliot-searchd\",\"version\":\"{}\",\"protocol_version\":{}}}",
            env!("CARGO_PKG_VERSION"),
            PROTOCOL_VERSION,
        ),
    )
}

fn cmd_list_sources(
    writer: &mut impl std::io::Write,
    store: &DirectStore,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
) -> Result<(), String> {
    refresh_storage(storage, canonical_root)?;
    emit_source_list(writer, store, storage)
}

fn cmd_migration_plan(
    writer: &mut impl std::io::Write,
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

fn cmd_index_directory(
    writer: &mut impl std::io::Write,
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

fn cmd_prepare_root(
    writer: &mut impl std::io::Write,
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

fn cmd_prepare_revision(
    writer: &mut impl std::io::Write,
    store: &DirectStore,
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
    revision_id: &str,
) -> Result<(), String> {
    // Backfill can remove a search gap without changing any source
    // event. Source-fence equality alone cannot validate older pages.
    let invalidated = invalidate_search_state(continuations, handles);
    let gap = store.prepare_revision(revision_id)?;
    refresh_storage(storage, canonical_root)?;
    crate::preparation_composition::emit_prepared(writer, revision_id, invalidated, gap)
}

fn cmd_index_file(
    writer: &mut impl std::io::Write,
    store: &mut DirectStore,
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
    path: &Path,
) -> Result<(), String> {
    // Unchanged source bytes may still acquire previously missing preparation.
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

fn cmd_search_page<W: std::io::Write>(
    state: &mut CommandState<'_, W>,
    mode: &str,
    page_size: &str,
    query_hex: &str,
) -> Result<(), String> {
    let CommandState {
        writer,
        store,
        continuations,
        handles,
        canonical_root,
        storage,
        ..
    } = &mut *state;
    let query = decode_query(query_hex)?;
    let page_size = parse_page_size(page_size)?;
    let result = store.search(&query, parse_search_mode(mode)?)?;
    let page = continuations
        .create_page(store, result, page_size)
        .map_err(continuation_error)?;
    let public = handles
        .mint_page(store, &page.matches)
        .map_err(handle_error)?;
    refresh_storage(storage, canonical_root)?;
    emit_search_page(writer, &page, &public, storage)
}

fn cmd_continue<W: std::io::Write>(
    state: &mut CommandState<'_, W>,
    token: &str,
    page_size: &str,
) -> Result<(), String> {
    let CommandState {
        writer,
        store,
        continuations,
        handles,
        canonical_root,
        storage,
        ..
    } = &mut *state;
    let page_size = parse_page_size(page_size)?;
    let page = continuations
        .continue_page(store, token, page_size)
        .map_err(continuation_error)?;
    let public = handles
        .mint_page(store, &page.matches)
        .map_err(handle_error)?;
    refresh_storage(storage, canonical_root)?;
    emit_search_page(writer, &page, &public, storage)
}

fn cmd_expand_handle<W: std::io::Write>(
    state: &mut CommandState<'_, W>,
    token: &str,
    start: &str,
    end: &str,
) -> Result<(), String> {
    let CommandState {
        writer,
        store,
        canonical_root,
        storage,
        handles,
        ..
    } = &mut *state;
    let start = parse_u64(start, "SERVICE_START_OFFSET_INVALID")?;
    let end = parse_u64(end, "SERVICE_END_OFFSET_INVALID")?;
    let expansion = handles
        .expand(store, token, start, end)
        .map_err(handle_error)?;
    refresh_storage(storage, canonical_root)?;
    emit_handle_expansion(writer, &expansion, storage)
}

fn cmd_health(
    writer: &mut impl std::io::Write,
    store: &DirectStore,
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
) -> Result<(), String> {
    let verification = store.verify()?;
    let manifests = verify_directory_manifests(canonical_root, &store.namespace_id())?;
    refresh_storage(storage, canonical_root)?;
    let effective = crate::config_composition::build_effective_defaults()
        .map_err(|error| format!("DAEMON_CONFIG_INVALID:{error}"))?;
    let readiness = crate::config_composition::derive_readiness(
        &effective,
        &crate::config_composition::direct_dependencies(),
        &crate::config_composition::AcceptedReceipts::default(),
    );
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

fn cmd_verify_manifests(
    writer: &mut impl std::io::Write,
    store: &DirectStore,
    canonical_root: &Path,
    storage: &mut StorageSecurityStatus,
) -> Result<(), String> {
    let manifests = verify_directory_manifests(canonical_root, &store.namespace_id())?;
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

fn cmd_sync_directory<W: std::io::Write>(
    state: &mut CommandState<'_, W>,
    path_hex: &str,
) -> Result<(), String> {
    let CommandState {
        writer,
        store,
        continuations,
        handles,
        canonical_root,
        storage,
        attempt,
    } = &mut *state;
    let directory = decode_path(path_hex)?;
    attempt.arm();
    let (invalidated_continuations, invalidated_handles) =
        invalidate_search_state(continuations, handles);
    let result = sync_directory(store, canonical_root, &directory)?;
    store.verify()?;
    let manifests = verify_directory_manifests(canonical_root, &store.namespace_id())?;
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

fn cmd_read_revision(
    writer: &mut impl std::io::Write,
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

fn cmd_gc(
    writer: &mut impl std::io::Write,
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

fn refresh_storage(
    storage: &mut StorageSecurityStatus,
    canonical_root: &Path,
) -> Result<(), String> {
    *storage = StorageSecurityStatus::inspect(canonical_root)?;
    Ok(())
}

fn cmd_retire<W: std::io::Write>(
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

fn invalidate_search_state(
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
) -> (usize, usize) {
    (continuations.invalidate_all(), handles.invalidate_all())
}

fn emit_verification(
    writer: &mut impl std::io::Write,
    store: &DirectStore,
    canonical_root: &Path,
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    let verification = store.verify()?;
    let manifests = verify_directory_manifests(canonical_root, &store.namespace_id())?;
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

fn emit_source_list(
    writer: &mut impl std::io::Write,
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

fn continuation_error(error: ContinuationError) -> String {
    error.code().to_owned()
}

fn handle_error(error: ResultHandleError) -> String {
    error.code().to_owned()
}

fn parse_search_mode(value: &str) -> Result<bool, String> {
    match value {
        "sensitive" => Ok(false),
        "ascii-insensitive" => Ok(true),
        _ => Err("SERVICE_SEARCH_MODE_INVALID".to_owned()),
    }
}

fn parse_page_size(value: &str) -> Result<usize, String> {
    let value = value
        .parse::<usize>()
        .map_err(|_| "DIRECT_CONTINUATION_PAGE_SIZE_INVALID".to_owned())?;
    if value == 0 || value > MAX_PAGE_SIZE {
        Err("DIRECT_CONTINUATION_PAGE_SIZE_INVALID".to_owned())
    } else {
        Ok(value)
    }
}

fn decode_query(value: &str) -> Result<String, String> {
    String::from_utf8(decode_hex(value, MAX_SCAN_QUERY_BYTES)?)
        .map_err(|_| "SERVICE_QUERY_NOT_UTF8".to_owned())
}

fn decode_path(value: &str) -> Result<PathBuf, String> {
    decode_os_string(&decode_hex(value, MAX_PATH_BYTES)?).map(PathBuf::from)
}

#[cfg(unix)]
fn decode_os_string(bytes: &[u8]) -> Result<OsString, String> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes.to_vec()))
}

#[cfg(windows)]
fn decode_os_string(bytes: &[u8]) -> Result<OsString, String> {
    use std::os::windows::ffi::OsStringExt;
    if !bytes.len().is_multiple_of(2) {
        return Err("SERVICE_PATH_ENCODING_INVALID".to_owned());
    }
    let wide = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    Ok(OsString::from_wide(&wide))
}

#[cfg(not(any(unix, windows)))]
fn decode_os_string(bytes: &[u8]) -> Result<OsString, String> {
    String::from_utf8(bytes.to_vec())
        .map(OsString::from)
        .map_err(|_| "SERVICE_PATH_ENCODING_INVALID".to_owned())
}

fn decode_hex(value: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) || value.len() / 2 > max_bytes {
        return Err("SERVICE_HEX_INVALID".to_owned());
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().as_chunks::<2>().0 {
        let high = hex_nibble(pair[0]).ok_or_else(|| "SERVICE_HEX_INVALID".to_owned())?;
        let low = hex_nibble(pair[1]).ok_or_else(|| "SERVICE_HEX_INVALID".to_owned())?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn parse_u64(value: &str, error: &'static str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|_| error.to_owned())
}
