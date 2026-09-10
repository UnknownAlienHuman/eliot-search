//! Local owner-fenced observation-root commands for the primary daemon.
//!
//! Every command holds the single live [`DataRootGuard`] for its whole
//! lifetime. A running daemon that already holds the data-root lock denies a
//! second acquirer with `DATA_ROOT_ALREADY_OWNED`; the CLI never opens
//! another owner for a live service. Watcher notifications stay hints inside
//! [`SourceRootCatalog`]; only an explicit refresh plus a successful
//! multi-root sync re-proves workspace truth. Missing or replaced roots are
//! explicit gaps that fail closed instead of reading as empty.

use std::fmt::Write as _;
use std::path::Path;

use crate::development::DataRootGuard;
use crate::direct_store::DirectStore;
use crate::directory_manifest::sync_directory;
use crate::source_roots::SourceRootCatalog;

pub fn run(arguments: &[String]) -> Result<(), String> {
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
    emit_catalog_state(owner.source_roots())
}

fn emit_catalog_state(catalog: &SourceRootCatalog) -> Result<(), String> {
    for view in catalog.views().map_err(|error| error.code().to_owned())? {
        println!(
            "{{\"event\":\"source_root\",\"position\":{},\"path\":\"{}\",\"state\":\"{}\"}}",
            view.index, escape_json(&view.path), view.state.code(),
        );
    }
    for gap in catalog.observation_gaps() {
        println!(
            "{{\"event\":\"source_gap\",\"position\":{},\"reason\":\"{}\",\"state\":\"{}\"}}",
            gap.position, gap.reason.code(), gap.state.code(),
        );
    }
    let truth = catalog.current_workspace_truth();
    let cursor = catalog.reconciliation_cursor();
    // Index truth is always unavailable in this shell (no qualified
    // server/client/artifact/profile set), so the final claim stays false
    // while remaining derived instead of hard-coded.
    let proven = crate::provider_composition::evaluate_current_workspace_proven(
        truth.configured,
        truth.gap_count,
        truth.workspace_current,
        false,
    );
    let reason = crate::provider_composition::current_workspace_proven_reason(
        truth.configured,
        truth.gap_count,
        truth.workspace_current,
        false,
    );
    println!(
        concat!(
            "{{\"event\":\"source_roots_complete\",\"configured\":{},",
            "\"available\":{},\"unavailable\":{},\"gaps\":{},",
            "\"reconciliation_generation\":{},\"watcher_pending\":{},",
            "\"watcher_overflowed\":{},\"proven_reason\":\"{}\",",
            "\"current_workspace_proven\":{}}}",
        ),
        truth.configured,
        truth.available,
        truth.unavailable,
        truth.gap_count,
        truth.reconciliation_generation,
        cursor.pending_hints,
        cursor.overflowed,
        reason,
        proven,
    );
    Ok(())
}

fn sync_registered(owner: &mut DataRootGuard) -> Result<(), String> {
    owner.source_roots_mut().refresh();
    let truth = owner.source_roots().current_workspace_truth();
    if truth.configured == 0 {
        return Err("SOURCE_ROOTS_EMPTY".to_owned());
    }
    if truth.gap_count != 0 || truth.unavailable != 0 {
        // A missing/inaccessible root is not an empty inventory. Surface its
        // explicit gaps, then refuse to retire retained sources or begin a
        // partially preflighted multi-root sync.
        for gap in owner.source_roots().observation_gaps() {
            println!(
                "{{\"event\":\"source_gap\",\"position\":{},\"reason\":\"{}\",\"state\":\"{}\"}}",
                gap.position, gap.reason.code(), gap.state.code(),
            );
        }
        return Err("SOURCE_ROOTS_UNAVAILABLE".to_owned());
    }
    let paths = owner.source_roots().available_paths().into_iter()
        .map(|(index, path)| (index, path.to_path_buf()))
        .collect::<Vec<_>>();
    let data_root = owner.canonical_root().to_path_buf();
    let mut store = DirectStore::open(&data_root)?;
    let mut completed = 0_usize;
    for (index, path) in &paths {
        match sync_directory(&mut store, &data_root, path) {
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
    // All roots reconciled with zero preflight gaps and zero mid-sync
    // failures: cover the current generation so workspace truth can advance.
    // Index truth stays unavailable in this shell, so the final proven claim
    // remains false while derived. An interrupted sync never reaches here, so
    // its effects stay explicitly incomplete with no current claim.
    owner.source_roots_mut().mark_reconciled_synced();
    let truth = owner.source_roots().current_workspace_truth();
    let proven = crate::provider_composition::evaluate_current_workspace_proven(
        truth.configured,
        truth.gap_count,
        truth.workspace_current,
        false,
    );
    let reason = crate::provider_composition::current_workspace_proven_reason(
        truth.configured,
        truth.gap_count,
        truth.workspace_current,
        false,
    );
    println!(
        concat!(
            "{{\"event\":\"source_roots_synced\",\"completed_roots\":{},",
            "\"complete\":true,\"gaps\":{},\"reconciliation_generation\":{},",
            "\"proven_reason\":\"{}\",\"current_workspace_proven\":{},",
            "\"qdrant_available\":false}}",
        ),
        completed,
        truth.gap_count,
        truth.reconciliation_generation,
        reason,
        proven,
    );
    Ok(())
}

/// Escapes JSON string contents, including Windows separators and control bytes.
pub fn escape_json(value: &str) -> String {
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
