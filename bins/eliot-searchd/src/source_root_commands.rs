//! Local owner-fenced observation-root commands for the primary daemon.

use std::fmt::Write as _;
use std::path::Path;

use crate::development::DataRootGuard;
use crate::direct_store::DirectStore;
use crate::directory_manifest::sync_directory;

pub(crate) fn run(arguments: &[String]) -> Result<(), String> {
    let command = arguments.first().map(String::as_str).unwrap_or_default();
    let expected = match command {
        "--source-roots" | "--sync-source-roots" => 2,
        "--register-source-root" | "--unregister-source-root" => 3,
        _ => return Err("SOURCE_ROOT_USAGE_ERROR".to_owned()),
    };
    if arguments.len() != expected {
        return Err("SOURCE_ROOT_USAGE_ERROR".to_owned());
    }
    let mut owner = DataRootGuard::acquire(Path::new(&arguments[1]))?;
    match command {
        "--register-source-root" => {
            let view = owner.source_roots_mut().add(Path::new(&arguments[2]))
                .map_err(|error| error.code().to_owned())?;
            println!(
                "{{\"event\":\"source_root_registered\",\"path\":\"{}\",\"persisted\":true,\"access_granted\":false}}",
                escape_json(&view.path),
            );
        }
        "--unregister-source-root" => {
            let path = owner.source_roots_mut().remove(Path::new(&arguments[2]))
                .map_err(|error| error.code().to_owned())?;
            println!(
                "{{\"event\":\"source_root_unregistered\",\"path\":\"{}\",\"persisted\":true,\"retained_revisions_revoked\":false}}",
                escape_json(&path),
            );
        }
        "--sync-source-roots" => return sync_registered(&mut owner),
        _ => {}
    }
    let catalog = owner.source_roots();
    for view in catalog.views().map_err(|error| error.code().to_owned())? {
        println!(
            "{{\"event\":\"source_root\",\"position\":{},\"path\":\"{}\",\"state\":\"{}\"}}",
            view.index, escape_json(&view.path), view.state.code(),
        );
    }
    println!(
        "{{\"event\":\"source_roots_complete\",\"configured\":{},\"available\":{},\"unavailable\":{},\"current_workspace_proven\":false}}",
        catalog.configured_count(), catalog.available_count(), catalog.unavailable_count(),
    );
    Ok(())
}

fn sync_registered(owner: &mut DataRootGuard) -> Result<(), String> {
    owner.source_roots_mut().refresh();
    let catalog = owner.source_roots();
    if catalog.configured_count() == 0 {
        return Err("SOURCE_ROOTS_EMPTY".to_owned());
    }
    if catalog.unavailable_count() != 0 {
        // A missing/inaccessible root is not an empty inventory. Do not retire
        // its retained sources or begin a partially preflighted multi-root sync.
        return Err("SOURCE_ROOTS_UNAVAILABLE".to_owned());
    }
    let paths = catalog.available_paths().into_iter()
        .map(|(index, path)| (index, path.to_path_buf()))
        .collect::<Vec<_>>();
    let data_root = owner.canonical_root();
    let mut store = DirectStore::open(data_root)?;
    let mut completed = 0_usize;
    for (index, path) in &paths {
        match sync_directory(&mut store, data_root, path) {
            Ok(result) => {
                completed += 1;
                println!(
                    concat!(
                        "{{\"event\":\"source_root_synced\",\"position\":{},",
                        "\"generation\":{},\"indexed_sources\":{},",
                        "\"changed_sources\":{},\"retired_sources\":{},",
                        "\"manifest_digest\":\"{}\"}}"
                    ),
                    index, result.generation, result.indexed_sources,
                    result.changed_sources, result.retired_sources, result.manifest_digest,
                );
            }
            Err(error) => {
                // A directory sync may have committed individual revisions even
                // before its own final manifest. Never report a clean rollback.
                let reason = error.split(':').next().unwrap_or("SOURCE_ROOT_SYNC_FAILED");
                println!(
                    "{{\"event\":\"source_roots_sync_failed\",\"position\":{},\"completed_roots\":{},\"effects_may_have_committed\":true,\"complete\":false,\"reason\":\"{}\"}}",
                    index, completed, escape_json(reason),
                );
                return Err("SOURCE_ROOT_SYNC_INCOMPLETE".to_owned());
            }
        }
    }
    println!(
        "{{\"event\":\"source_roots_synced\",\"completed_roots\":{},\"complete\":true,\"current_workspace_proven\":false,\"qdrant_available\":false}}",
        completed,
    );
    Ok(())
}

/// Escapes JSON string contents, including Windows separators and control bytes.
pub(crate) fn escape_json(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character < ' ' => {
                write!(&mut escaped, "\\u{:04x}", u32::from(character))
                    .expect("writing to String cannot fail");
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escaping_preserves_windows_paths_and_unicode() {
        assert_eq!(escape_json("C:\\Корпус\\\"notes\"\n\0"), "C:\\\\Корпус\\\\\\\"notes\\\"\\n\\u0000");
    }

    #[test]
    fn incomplete_arguments_are_rejected_without_opening_a_root() {
        assert_eq!(run(&["--source-roots".to_owned()]), Err("SOURCE_ROOT_USAGE_ERROR".to_owned()));
        assert_eq!(run(&["--register-source-root".to_owned(), "not-opened".to_owned()]), Err("SOURCE_ROOT_USAGE_ERROR".to_owned()));
    }
}
