//! Explicit preparation/reconstruction from retained revisions, never live paths.
//!
//! Canonical preparation binding (T16): every stored record binds its source
//! revision, representation identity, materializer/unitizer profile revisions
//! and exact digest algorithms with real provenance. The legacy `emit_prepared`
//! and `emit_batch` contracts are preserved for existing service callers; the
//! canonical `--prepare-*` CLI path below emits the extended binding.

use crate::direct_preparation::{
    CANONICAL_MATERIALIZER_REVISION, CANONICAL_UNITIZER_REVISION, CONTENT_DIGEST_ALGORITHM,
    MANIFEST_DIGEST_ALGORITHM, REPRESENTATION_DIGEST_ALGORITHM,
};
use crate::direct_store::{PreparationBatch, PreparationCursor};
use crate::service_output::{json_string, write_line};
use crate::{development::DataRootGuard, direct_preparation, direct_store::DirectStore, sha256};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

/// Shared argument validation runs before a service command arms its mutation fence.
pub fn validate_revision(revision: &str) -> Result<(), String> {
    if sha256::decode_digest(revision).is_none() {
        Err("DIRECT_REVISION_ID_INVALID".to_owned())
    } else {
        Ok(())
    }
}

/// A stored record may describe an unsupported-input gap, not a searchable layout.
/// This response acknowledges storage only; it does not grant access or completeness.
/// Legacy contract preserved for existing service callers (`public_runtime_service`).
pub fn emit_prepared(
    writer: &mut impl Write,
    revision: &str,
    invalidated: (usize, usize),
    gap: Option<&'static str>,
) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"revision_preparation_stored\",\"revision_id\":{},",
                "\"profile_sha256\":{},\"invalidated_continuations\":{},",
                "\"invalidated_handles\":{},\"layout_available\":{},\"preparation_gap\":{}}}"
            ),
            json_string(revision),
            json_string(&sha256::hex(&direct_preparation::profile_digest())),
            invalidated.0,
            invalidated.1,
            gap.is_none(),
            gap.map_or_else(|| "null".to_owned(), json_string),
        ),
    )
}

/// Canonical stored record with real provenance: representation identity,
/// materializer/unitizer profile digests and revisions and exact digest
/// algorithms. No receipt is substituted.
pub fn emit_prepared_canonical(
    writer: &mut impl Write,
    revision: &str,
    invalidated: (usize, usize),
    receipt: &direct_preparation::CanonicalPreparationReceipt,
) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"revision_preparation_stored\",\"revision_id\":{},",
                "\"profile_sha256\":{},\"invalidated_continuations\":{},",
                "\"invalidated_handles\":{},\"layout_available\":{},\"preparation_gap\":{},",
                "\"representation_id\":{},\"materializer_profile_digest\":{},",
                "\"unitizer_profile_digest\":{},\"materializer_profile_revision\":{},",
                "\"unitizer_profile_revision\":{},\"content_digest_algorithm\":{},",
                "\"representation_digest_algorithm\":{},\"manifest_digest_algorithm\":{}}}"
            ),
            json_string(revision),
            json_string(&sha256::hex(&direct_preparation::profile_digest())),
            invalidated.0,
            invalidated.1,
            receipt.gap.is_none(),
            receipt.gap.map_or_else(|| "null".to_owned(), json_string),
            json_string(&receipt.representation_hex()),
            json_string(&sha256::hex(&receipt.materializer_digest)),
            json_string(&sha256::hex(&receipt.unitizer_digest)),
            CANONICAL_MATERIALIZER_REVISION,
            CANONICAL_UNITIZER_REVISION,
            json_string(digest_algorithm_name(CONTENT_DIGEST_ALGORITHM)),
            json_string(digest_algorithm_name(REPRESENTATION_DIGEST_ALGORITHM)),
            json_string(digest_algorithm_name(MANIFEST_DIGEST_ALGORITHM)),
        ),
    )
}

/// Exact digest algorithm wire name bound into canonical preparation objects.
/// Unknown tags never reach here: storage verification rejects them first.
const fn digest_algorithm_name(tag: u8) -> &'static str {
    match tag {
        1 => "blake3_256",
        2 => "sha256",
        _ => "unknown",
    }
}

