//! Explicit preparation/reconstruction from a retained revision, never a live path.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use crate::{development::DataRootGuard, direct_store::DirectStore, direct_preparation, sha256};
use crate::service_output::{json_string, write_line};

/// Shared argument validation runs before a service command arms its mutation fence.
pub(crate) fn validate_revision(revision: &str) -> Result<(), String> {
    if sha256::decode_digest(revision).is_none() {
        Err("DIRECT_REVISION_ID_INVALID".to_owned())
    } else {
        Ok(())
    }
}

/// A stored record may describe an unsupported-input gap, not a searchable layout.
/// This response acknowledges storage only; it does not grant access or completeness.
pub(crate) fn emit_prepared(
    writer: &mut impl Write,
    revision: &str,
    invalidated: (usize, usize),
) -> Result<(), String> {
    write_line(writer, &format!(
        concat!(
            "{{\"event\":\"revision_preparation_stored\",\"revision_id\":{},",
            "\"profile_sha256\":{},\"invalidated_continuations\":{},",
            "\"invalidated_handles\":{}}}"
        ),
        json_string(revision), json_string(&sha256::hex(&direct_preparation::profile_digest())),
        invalidated.0, invalidated.1,
    ))
}

pub(crate) fn maybe_run() -> Option<ExitCode> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().and_then(|arg| arg.to_str()) != Some("--prepare-revision") { return None; }
    let result = (|| -> Result<(), String> {
        if args.len() != 3 { return Err("USAGE_ERROR".to_owned()); }
        let revision = args[2].to_str().ok_or_else(|| "DIRECT_REVISION_ID_INVALID".to_owned())?;
        validate_revision(revision)?;
        let owner = DataRootGuard::acquire(Path::new(&args[1]))?;
        crate::catalog_presence::require_existing(owner.canonical_root())?;
        let mut store = DirectStore::open(owner.canonical_root())?;
        store.prepare_revision(revision)?;
        emit_prepared(&mut std::io::stdout().lock(), revision, (0, 0))
    })();
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            ExitCode::from(2)
        }
    })
}
