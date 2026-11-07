//! Retained-only preparation and stored-unit response for one existing source.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_contracts::SourceId;
use search_control_redb::current_preparation::{
    CurrentPreparation, PreparationControlError, PreparationGap, PreparationOutcome,
    capture_preparation, commit_preparation, ensure_preparation_settings,
};
use search_revision_store::{ObjectStoreError, read_object, store_object};
use search_runtime_owner::{PlainRootGuard, open_plain};
use search_source_registry::{
    CurrentSourceRecord, MAX_PREPARATION_RESPONSE_BYTES, MAX_SOURCE_OBJECT_BYTES, ObjectKind,
    ObjectRef,
};
use search_unitizer::{
    PreparationError, PreparationLimits, PreparedUtf8, decode_prepared_utf8, prepare_utf8,
};
use serde_json::{Value, json};

const fn limits() -> PreparationLimits {
    PreparationLimits {
        max_input_bytes: 8 * 1024 * 1024,
        max_output_bytes: 8 * 1024 * 1024,
        max_lines: 1_000_000,
        max_units: 1_000_000,
        max_encoded_bytes: 64 * 1024 * 1024,
    }
}
fn check(deadline: Instant, cancel: &AtomicBool) -> Result<(), &'static str> {
    if cancel.load(Ordering::Relaxed) {
        Err("PREPARATION_CANCELLED")
    } else if Instant::now() >= deadline {
        Err("PREPARATION_DEADLINE")
    } else {
        Ok(())
    }
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    out
}
fn object_json(reference: ObjectRef) -> Value {
    json!({"version":reference.format_version,"kind":reference.kind.name(),"digest":hex(reference.digest.as_bytes()),"bytes":reference.byte_length})
}
fn base(source: &CurrentSourceRecord, state: &CurrentPreparation) -> Value {
    json!({"operation":"prepare-source","state":"gap","source_namespace_id":source.source_namespace_id.to_string(),"source_id":source.source_id.to_string(),"source_row_revision":source.row_revision,"registration_revision":source.registration_revision,"settings_revision":state.settings_revision,"outcome_revision":state.outcome_revision,"settings":{"utf8_version":state.settings.utf8_version,"line_unitizer_version":state.settings.line_unitizer_version,"preferred_unit_bytes":state.settings.preferred_unit_bytes,"max_unit_bytes":state.settings.max_unit_bytes},"input":object_json(source.object_ref),"artifact":null,"representation_bytes":null,"unit_count":null,"unit":null,"gap":null})
}
const fn gap(error: PreparationError) -> Option<PreparationGap> {
    match error {
        PreparationError::BinaryContent => Some(PreparationGap::BinaryContent),
        PreparationError::InvalidUtf8 => Some(PreparationGap::InvalidUtf8),
        PreparationError::InputTooLarge => Some(PreparationGap::InputTooLarge),
        PreparationError::OutputTooLarge => Some(PreparationGap::OutputTooLarge),
        PreparationError::TooManyLines => Some(PreparationGap::TooManyLines),
        PreparationError::TooManyUnits => Some(PreparationGap::TooManyUnits),
        PreparationError::UnitTooLarge => Some(PreparationGap::UnitTooLarge),
        PreparationError::EncodedTooLarge => Some(PreparationGap::EncodedTooLarge),
        _ => None,
    }
}
fn create_outcome(
    root: &PlainRootGuard,
    source: &CurrentSourceRecord,
    settings: search_source_registry::PreparationSettings,
    budget: PreparationLimits,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PreparationOutcome, &'static str> {
    let input_limit =
        u64::try_from(budget.max_input_bytes).map_err(|_| "PREPARATION_LIMITS_INVALID")?;
    if source.object_ref.byte_length > input_limit {
        return Ok(PreparationOutcome::Gap {
            reason: PreparationGap::InputTooLarge,
        });
    }
    let bytes = read_object(
        root,
        source.object_ref,
        input_limit.min(MAX_SOURCE_OBJECT_BYTES),
        deadline,
        cancel,
    )
    .map_err(ObjectStoreError::code)?;
    match prepare_utf8(source, bytes, settings, budget, deadline, cancel) {
        Ok(encoded) => {
            let maximum = u64::try_from(budget.max_encoded_bytes)
                .map_err(|_| "PREPARATION_LIMITS_INVALID")?;
            let artifact = store_object(
                root,
                ObjectKind::Prepared,
                &encoded,
                maximum,
                deadline,
                cancel,
            )
            .map_err(ObjectStoreError::code)?;
            Ok(PreparationOutcome::Ready { artifact })
        }
        Err(error) => gap(error).map_or_else(
            || Err(error.code()),
            |reason| Ok(PreparationOutcome::Gap { reason }),
        ),
    }
}
fn ready_response(
    source: &CurrentSourceRecord,
    state: &CurrentPreparation,
    artifact: ObjectRef,
    view: &PreparedUtf8<'_>,
    ordinal: usize,
) -> Result<Value, &'static str> {
    let mut response = base(source, state);
    response["artifact"] = object_json(artifact);
    response["representation_bytes"] = json!(view.text().len());
    response["unit_count"] = json!(view.units().len());
    if view.units().is_empty() {
        if !view.text().is_empty() {
            return Err("PREPARATION_ARTIFACT_INVALID");
        }
        if ordinal != 0 {
            return Err("PREPARATION_UNIT_OUT_OF_RANGE");
        }
        response["state"] = json!("empty");
        return Ok(response);
    }
    let unit = view
        .units()
        .get(ordinal)
        .ok_or("PREPARATION_UNIT_OUT_OF_RANGE")?;
    let text = view
        .text()
        .get(unit.source_start..unit.source_end)
        .ok_or("PREPARATION_ARTIFACT_INVALID")?;
    response["state"] = json!("ready");
    response["unit"] = json!({"ordinal":ordinal,"start":unit.source_start,"end":unit.source_end,"line_start":unit.logical_line_start,"line_end":unit.logical_line_end,"starts_at_line_boundary":unit.starts_at_line_boundary,"ends_at_line_boundary":unit.ends_at_line_boundary,"text":text});
    Ok(response)
}
/// Verified current preparation returned to in-process command composition.
pub(super) struct PreparedSource {
    source: CurrentSourceRecord,
    current: CurrentPreparation,
    stored_artifact: Option<(ObjectRef, Vec<u8>)>,
}