/// A finite page summary, not an assertion that a caller processed every earlier page.
/// One event-first frame works unchanged through the existing proxy response boundary.
/// Legacy contract preserved; canonical manifest bindings are appended additively.
pub fn emit_batch(
    writer: &mut impl Write,
    batch: &PreparationBatch,
    invalidated: (usize, usize),
) -> Result<(), String> {
    let gaps = batch
        .gaps
        .iter()
        .map(|(revision, reason)| {
            format!(
                "{{\"revision_id\":{},\"reason\":{}}}",
                json_string(revision),
                json_string(reason),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let manifests = batch
        .manifests()
        .iter()
        .map(|(revision_id, representation_id, gap)| {
            format!(
                concat!(
                    "{{\"revision_id\":{},\"representation_id\":{},\"preparation_gap\":{},",
                    "\"materializer_profile_revision\":{},\"unitizer_profile_revision\":{},",
                    "\"content_digest_algorithm\":\"sha256\",",
                    "\"representation_digest_algorithm\":\"blake3_256\",",
                    "\"manifest_digest_algorithm\":\"sha256\"}}"
                ),
                json_string(revision_id),
                json_string(representation_id),
                gap.map_or_else(|| "null".to_owned(), json_string),
                CANONICAL_MATERIALIZER_REVISION,
                CANONICAL_UNITIZER_REVISION,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let materializer_hex = direct_preparation::canonical_materializer_digest()
        .map_or_else(|_| "invalid".to_owned(), |bytes| sha256::hex(&bytes));
    let unitizer_hex = direct_preparation::canonical_unitizer_digest()
        .map_or_else(|_| "invalid".to_owned(), |bytes| sha256::hex(&bytes));
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"preparation_batch_complete\",\"profile_sha256\":{},",
                "\"stored_records\":{},\"available_layouts\":{},\"source_bytes\":{},",
                "\"preparation_gap_count\":{},\"preparation_gaps\":[{}],",
                "\"exhausted\":{},\"next_cursor\":{},\"diagnostic_internal_identifiers\":true,",
                "\"invalidated_continuations\":{},\"invalidated_handles\":{},",
                "\"materializer_profile_digest\":{},\"unitizer_profile_digest\":{},",
                "\"materializer_profile_revision\":{},\"unitizer_profile_revision\":{},",
                "\"content_digest_algorithm\":\"sha256\",",
                "\"representation_digest_algorithm\":\"blake3_256\",",
                "\"manifest_digest_algorithm\":\"sha256\",",
                "\"preparation_manifests\":[{}]}}"
            ),
            json_string(&sha256::hex(&direct_preparation::profile_digest())),
            batch.stored,
            batch.layouts,
            batch.source_bytes,
            batch.gaps.len(),
            gaps,
            batch.next_cursor.is_none(),
            batch
                .next_cursor
                .as_deref()
                .map_or_else(|| "null".to_owned(), json_string),
            invalidated.0,
            invalidated.1,
            json_string(&materializer_hex),
            json_string(&unitizer_hex),
            CANONICAL_MATERIALIZER_REVISION,
            CANONICAL_UNITIZER_REVISION,
            manifests,
        ),
    )
}

pub fn maybe_run() -> Option<ExitCode> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let command = args.first()?.to_str()?;
    if !matches!(command, "--prepare-revision" | "--prepare-root") {
        return None;
    }
    let result = (|| -> Result<(), String> {
        let (revision, cursor) = match (command, args.as_slice()) {
            ("--prepare-revision", [_, _, value]) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| "DIRECT_REVISION_ID_INVALID".to_owned())?;
                validate_revision(value)?;
                (Some(value), None)
            }
            ("--prepare-root", [_, _]) => (None, None),
            ("--prepare-root", [_, _, value]) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| "DIRECT_PREPARATION_CURSOR_INVALID".to_owned())?;
                (None, Some(PreparationCursor::parse(value)?))
            }
            _ => return Err("USAGE_ERROR".to_owned()),
        };
        let owner = DataRootGuard::acquire(Path::new(&args[1]))?;
        crate::catalog_presence::require_existing(owner.canonical_root())?;
        let store = DirectStore::open(owner.canonical_root())?;
        let mut output = std::io::stdout().lock();
        if let Some(revision) = revision {
            let receipt = store.prepare_revision_canonical(revision)?;
            emit_prepared_canonical(&mut output, revision, (0, 0), &receipt)
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
