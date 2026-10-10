use std::ffi::OsString;
use std::io;
use std::path::Path;

use crate::service_output::{emit_indexed_source, emit_streaming_search, json_string};
use crate::storage_security::StorageSecurityStatus;

use super::commands::{
    cmd_gc_root, cmd_index_directory, cmd_read_revision, cmd_repair_root, cmd_retire_source,
};
use super::output::{emit_sources, emit_verification, write_stdout};
use super::store::{with_store, with_store_mut};
use super::support::require_count;

pub(super) fn dispatch(command: &str, arguments: &[OsString]) -> Result<(), String> {
    match command {
        "--initialize-data-root" | "--recover-initialization" => {
            require_count(arguments, 3)?;
            let request = arguments[2]
                .to_str()
                .ok_or_else(|| "DATA_ROOT_INITIALIZATION_ID_INVALID".to_owned())?;
            let request = crate::owner_composition::InitializationRequest::parse(request)
                .map_err(|error| error.code().to_owned())?;
            let root = Path::new(&arguments[1]);
            let receipt = if command == "--initialize-data-root" {
                crate::owner_composition::initialize_new(root, &request)?
            } else {
                crate::owner_composition::recover_initialization(root, &request)?
            };
            write_stdout(&format!(
                "{{\"event\":\"data_root_initialized\",\"namespace_id\":{},\"replayed\":{}}}",
                json_string(&crate::sha256::hex(&receipt.namespace)),
                receipt.replayed,
            ))
        }
        "--health-data-root" => {
            require_count(arguments, 2)?;
            with_store(Path::new(&arguments[1]), |root, store, storage| {
                let verification = store.verify_catalog()?;
                write_stdout(&format!(
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
                ))?;
                let _ = root;
                Ok(())
            })
        }
        "--index-file" => {
            require_count(arguments, 3)?;
            with_store_mut(Path::new(&arguments[1]), |root, store| {
                let indexed = store.index_file(Path::new(&arguments[2]))?;
                store.verify()?;
                let storage = StorageSecurityStatus::inspect(root)?;
                let mut output = io::stdout().lock();
                emit_indexed_source(&mut output, &indexed, 0, 0, &storage)
            })
        }
        "--index-directory" => {
            require_count(arguments, 3)?;
            cmd_index_directory(arguments)
        }
        "--search-root" | "--search-root-ascii-insensitive" => {
            require_count(arguments, 3)?;
            let query = arguments[2]
                .to_str()
                .ok_or_else(|| "DIRECT_QUERY_NOT_UTF8".to_owned())?;
            with_store(Path::new(&arguments[1]), |_root, store, storage| {
                store.verify()?;
                let result = store.search(query, command == "--search-root-ascii-insensitive")?;
                let mut output = io::stdout().lock();
                emit_streaming_search(&mut output, &store.namespace_id(), &result, storage)
            })
        }
        "--list-sources" => {
            require_count(arguments, 2)?;
            with_store(Path::new(&arguments[1]), |_root, store, storage| {
                store.verify_catalog()?;
                emit_sources(store, storage)
            })
        }
        "--verify-root" => {
            require_count(arguments, 2)?;
            with_store(Path::new(&arguments[1]), |_root, store, storage| {
                emit_verification(store, storage)
            })
        }
        "--retire-source" => {
            require_count(arguments, 3)?;
            cmd_retire_source(arguments)
        }
        "--read-revision" => {
            require_count(arguments, 5)?;
            cmd_read_revision(arguments)
        }
        "--repair-root" => {
            require_count(arguments, 2)?;
            cmd_repair_root(arguments)
        }
        "--gc-root" => {
            require_count(arguments, 3)?;
            cmd_gc_root(arguments)
        }
        _ => Err("UNKNOWN_PERSISTENT_COMMAND".to_owned()),
    }
}
