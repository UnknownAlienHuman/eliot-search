use std::ffi::OsString;
use std::io;
use std::path::Path;

use crate::owner_composition::DataRootRequest;
use crate::service_output::{emit_indexed_source, emit_streaming_search, json_string};
use crate::storage_security::StorageSecurityStatus;

use super::commands::{
    cmd_gc_root, cmd_index_directory, cmd_read_revision, cmd_repair_root, cmd_retire_source,
};
use super::output::{emit_sources, emit_verification, write_stdout};
use super::store::{with_store_mut_request, with_store_request};
use super::support::require_count;

pub(super) fn dispatch(
    command: &str,
    arguments: &[OsString],
    cli_request: &DataRootRequest,
) -> Result<(), String> {
    cli_request.preflight()?;
    match command {
        "--inspect-catalog-recovery" => cmd_inspect_catalog_recovery(arguments, cli_request),
        "--initialize-data-root" | "--recover-initialization" => {
            require_count(arguments, 3)?;
            let request = arguments[2]
                .to_str()
                .ok_or_else(|| "DATA_ROOT_INITIALIZATION_ID_INVALID".to_owned())?;
            let request = crate::owner_composition::InitializationRequest::parse(request)
                .map_err(|error| error.code().to_owned())?;
            let root = Path::new(&arguments[1]);
            let receipt = if command == "--initialize-data-root" {
                crate::owner_composition::initialize_new_request(root, &request, cli_request)?
            } else {
                crate::owner_composition::recover_initialization_request(
                    root,
                    &request,
                    cli_request,
                )?
            };
            cli_request.preflight()?;
            write_stdout(
                cli_request,
                &format!(
                    "{{\"event\":\"data_root_initialized\",\"namespace_id\":{},\"replayed\":{}}}",
                    json_string(&crate::sha256::hex(&receipt.namespace)),
                    receipt.replayed,
                ),
            )
        }
        "--health-data-root" => {
            require_count(arguments, 2)?;
            with_store_request(
                Path::new(&arguments[1]),
                cli_request,
                |root, store, storage| {
                    let verification = store.verify_catalog()?;
                    cli_request.preflight()?;
                    write_stdout(
                        cli_request,
                        &format!(
                            concat!(
                                "{{\"event\":\"health\",\"namespace_id\":{},",
                                "\"registered_sources\":{},\"active_sources\":{},",
                                "\"verified_revisions\":{},\"verification\":\"catalog-metadata-only\",\"storage_security\":{},",
                                "\"encrypted_at_rest\":{}}}"
                            ),
                            json_string(&store.namespace_id()),
                            verification.registered_sources,
                            verification.active_sources,
                            verification.verified_revisions,
                            storage.json(),
                            storage.encrypted_at_rest,
                        ),
                    )?;
                    let _ = root;
                    Ok(())
                },
            )
        }
        "--index-file" => {
            require_count(arguments, 3)?;
            with_store_mut_request(Path::new(&arguments[1]), cli_request, |root, store| {
                let indexed = store.index_file(Path::new(&arguments[2]))?;
                store.verify()?;
                let storage =
                    StorageSecurityStatus::inspect_with_check(root, &|| cli_request.preflight())?;
                cli_request.preflight()?;
                let mut output = io::stdout().lock();
                emit_indexed_source(
                    &mut cli_request.output(&mut output),
                    &indexed,
                    0,
                    0,
                    &storage,
                )
            })
        }
        "--index-directory" => {
            require_count(arguments, 3)?;
            cmd_index_directory(arguments, cli_request)
        }
        "--search-root" | "--search-root-ascii-insensitive" => {
            require_count(arguments, 3)?;
            let query = arguments[2]
                .to_str()
                .ok_or_else(|| "DIRECT_QUERY_NOT_UTF8".to_owned())?;
            with_store_request(
                Path::new(&arguments[1]),
                cli_request,
                |_root, store, storage| {
                    store.verify()?;
                    let result =
                        store.search(query, command == "--search-root-ascii-insensitive")?;
                    cli_request.preflight()?;
                    let mut output = io::stdout().lock();
                    emit_streaming_search(
                        &mut cli_request.output(&mut output),
                        &store.namespace_id(),
                        &result,
                        storage,
                    )
                },
            )
        }
        "--list-sources" => {
            require_count(arguments, 2)?;
            with_store_request(
                Path::new(&arguments[1]),
                cli_request,
                |_root, store, storage| {
                    store.verify_catalog()?;
                    cli_request.preflight()?;
                    emit_sources(store, storage, cli_request)
                },
            )
        }
        "--verify-root" => {
            require_count(arguments, 2)?;
            with_store_request(
                Path::new(&arguments[1]),
                cli_request,
                |_root, store, storage| {
                    cli_request.preflight()?;
                    emit_verification(store, storage, cli_request)
                },
            )
        }
        "--retire-source" => {
            require_count(arguments, 3)?;
            cmd_retire_source(arguments, cli_request)
        }
        "--read-revision" => {
            require_count(arguments, 5)?;
            cmd_read_revision(arguments, cli_request)
        }
        "--repair-root" => {
            require_count(arguments, 2)?;
            cmd_repair_root(arguments)
        }
        "--gc-root" => {
            require_count(arguments, 3)?;
            cmd_gc_root(arguments, cli_request)
        }
        _ => Err("UNKNOWN_PERSISTENT_COMMAND".to_owned()),
    }
}

fn cmd_inspect_catalog_recovery(
    arguments: &[OsString],
    request: &DataRootRequest,
) -> Result<(), String> {
    require_count(arguments, 3)?;
    let name = arguments[2]
        .to_str()
        .ok_or_else(|| "OWNER_OPERATION_CONFLICT".to_owned())?;
    let name = crate::owner_composition::CatalogRecoveryRequest::parse(name)
        .map_err(|error| error.code().to_owned())?;
    let observation = crate::owner_composition::inspect_catalog_recovery_request(
        Path::new(&arguments[1]),
        &name,
        request,
    )?;
    write_stdout(
        request,
        &format!(
            concat!(
                "{{\"event\":\"catalog_recovery_inspection\",\"operation_id\":{},",
                "\"operation_kind\":{},\"recovery_state\":{},\"owner_epoch\":{},",
                "\"owner_generation\":{}}}"
            ),
            json_string(&observation.operation_id.to_string()),
            json_string(observation.kind.as_str()),
            json_string(observation.state.as_str()),
            observation.owner_epoch,
            observation.owner_generation,
        ),
    )
}