/// Reuses or creates the selected source's verified preparation without CLI serialization.
pub(super) fn prepare(
    root: &PlainRootGuard,
    id: SourceId,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PreparedSource, &'static str> {
    check(deadline, cancel)?;
    let control = root.control_path();
    ensure_preparation_settings(&control, id, deadline, cancel)
        .map_err(PreparationControlError::code)?;
    let capture = capture_preparation(&control, id, deadline, cancel)
        .map_err(PreparationControlError::code)?;
    let source = capture.source.clone();
    let settings = capture.settings;
    let budget = limits();
    let encoded_limit =
        u64::try_from(budget.max_encoded_bytes).map_err(|_| "PREPARATION_LIMITS_INVALID")?;
    let reusable = capture
        .expected_outcome
        .as_ref()
        .filter(|o| o.matches_input(&source, settings))
        .and_then(|o| match o.result {
            PreparationOutcome::Ready { artifact } => Some(artifact),
            PreparationOutcome::Gap { .. } => None,
        });
    let result = if let Some(artifact) = reusable {
        PreparationOutcome::Ready { artifact }
    } else {
        create_outcome(root, &source, settings, budget, deadline, cancel)?
    };
    match result {
        PreparationOutcome::Ready { artifact } => {
            let stored = read_object(root, artifact, encoded_limit, deadline, cancel)
                .map_err(ObjectStoreError::code)?;
            let _view = decode_prepared_utf8(&stored, &source, &settings, budget, deadline, cancel)
                .map_err(PreparationError::code)?;
            let current = commit_preparation(&control, &capture, result, deadline, cancel)
                .map_err(PreparationControlError::code)?;
            Ok(PreparedSource {
                source,
                current,
                stored_artifact: Some((artifact, stored)),
            })
        }
        PreparationOutcome::Gap { reason } => {
            let current = commit_preparation(&control, &capture, result, deadline, cancel)
                .map_err(PreparationControlError::code)?;
            match current.outcome.as_ref().map(|o| o.result) {
                Some(PreparationOutcome::Ready { artifact }) => {
                    let stored = read_object(root, artifact, encoded_limit, deadline, cancel)
                        .map_err(ObjectStoreError::code)?;
                    let _view =
                        decode_prepared_utf8(&stored, &source, &settings, budget, deadline, cancel)
                            .map_err(PreparationError::code)?;
                    Ok(PreparedSource {
                        source,
                        current,
                        stored_artifact: Some((artifact, stored)),
                    })
                }
                Some(PreparationOutcome::Gap { reason: actual }) if actual == reason => {
                    Ok(PreparedSource {
                        source,
                        current,
                        stored_artifact: None,
                    })
                }
                _ => Err("PREPARATION_CONTROL_CORRUPT"),
            }
        }
    }
}

fn response(
    prepared: &PreparedSource,
    ordinal: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Value, &'static str> {
    let state = &prepared.current;
    match state.outcome.as_ref().map(|outcome| outcome.result) {
        Some(PreparationOutcome::Ready { artifact }) => {
            let (stored_ref, stored) = prepared
                .stored_artifact
                .as_ref()
                .filter(|(stored_ref, _)| *stored_ref == artifact)
                .ok_or("PREPARATION_CONTROL_CORRUPT")?;
            let view = decode_prepared_utf8(
                stored,
                &prepared.source,
                &state.settings,
                limits(),
                deadline,
                cancel,
            )
            .map_err(PreparationError::code)?;
            ready_response(&prepared.source, state, *stored_ref, &view, ordinal)
        }
        Some(PreparationOutcome::Gap { reason }) => {
            let mut value = base(&prepared.source, state);
            value["gap"] = json!(reason.code());
            Ok(value)
        }
        None => Err("PREPARATION_CONTROL_CORRUPT"),
    }
}

pub fn run(
    path: &Path,
    id: SourceId,
    ordinal: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> ExitCode {
    let guard = match open_plain(path, deadline, cancel) {
        Ok(guard) => guard,
        Err(e) => return emit_error(e.code()),
    };
    let prepared = match prepare(&guard, id, deadline, cancel) {
        Ok(prepared) => prepared,
        Err(code) => return emit_error(code),
    };
    let response = match response(&prepared, ordinal, deadline, cancel) {
        Ok(response) => response,
        Err(code) => return emit_error(code),
    };
    let bytes = match serde_json::to_vec(&response) {
        Ok(bytes) if bytes.len() < MAX_PREPARATION_RESPONSE_BYTES => bytes,
        _ => return emit_error("PREPARATION_RESPONSE_TOO_LARGE"),
    };
    if let Err(code) = check(deadline, cancel) {
        return emit_error(code);
    }
    let mut stdout = std::io::stdout().lock();
    if stdout
        .write_all(&bytes)
        .and_then(|()| stdout.write_all(b"\n"))
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return emit_error("OUTPUT_WRITE_ERROR");
    }
    drop(guard);
    ExitCode::SUCCESS
}
fn emit_error(code: &str) -> ExitCode {
    eprintln!("{{\"error\":\"{code}\"}}");
    ExitCode::from(2)
}
