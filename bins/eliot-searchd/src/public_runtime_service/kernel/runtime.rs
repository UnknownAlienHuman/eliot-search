//! Owner lifetime, startup readiness, fail-stop session and clean release.

use std::env;
use std::io;
use std::path::Path;

use crate::catalog_quarantine;
use crate::continuation::{ContinuationCatalog, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE};
use crate::development::DataRootGuard;
use crate::direct_store::DirectStore;
use crate::directory_manifest::verify_directory_manifests_with_check;
use crate::result_handles::{MAX_HANDLE_EXPANSION_BYTES, ResultHandleCatalog};
use crate::service_output::write_line;
use crate::sha256;
use crate::storage_security::StorageSecurityStatus;

use super::dispatch::execute_command;
use super::session;
use super::spec::{MAX_COMMAND_BYTES, PROTOCOL_VERSION};
use super::state::invalidate_search_state;

pub(super) fn run_service(
    root: &Path,
    startup: &crate::owner_composition::DataRootRequest,
) -> Result<(), String> {
    let mut guard = DataRootGuard::open_existing_request(root, startup)?;
    catalog_quarantine::check(guard.canonical_root())?;
    let mut store = DirectStore::open_existing_mutating(&guard)?;
    let verification = store.verify()?;
    let manifests = verify_directory_manifests_with_check(
        guard.canonical_root(),
        &store.namespace_id(),
        &|| store.check_operation(),
    )?;
    let mut storage =
        StorageSecurityStatus::inspect_with_check(guard.canonical_root(), &|| startup.preflight())?;
    let mut continuations = ContinuationCatalog::new(&store.namespace_id());
    let mut handles = ResultHandleCatalog::new(&store.namespace_id());

    let input = io::stdin();
    let mut reader = input.lock();
    let output = io::stdout();
    let mut writer = output.lock();
    let (owner_incarnation, owner_root, _) = guard.journal_owner_inputs();
    let cli_args = env::args_os().skip(1).collect::<Vec<_>>();
    let (_, cli) = crate::config_composition::strip_config_args_os(&cli_args)?;
    let effective = crate::config_composition::effective_from_process(&cli)?;
    let readiness = crate::config_composition::derive_readiness(
        &effective,
        crate::config_composition::direct_dependencies(),
        crate::config_composition::AcceptedReceipts::default(),
    );
    guard.verify_existing()?;
    write_line(
        &mut startup.output(&mut writer),
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
            readiness.capabilities.search_available,
            readiness.capabilities.indexed_search_available,
            DEFAULT_PAGE_SIZE,
            MAX_PAGE_SIZE,
            MAX_HANDLE_EXPANSION_BYTES,
            storage.json(),
            storage.encrypted_at_rest,
        ),
    )?;

    guard.finish_service_startup(startup)?;
    let mut shutdown_request = None;
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
                (output, attempt, &mut shutdown_request),
            )
        },
    );
    drop(reader);
    if result.is_err() {
        invalidate_search_state(&mut continuations, &mut handles);
    }
    result?;
    let shutdown_request = match shutdown_request {
        Some(request) => request,
        None => guard
            .admit_service_command("eof-shutdown")?
            .request()
            .retain(),
    };
    // Admission has stopped; retire dependent state before publishing RELEASED.
    invalidate_search_state(&mut continuations, &mut handles);
    drop(continuations);
    drop(handles);
    drop(store);
    guard.bind_service_shutdown(&shutdown_request)?;
    guard.begin_drain(search_runtime_owner::DrainReason::Shutdown)?;
    let receipt = guard.release_cleanly()?;
    write_line(
        &mut shutdown_request.output(&mut writer),
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
