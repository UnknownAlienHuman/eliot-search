//! Explicit whole-served import; source CONTROL and backend authority stay inert.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Instant;

use search_control_redb::plain_control::{ControlWriter, PlainControlError};
use search_control_redb::publication::{CapturedSource, Generation, PublicationError};
use search_revision_store::{ObjectStoreError, read_object_in, store_object};
use search_runtime_owner::{PlainRootError, PlainRootGuard, initialize_plain, open_plain_marked};
use search_source_registry::{MAX_SOURCE_OBJECT_BYTES, ObjectKind};

use crate::app::PublicationRequest;
use crate::archive_export::{self, Header, plain};
use crate::retained_objects::check;

fn paths(
    archive: &Path,
    destination: &Path,
    retry: bool,
) -> Result<(PathBuf, PathBuf), &'static str> {
    if !archive.is_absolute() || !destination.is_absolute() {
        return Err("IMPORT_PATH_INVALID");
    }
    plain(archive, true)?;
    let archive = archive.canonicalize().map_err(|_| "IMPORT_PATH_IO")?;
    let parent = destination.parent().ok_or("IMPORT_PATH_INVALID")?;
    plain(parent, true)?;
    let parent = parent.canonicalize().map_err(|_| "IMPORT_PATH_IO")?;
    plain(&parent, true)?;
    let destination = parent.join(destination.file_name().ok_or("IMPORT_PATH_INVALID")?);
    let exists = match fs::symlink_metadata(&destination) {
        Ok(_) => {
            plain(&destination, true)?;
            if !retry
                && fs::read_dir(&destination)
                    .map_err(|_| "IMPORT_PATH_IO")?
                    .next()
                    .transpose()
                    .map_err(|_| "IMPORT_PATH_IO")?
                    .is_some()
            {
                return Err("IMPORT_DESTINATION_EXISTS");
            }
            true
        }
        Err(error) if !retry && error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err("IMPORT_PATH_IO"),
    };
    let destination = if exists {
        destination.canonicalize().map_err(|_| "IMPORT_PATH_IO")?
    } else {
        destination
    };
    if archive.starts_with(&destination) || destination.starts_with(&archive) {
        return Err("IMPORT_PATH_OVERLAP");
    }
    Ok((archive, destination))
}

fn marker(destination: &Path, header: &Header) -> Result<Vec<u8>, &'static str> {
    let mut hash = blake3::Hasher::new();
    hash.update(b"eliot-search-import-destination-v1\0");
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        for unit in destination.as_os_str().encode_wide() {
            hash.update(&unit.to_le_bytes());
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hash.update(destination.as_os_str().as_bytes());
    }
    #[cfg(not(any(unix, windows)))]
    return Err("IMPORT_PLATFORM_UNSUPPORTED");
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(b"eliot-search-import-v1\n");
    bytes.extend_from_slice(hash.finalize().as_bytes());
    bytes.extend_from_slice(&header.control_digest);
    bytes.extend_from_slice(&header.served_generation.namespace);
    bytes.extend_from_slice(&header.served_generation.sequence.to_le_bytes());
    if bytes.len() > 256 {
        return Err("IMPORT_MARKER_LIMIT");
    }
    Ok(bytes)
}

fn copy_objects(
    root: &PlainRootGuard,
    archive: &Path,
    sources: &[CapturedSource],
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    for captured in sources {
        let outcome = captured
            .preparation
            .outcome
            .as_ref()
            .ok_or("IMPORT_BASELINE_MISMATCH")?;
        let artifact = match outcome.result {
            search_control_redb::current_preparation::PreparationOutcome::Ready { artifact } => {
                Some(artifact)
            }
            search_control_redb::current_preparation::PreparationOutcome::Gap { .. } => None,
        };
        let references = [
            Some(captured.source.object_ref),
            Some(outcome.input),
            artifact,
        ];
        for (index, reference) in references.iter().enumerate() {
            let Some(reference) = *reference else {
                continue;
            };
            if references[..index].contains(&Some(reference)) {
                continue;
            }
            check(deadline, cancel)?;
            let maximum = if reference.kind == ObjectKind::Source {
                MAX_SOURCE_OBJECT_BYTES
            } else {
                64 * 1024 * 1024
            };
            let bytes = read_object_in(
                &archive.join("objects"),
                reference,
                maximum,
                deadline,
                cancel,
            )
            .map_err(ObjectStoreError::code)?;
            let stored = store_object(root, reference.kind, &bytes, maximum, deadline, cancel)
                .map_err(ObjectStoreError::code)?;
            if stored != reference {
                return Err("IMPORT_OBJECT_MISMATCH");
            }
        }
    }
    Ok(())
}

