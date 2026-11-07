//! One stopped-root complete mark, verified objects, then one streaming local sweep.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_control_redb::object_marks::{MarkError, ObjectMarks, PreparedExpectation};
use search_revision_store::{
    ObjectStoreError, collection_candidate, read_object, remove_unreferenced_object,
};
use search_runtime_owner::{PlainRootError, PlainRootGuard, open_plain_with_recovery};
use search_source_registry::{MAX_SOURCE_OBJECT_BYTES, ObjectKind};
use search_unitizer::{PreparationError, PreparationLimits, inspect_prepared_utf8};

struct Summary {
    marked: u64,
    verified: u64,
    deleted: u64,
    preserved: u64,
}

fn check(deadline: Instant, cancel: &AtomicBool) -> Result<(), &'static str> {
    if cancel.load(Ordering::Relaxed) {
        Err("COLLECTION_CANCELLED")
    } else if Instant::now() >= deadline {
        Err("COLLECTION_DEADLINE")
    } else {
        Ok(())
    }
}

fn collect(
    root: &PlainRootGuard,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Summary, &'static str> {
    let parent = root.path().parent().ok_or("COLLECTION_SCRATCH_PATH")?;
    let mut marks = ObjectMarks::build(&root.control_path(), parent, deadline, cancel)
        .map_err(MarkError::code)?;
    let collected = (|| {
        let prepared = close_prepared_inputs(root, &mut marks, deadline, cancel)?;
        let mut verified = 0_u64;
        let mut prepared_count = 0_u64;
        let mut previous = None;
        while let Some(reference) = marks
            .next(previous, deadline, cancel)
            .map_err(MarkError::code)?
        {
            check(deadline, cancel)?;
            if reference.kind == ObjectKind::Source {
                let bytes = read_object(root, reference, MAX_SOURCE_OBJECT_BYTES, deadline, cancel)
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
        check(deadline, cancel)?;
        let mut summary = Summary {
            marked: marks.len(),
            verified,
            deleted: 0,
            preserved: 0,
        };
        // No final object is removed before the entire snapshot and every marked
        // object have passed. This is the only final-directory enumeration.
        let entries =
            fs::read_dir(root.object_dir()).map_err(|_| "COLLECTION_OBJECT_DIRECTORY_IO")?;
        for entry in entries {
            check(deadline, cancel)?;
            let entry = entry.map_err(|_| "COLLECTION_OBJECT_DIRECTORY_IO")?;
            let candidate = collection_candidate(root, &entry.file_name(), deadline, cancel)
                .map_err(ObjectStoreError::code)?;
            let removed = if let Some(reference) = candidate {
                if marks
                    .contains(reference, deadline, cancel)
                    .map_err(MarkError::code)?
                {
                    false
                } else {
                    remove_unreferenced_object(root, reference, deadline, cancel)
                        .map_err(ObjectStoreError::code)?
                }
            } else {
                false
            };
            if removed {
                summary.deleted = summary
                    .deleted
                    .checked_add(1)
                    .ok_or("COLLECTION_INPUT_LIMIT")?;
            } else {
                summary.preserved = summary
                    .preserved
                    .checked_add(1)
                    .ok_or("COLLECTION_INPUT_LIMIT")?;
            }
        }
        check(deadline, cancel)?;
        Ok(summary)
    })();
    // Always close and remove only the owned scratch, including budget failures.
    let closed = marks.close(deadline, cancel).map_err(MarkError::code);
    closed?;
    collected
}

fn close_prepared_inputs(
    root: &PlainRootGuard,
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
        let bytes = read_object(root, reference, 64 * 1024 * 1024, deadline, cancel)
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

pub fn run(path: &Path, deadline: Instant, cancel: &Arc<AtomicBool>) -> ExitCode {
    let result = (|| {
        let root =
            open_plain_with_recovery(path, deadline, cancel).map_err(PlainRootError::code)?;
        let summary = collect(&root, deadline, cancel)?;
        check(deadline, cancel)?;
        let bytes = format!(
            "{{\"operation\":\"collect-objects\",\"state\":\"collected\",\"marked\":{},\"verified\":{},\"deleted\":{},\"preserved\":{}}}\n",
            summary.marked, summary.verified, summary.deleted, summary.preserved
        );
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(bytes.as_bytes())
            .and_then(|()| stdout.flush())
            .map_err(|_| "OUTPUT_WRITE_ERROR")?;
        drop(stdout);
        drop(root);
        Ok::<(), &'static str>(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            eprintln!("{{\"error\":\"{code}\"}}");
            ExitCode::from(2)
        }
    }
}
