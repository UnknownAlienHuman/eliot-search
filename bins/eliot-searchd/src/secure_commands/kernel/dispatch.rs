use std::ffi::OsString;
#[cfg(test)]
use std::{io, path::Path};
#[cfg(test)]
use crate::service_output::{emit_streaming_search, json_string};
#[cfg(test)]
use super::commands::cmd_read_revision;
#[cfg(test)]
use super::output::{emit_sources, emit_verification, write_stdout};
#[cfg(test)]
use super::store::with_store;
#[cfg(test)]
use super::support::require_count;

pub(super) fn dispatch(command: &str, _arguments: &[OsString]) -> Result<(), String> {
    #[cfg(test)]
    let arguments = _arguments;
    match command {
        #[cfg(test)]
        "--health-data-root" => {
            require_count(arguments, 2)?;
            with_store(Path::new(&arguments[1]), |root, store, storage| {
                let verification = store.verify()?;
                write_stdout(&format!(
                    concat!(
                        "{{\"event\":\"health\",\"namespace_id\":{},",
                        "\"registered_sources\":{},\"active_sources\":{},",
                        "\"verified_revisions\":{},\"storage_security\":{},",
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
        "--index-file" | "--index-directory" | "--retire-source" | "--repair-root" | "--gc-root" => {
            Err(super::support::LEGACY_EFFECTS_UNAVAILABLE.to_owned())
        }
        #[cfg(test)]
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
        #[cfg(test)]
        "--list-sources" => {
            require_count(arguments, 2)?;
            with_store(Path::new(&arguments[1]), |_root, store, storage| {
                store.verify()?;
                emit_sources(&mut io::stdout().lock(), store, storage)
            })
        }
        #[cfg(test)]
        "--verify-root" => {
            require_count(arguments, 2)?;
            with_store(Path::new(&arguments[1]), |_root, store, storage| {
                emit_verification(&mut io::stdout().lock(), store, storage)
            })
        }
        #[cfg(test)]
        "--read-revision" => {
            require_count(arguments, 5)?;
            cmd_read_revision(arguments)
        }
        _ => Err("UNKNOWN_PERSISTENT_COMMAND".to_owned()),
    }
}
