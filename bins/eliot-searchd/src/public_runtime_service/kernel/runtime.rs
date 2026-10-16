//! Retained native read custody with finite startup, command and shutdown contexts.

use std::ffi::OsString;
use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::config_composition::EffectiveDaemonConfig;
use crate::continuation::{ContinuationCatalog, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE};
use crate::direct_store::DirectRead;
use crate::directory_manifest::verify_directory_manifests;
use crate::owner_composition::{
    NativeRootAdmission, NativeRootLimits, NativeRootRequest, admit_existing,
};
use crate::result_handles::{MAX_HANDLE_EXPANSION_BYTES, ResultHandleCatalog};
use crate::service_output::{write_error, write_line};
use crate::storage_security::StorageSecurityStatus;
use search_runtime_owner::{OwnerError, RootOpenState};

use super::diagnostics::current_readiness;
use super::dispatch::{command_refusal, execute_command};
use super::session::{self, MutationAttempt, ServiceControl};
use super::spec::{MAX_COMMAND_BYTES, PROTOCOL_VERSION};
use super::state::invalidate_search_state;
use crate::direct_store::NativeOutput;

fn operation_context(started: Instant) -> Result<(Instant, Arc<AtomicBool>), OwnerError> {
    let deadline = started
        .checked_add(Duration::from_secs(120))
        .ok_or(OwnerError::OwnerDeadlineInvalid)?;
    Ok((deadline, Arc::new(AtomicBool::new(false))))
}

fn native_failure(attempt: &mut MutationAttempt, error: OwnerError) -> String {
    attempt.refuse_native(error);
    error.code().to_owned()
}

pub(super) fn run_service(arguments: &[OsString], started: Instant) -> Result<(), String> {
    let (deadline, cancellation) = operation_context(started).map_err(owner_error)?;
    let limits =
        NativeRootLimits::new([256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256])
            .map_err(owner_error)?;
    let request = NativeRootRequest::for_service(arguments, deadline, cancellation, limits)
        .map_err(owner_error)?;
    let mut startup = match admit_existing(request).map_err(owner_error)? {
        NativeRootAdmission::Inspection(owner) => owner.read_startup().map_err(owner_error)?,
        NativeRootAdmission::Unavailable(RootOpenState::Absent, _) => {
            return Err("SERVICE_ROOT_UNPROVISIONED".to_owned());
        }
        NativeRootAdmission::Unavailable(_, _) | NativeRootAdmission::Mutation(_) => {
            return Err("SERVICE_NATIVE_ROOT_UNAVAILABLE".to_owned());
        }
    };
    let (_, cli) = crate::config_composition::strip_config_args_os(arguments)?;
    let config = crate::config_composition::effective_from_process(&cli)?;
    let output = io::stdout();
    let mut writer = output.lock();
    let (namespace, mut storage) = announce_startup(&mut startup, &mut writer, &config)?;
    let mut service = startup.finish().map_err(owner_error)?;
    let mut continuations = ContinuationCatalog::new(&namespace);
    let mut handles = ResultHandleCatalog::new(&namespace);
    let input = io::stdin();
    let mut reader = input.lock();
    let mut shutdown = None;
    let result = session::serve(
        &mut reader,
        &mut writer,
        MAX_COMMAND_BYTES,
        |command, output, attempt| {
            // Pure name validation precedes native admission and store work.
            let refusal = command_refusal(command);
            let (deadline, cancellation) = operation_context(Instant::now())
                .map_err(|error| native_failure(attempt, error))?;
            if let Some(code) = refusal {
                service
                    .refuse_command(command, deadline, cancellation, output, code)
                    .map_err(|error| native_failure(attempt, error))?;
                return Ok(ServiceControl::Continue);
            }
            let child = service
                .read_command(command, deadline, cancellation)
                .map_err(|error| native_failure(attempt, error))?;
            let result = execute_command(
                command,
                &child,
                &mut continuations,
                &mut handles,
                &mut storage,
                &mut NativeOutput::new(&child, output),
                &config,
            );
            child
                .native_checkpoint()
                .map_err(|error| native_failure(attempt, error))?;
            let control = match result {
                Ok(control) => control,
                Err(error) if output.incomplete() => return Err(error),
                Err(error) => {
                    let emitted = write_error(&mut NativeOutput::new(&child, output), &error);
                    if let Some(refusal) = child.native_refusal() {
                        return Err(native_failure(attempt, refusal));
                    }
                    emitted?;
                    child
                        .native_checkpoint()
                        .map_err(|error| native_failure(attempt, error))?;
                    ServiceControl::Continue
                }
            };
            if control == ServiceControl::Stop {
                shutdown = Some(child.retain_shutdown().map_err(|error| {
                    attempt.refuse_native(error);
                    error.code().to_owned()
                })?);
            }
            Ok(control)
        },
    );
    drop(reader);
    invalidate_search_state(&mut continuations, &mut handles);
    result?;
    let shutdown = if let Some(retained) = shutdown {
        retained
    } else {
        // EOF is a new operation, captured before any teardown. It never
        // renews a previously dispatched shutdown or incomplete command.
        let (deadline, cancellation) = operation_context(Instant::now()).map_err(owner_error)?;
        service
            .read_command("shutdown/eof", deadline, cancellation)
            .map_err(owner_error)?
            .retain_shutdown()
            .map_err(owner_error)?
    };
    drop(continuations);
    drop(handles);
    service
        .close()
        .finish(shutdown)
        .map_err(owner_error)?
        .emit(&mut writer)
        .map_err(owner_error)
}

