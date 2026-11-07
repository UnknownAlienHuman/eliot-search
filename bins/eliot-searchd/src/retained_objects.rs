//! Shared concrete closed-store verification for GC and archive copies.

use search_control_redb::object_marks::{MarkError, ObjectMarks, PreparedExpectation};
use search_revision_store::{ObjectStoreError, read_object_in};
use search_source_registry::{MAX_SOURCE_OBJECT_BYTES, ObjectKind};
use search_unitizer::{PreparationError, PreparationLimits, inspect_prepared_utf8};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

pub fn check(deadline: Instant, cancel: &AtomicBool) -> Result<(), &'static str> {
    if cancel.load(Ordering::Relaxed) {
        Err("COLLECTION_CANCELLED")
    } else if Instant::now() >= deadline {
        Err("COLLECTION_DEADLINE")
    } else {
        Ok(())
    }
}

pub fn verify(
    control: &Path,
    objects: &Path,
    scratch: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ObjectMarks, &'static str> {
    let mut marks =
        ObjectMarks::build(control, scratch, deadline, cancel).map_err(MarkError::code)?;
    let result = verify_marked(objects, &mut marks, deadline, cancel);
    if let Err(error) = result {
        marks.close(deadline, cancel).map_err(MarkError::code)?;
        return Err(error);
    }
    Ok(marks)
}

fn verify_marked(
    objects: &Path,
    marks: &mut ObjectMarks,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    let prepared = close_prepared_inputs(objects, marks, deadline, cancel)?;
    let mut verified = 0_u64;
    let mut prepared_count = 0_u64;
    let mut previous = None;
    while let Some(reference) = marks
        .next(previous, deadline, cancel)
        .map_err(MarkError::code)?
    {
        check(deadline, cancel)?;
        if reference.kind == ObjectKind::Source {
            let bytes = read_object_in(
                objects,
                reference,
                MAX_SOURCE_OBJECT_BYTES,
                deadline,
                cancel,
            )
            .map_err(ObjectStoreError::code)?;
            drop(bytes);
        } else {
            prepared_count += 1;
        }
        verified = verified.checked_add(1).ok_or("COLLECTION_INPUT_LIMIT")?;
        previous = Some(reference);
    }
    if verified != marks.len() || prepared_count != prepared {
        return Err("COLLECTION_MARK_INCOMPLETE");
    }
    check(deadline, cancel)
}
fn close_prepared_inputs(
    objects: &Path,
    marks: &mut ObjectMarks,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<u64, &'static str> {
    let limits = PreparationLimits {
        max_input_bytes: 8 * 1024 * 1024,
        max_output_bytes: 8 * 1024 * 1024,
        max_lines: 1_000_000,
        max_units: 1_000_000,
        max_encoded_bytes: 64 * 1024 * 1024,
    };
    let mut prepared = 0_u64;
    let mut previous = None;
    while let Some(reference) = marks
        .next(previous, deadline, cancel)
        .map_err(MarkError::code)?
    {
        previous = Some(reference);
        if reference.kind != ObjectKind::Prepared {
            continue;
        }
        let bytes = read_object_in(objects, reference, 64 * 1024 * 1024, deadline, cancel)
            .map_err(ObjectStoreError::code)?;
        let (binding, view) = inspect_prepared_utf8(&bytes, limits, deadline, cancel)
            .map_err(PreparationError::code)?;
        marks
            .include_prepared_input(
                reference,
                PreparedExpectation {
                    source_namespace_id: binding.source_namespace_id,
                    source_id: binding.source_id,
                    input: binding.input,
                    revision_settings: Some((binding.registration_revision, binding.settings)),
                },
                deadline,
                cancel,
            )
            .map_err(MarkError::code)?;
        drop(view);
        prepared = prepared.checked_add(1).ok_or("COLLECTION_INPUT_LIMIT")?;
    }
    // Embedded inputs sort before Prepared marks. The next complete verification
    // pass starts from zero, so newly included Source leaves cannot be skipped.
    Ok(prepared)
}
