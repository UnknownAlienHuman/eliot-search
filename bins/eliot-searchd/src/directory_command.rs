//! One-shot directory inventory and reconciliation commands.

use std::env;
use std::path::Path;
use std::process::ExitCode;

use crate::development::DataRootGuard;
use crate::direct_store::DirectStore;
use crate::directory_manifest::{sync_directory, verify_directory_manifests};

/// Intercepts directory-manifest commands before the general dispatcher.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let command = arguments.first().and_then(|value| value.to_str())?;
    let result = match (command, arguments.as_slice()) {
        ("--sync-directory", [_, root, directory]) => {
            run_sync(Path::new(root), Path::new(directory))
        }
        ("--verify-directory-manifests", [_, root]) => {
            run_verify(Path::new(root))
        }
        ("--sync-directory" | "--verify-directory-manifests", _) => {
            Err("USAGE_ERROR".to_owned())
        }
        _ => return None,
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            ExitCode::from(2)
        }
    })
}

fn run_sync(root: &Path, directory: &Path) -> Result<(), String> {
    let guard = DataRootGuard::acquire(root)?;
    let mut store = DirectStore::open(guard.canonical_root())?;
    store.verify()?;
    let result = sync_directory(
        &mut store,
        guard.canonical_root(),
        directory,
    )?;
    store.verify()?;
    let manifests = verify_directory_manifests(
        guard.canonical_root(),
        &store.namespace_id(),
    )?;
    println!(
        concat!(
            "{{\"event\":\"directory_sync_complete\",",
            "\"namespace_id\":\"{}\",\"directory_digest\":\"{}\",",
            "\"previous_generation\":{},\"generation\":{},",
            "\"previous_sources\":{},\"indexed_sources\":{},",
            "\"changed_sources\":{},\"missing_sources\":{},",
            "\"retired_sources\":{},\"moved_or_rebound_sources\":{},",
            "\"manifest_digest\":\"{}\",\"manifest_files\":{},",
            "\"source_backed\":true,\"encrypted_at_rest\":false}}"
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
    );
    Ok(())
}

fn run_verify(root: &Path) -> Result<(), String> {
    let guard = DataRootGuard::acquire(root)?;
    let store = DirectStore::open(guard.canonical_root())?;
    let source_verification = store.verify()?;
    let verification = verify_directory_manifests(
        guard.canonical_root(),
        &store.namespace_id(),
    )?;
    println!(
        concat!(
            "{{\"event\":\"directory_manifests_verified\",",
            "\"namespace_id\":\"{}\",\"manifest_files\":{},",
            "\"directories\":{},\"current_entries\":{},",
            "\"highest_generation\":{},\"registered_sources\":{},",
            "\"active_sources\":{},\"source_backed\":true,",
            "\"encrypted_at_rest\":false}}"
        ),
        store.namespace_id(),
        verification.manifest_files,
        verification.directories,
        verification.current_entries,
        verification.highest_generation,
        source_verification.registered_sources,
        source_verification.active_sources,
    );
    Ok(())
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(&mut output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}