fn owner_error(error: OwnerError) -> String {
    error.code().to_owned()
}

fn announce_startup(
    startup: &mut crate::owner_composition::NativeReadStartup,
    writer: &mut impl io::Write,
    config: &EffectiveDaemonConfig,
) -> Result<(String, StorageSecurityStatus), String> {
    let identity = startup.identity().map_err(owner_error)?;
    let store = startup.read();
    let root = store.root();
    let verification = store.verify()?;
    let namespace = store.namespace_id()?;
    let manifests = verify_directory_manifests(&root, &namespace)?;
    let storage = StorageSecurityStatus::inspect(&root)?;
    let readiness = current_readiness(config);
    write_line(
        &mut NativeOutput::new(&store, writer),
        &format!(
            concat!(
                "{{\"event\":\"data_root_ready\",",
                "\"protocol_version\":{},\"namespace_id\":\"{}\",",
                "\"registered_sources\":{},\"active_sources\":{},",
                "\"directory_manifests\":{},",
                "\"runtime_owner_ready\":true,\"direct_store_ready\":true,",
                "\"owner_epoch\":{},\"recovered_previous_active\":false,",
                "\"installation_incarnation_id\":\"{}\",\"data_root_id\":\"{}\",",
                "\"read_only\":true,\"service_mutations_available\":false,",
                "\"service_control_migration_available\":false,\"owner_state_written\":false,",
                "\"source_backed_search_available\":true,",
                "\"search_available\":{},\"indexed_search_available\":{},",
                "\"paged_search_available\":true,\"opaque_source_handles_available\":true,",
                "\"default_page_size\":{},\"max_page_size\":{},",
                "\"max_handle_expansion_bytes\":{},",
                "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
            ),
            PROTOCOL_VERSION,
            namespace,
            verification.registered_sources,
            verification.active_sources,
            manifests.manifest_files,
            identity.epoch,
            identity.installation_incarnation,
            identity.root,
            readiness.capabilities.search_available,
            readiness.capabilities.indexed_search_available,
            DEFAULT_PAGE_SIZE,
            MAX_PAGE_SIZE,
            MAX_HANDLE_EXPANSION_BYTES,
            storage.json(),
            storage.encrypted_at_rest,
        ),
    )?;
    store.checkpoint()?;
    Ok((namespace, storage))
}
