//! Explicit preparation/reconstruction from retained revisions, never live paths.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use crate::{development::DataRootGuard, direct_store::DirectStore, direct_preparation, sha256};
use crate::direct_store::{PreparationBatch, PreparationCursor};
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
    gap: Option<&'static str>,
) -> Result<(), String> {
    write_line(writer, &format!(
        concat!(
            "{{\"event\":\"revision_preparation_stored\",\"revision_id\":{},",
            "\"profile_sha256\":{},\"invalidated_continuations\":{},",
            "\"invalidated_handles\":{},\"layout_available\":{},\"preparation_gap\":{}}}"
        ),
        json_string(revision), json_string(&sha256::hex(&direct_preparation::profile_digest())),
        invalidated.0, invalidated.1, gap.is_none(),
        gap.map_or_else(|| "null".to_owned(), json_string),
    ))
}

/// A finite page summary, not an assertion that a caller processed every earlier page.
/// One event-first frame works unchanged through the existing proxy response boundary.
pub(crate) fn emit_batch(
    writer: &mut impl Write, batch: &PreparationBatch, invalidated: (usize, usize),
) -> Result<(), String> {
    let gaps = batch.gaps.iter().map(|(revision, reason)| format!(
        "{{\"revision_id\":{},\"reason\":{}}}", json_string(revision), json_string(reason),
    )).collect::<Vec<_>>().join(",");
    write_line(writer, &format!(
        concat!(
            "{{\"event\":\"preparation_batch_complete\",\"profile_sha256\":{},",
            "\"stored_records\":{},\"available_layouts\":{},\"source_bytes\":{},",
            "\"preparation_gap_count\":{},\"preparation_gaps\":[{}],",
            "\"exhausted\":{},\"next_cursor\":{},\"diagnostic_internal_identifiers\":true,",
            "\"invalidated_continuations\":{},\"invalidated_handles\":{}}}"
        ),
        json_string(&sha256::hex(&direct_preparation::profile_digest())),
        batch.stored, batch.layouts, batch.source_bytes, batch.gaps.len(), gaps,
        batch.next_cursor.is_none(),
        batch.next_cursor.as_deref().map_or_else(|| "null".to_owned(), json_string),
        invalidated.0, invalidated.1,
    ))
}

pub(crate) fn maybe_run() -> Option<ExitCode> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let command = args.first()?.to_str()?;
    if !matches!(command, "--prepare-revision" | "--prepare-root") { return None; }
    let result = (|| -> Result<(), String> {
        let (revision, cursor) = match (command, args.as_slice()) {
            ("--prepare-revision", [_, _, value]) => {
                let value = value.to_str().ok_or_else(|| "DIRECT_REVISION_ID_INVALID".to_owned())?;
                validate_revision(value)?;
                (Some(value), None)
            }
            ("--prepare-root", [_, _]) => (None, None),
            ("--prepare-root", [_, _, value]) => {
                let value = value.to_str().ok_or_else(|| "DIRECT_PREPARATION_CURSOR_INVALID".to_owned())?;
                (None, Some(PreparationCursor::parse(value)?))
            }
            _ => return Err("USAGE_ERROR".to_owned()),
        };
        let owner = DataRootGuard::acquire(Path::new(&args[1]))?;
        crate::catalog_presence::require_existing(owner.canonical_root())?;
        let mut store = DirectStore::open(owner.canonical_root())?;
        let mut output = std::io::stdout().lock();
        if let Some(revision) = revision {
            let gap = store.prepare_revision(revision)?;
            emit_prepared(&mut output, revision, (0, 0), gap)
        } else {
            let batch = store.prepare_root(cursor.as_ref())?;
            emit_batch(&mut output, &batch, (0, 0))
        }
    })();
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            ExitCode::from(2)
        }
    })
}
