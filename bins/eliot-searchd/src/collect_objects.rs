//! One stopped-root complete mark, verified objects, then one streaming local sweep.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_control_redb::object_marks::MarkError;
use search_revision_store::{ObjectStoreError, collection_candidate, remove_unreferenced_object};
use search_runtime_owner::{PlainRootError, PlainRootGuard, open_plain_with_recovery};

use crate::retained_objects::{self, check};

struct Summary {
    marked: u64,
    verified: u64,
    deleted: u64,
    preserved: u64,
}

fn collect(
    root: &PlainRootGuard,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Summary, &'static str> {
    let parent = root.path().parent().ok_or("COLLECTION_SCRATCH_PATH")?;
    let marks = retained_objects::verify(
        &root.control_path(),
        &root.object_dir(),
        parent,
        deadline,
        cancel,
    )?;
    let collected = (|| {
        check(deadline, cancel)?;
        let mut summary = Summary {
            marked: marks.len(),
            verified: marks.len(),
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