fn import(
    archive: &Path,
    destination: &Path,
    retry: bool,
    request: &PublicationRequest,
    deadline: Instant,
    cancel: &Arc<AtomicBool>,
) -> Result<(PlainRootGuard, Generation), &'static str> {
    check(deadline, cancel)?;
    let (archive, destination) = paths(archive, destination, retry)?;
    // All archive closure and both metadata budgets are accepted before DEST effects.
    let (header, sources) = archive_export::read_served(&archive, deadline, cancel)?;
    let (mut root, marker) = if retry {
        let expected = marker(&destination, &header)?;
        let root = open_plain_marked(&destination, &expected, deadline, cancel)
            .map_err(PlainRootError::code)?;
        let canonical = root.path().canonicalize().map_err(|_| "IMPORT_PATH_IO")?;
        if marker(&canonical, &header)? != expected {
            return Err("IMPORT_DESTINATION_MISMATCH");
        }
        (root, expected)
    } else {
        let mut root =
            initialize_plain(&destination, deadline, cancel).map_err(PlainRootError::code)?;
        let canonical = root.path().canonicalize().map_err(|_| "IMPORT_PATH_IO")?;
        if archive.starts_with(&canonical) || canonical.starts_with(&archive) {
            return Err("IMPORT_PATH_OVERLAP");
        }
        let expected = marker(&canonical, &header)?;
        root.publish_import_marker(&expected, deadline, cancel)
            .map_err(PlainRootError::code)?;
        (root, expected)
    };
    copy_objects(&root, &archive, &sources, deadline, cancel)?;
    let writer = ControlWriter::open(&root.control_path(), deadline, cancel)
        .map_err(PlainControlError::code)?;
    if let Err(error) = writer.install_served_corpus(&sources, deadline, cancel) {
        writer
            .close(deadline, cancel)
            .map_err(PlainControlError::code)?;
        return Err(error.code());
    }
    let view = crate::published_corpus::publish(&root, writer, request, deadline, cancel)?;
    let accepted = (|| {
        if view.generation.namespace == header.served_generation.namespace
            || view.generation.source_count != header.served_generation.source_count
            || view.generation.gap_count != header.served_generation.gap_count
            || view.generation.point_count != header.served_generation.point_count
            || view
                .source_inventory(deadline, cancel)
                .map_err(PublicationError::code)?
                != sources
        {
            return Err("IMPORT_BASELINE_MISMATCH");
        }
        Ok(view.generation.clone())
    })();
    view.close().map_err(PublicationError::code)?;
    let generation = accepted?;
    root.clear_import_marker(&marker, deadline, cancel)
        .map_err(PlainRootError::code)?;
    Ok((root, generation))
}

pub fn run(
    archive: &Path,
    destination: &Path,
    retry: bool,
    request: &PublicationRequest,
    deadline: Instant,
    cancel: &Arc<AtomicBool>,
) -> ExitCode {
    let result = (|| {
        let (root, generation) = import(archive, destination, retry, request, deadline, cancel)?;
        check(deadline, cancel)?;
        let output = serde_json::json!({"operation":"archive-import","state":"complete","generation":generation.sequence,
            "collection":generation.collection(),"source_count":generation.source_count,"gap_count":generation.gap_count,"point_count":generation.point_count,"page_count":generation.page_count});
        let bytes = serde_json::to_vec(&output).map_err(|_| "IMPORT_RESPONSE_INVALID")?;
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(&bytes)
            .and_then(|()| stdout.write_all(b"\n"))
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
