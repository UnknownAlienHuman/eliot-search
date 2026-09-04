//! One-shot fail-closed DIRECT maintenance commands.

use std::env;
use std::path::Path;
use std::process::ExitCode;

use crate::development::DataRootGuard;
use crate::direct_store::DirectStore;
use crate::maintenance::repair_control_log;
use crate::maintenance_guard::guarded_collect_orphan_revisions;

/// Intercepts repair and GC commands before the general dispatcher.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let command = arguments.first().and_then(|value| value.to_str())?;
    let result = match (command, arguments.as_slice()) {
        ("--repair-root", [_, root]) => run_repair(Path::new(root)),
        ("--gc-root", [_, root, mode]) => {
            let apply = match mode.to_str() {
                Some("--dry-run") => false,
                Some("--apply") => true,
                _ => return Some(error_exit("USAGE_ERROR")),
            };
            run_gc(Path::new(root), apply)
        }
        ("--repair-root" | "--gc-root", _) => Err("USAGE_ERROR".to_owned()),
        _ => return None,
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => error_exit(&error),
    })
}

fn run_repair(root: &Path) -> Result<(), String> {
    let guard = DataRootGuard::acquire(root)?;
    let repair = repair_control_log(guard.canonical_root())?;
    let store = DirectStore::open(guard.canonical_root())?;
    store.verify()?;
    println!(
        concat!(
            "{{\"event\":\"direct_store_repair_complete\",",
            "\"namespace_id\":\"{}\",\"repaired\":{},",
            "\"removed_bytes\":{},\"retained_events\":{},",
            "\"last_sequence\":{},\"last_digest\":\"{}\"}}"
        ),
        store.namespace_id(),
        repair.repaired,
        repair.removed_bytes,
        repair.retained_events,
        repair.last_sequence,
        repair.last_digest,
    );
    Ok(())
}

fn run_gc(root: &Path, apply: bool) -> Result<(), String> {
    let guard = DataRootGuard::acquire(root)?;
    let store = DirectStore::open(guard.canonical_root())?;
    store.verify()?;
    let result = guarded_collect_orphan_revisions(guard.canonical_root(), apply)?;
    println!(
        concat!(
            "{{\"event\":\"direct_store_gc_complete\",",
            "\"namespace_id\":\"{}\",\"applied\":{},",
            "\"referenced_revisions\":{},\"scanned_objects\":{},",
            "\"orphan_objects\":{},\"orphan_bytes\":{},",
            "\"deleted_objects\":{},\"deleted_bytes\":{},",
            "\"unexpected_objects\":{}}}"
        ),
        store.namespace_id(),
        result.applied,
        result.referenced_revisions,
        result.scanned_objects,
        result.orphan_objects,
        result.orphan_bytes,
        result.deleted_objects,
        result.deleted_bytes,
        result.unexpected_objects,
    );
    Ok(())
}

fn error_exit(error: &str) -> ExitCode {
    eprintln!("{{\"error\":{}}}", json_string(error));
    ExitCode::from(2)
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
