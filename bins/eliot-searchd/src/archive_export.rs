//! Fresh non-runnable archives, independently verified before completion publication.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_control_redb::object_marks::{MarkError, ObjectMarks};
use search_control_redb::plain_control::{
    ControlCopy, PlainControlError, copy_plain_control, describe_plain_control,
};
use search_control_redb::publication::Generation;
use search_revision_store::{ObjectStoreError, object_name, read_object};
use search_runtime_owner::{
    ARCHIVE_COMPLETE_MARKER, PlainRootError, PlainRootGuard, open_plain_with_recovery,
    sync_directory,
};
use search_source_registry::{MAX_SOURCE_OBJECT_BYTES, ObjectKind};
use serde::{Deserialize, Serialize};

use crate::retained_objects::{self, check};

const HEADER: &str = "archive.header";
const MAGIC: &[u8] = b"eliot-search-archive-complete-v1\n";
const HEADER_BYTES: usize = 4096;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    format: String,
    version: u16,
    control_format: u16,
    object_format: u16,
    baseline: String,
    served_generation: Generation,
    control_bytes: u64,
    control_digest: [u8; 32],
    closure_objects: u64,
}

fn plain(path: &Path, directory: bool) -> Result<(), &'static str> {
    let meta = fs::symlink_metadata(path).map_err(|_| "ARCHIVE_PATH_IO")?;
    if meta.file_type().is_symlink()
        || (directory && !meta.is_dir())
        || (!directory && !meta.is_file())
    {
        return Err("ARCHIVE_PATH_UNSUPPORTED");
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x0000_0400 != 0 {
            return Err("ARCHIVE_PATH_UNSUPPORTED");
        }
    }
    Ok(())
}

fn destination(source: &PlainRootGuard, out: &Path) -> Result<PathBuf, &'static str> {
    if !out.is_absolute() {
        return Err("ARCHIVE_PATH_INVALID");
    }
    let name = out.file_name().ok_or("ARCHIVE_PATH_INVALID")?;
    let parent = out.parent().ok_or("ARCHIVE_PATH_INVALID")?;
    plain(parent, true)?;
    let parent = parent.canonicalize().map_err(|_| "ARCHIVE_PATH_IO")?;
    plain(&parent, true)?;
    let source = source
        .path()
        .canonicalize()
        .map_err(|_| "ARCHIVE_PATH_IO")?;
    let out = parent.join(name);
    if out.starts_with(&source) || source.starts_with(&out) {
        return Err("ARCHIVE_PATH_OVERLAP");
    }
    match fs::symlink_metadata(&out) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(out),
        Err(_) => Err("ARCHIVE_PATH_IO"),
        Ok(_) => Err("ARCHIVE_DESTINATION_EXISTS"),
    }
}

fn write_new(
    path: &Path,
    bytes: &[u8],
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    check(deadline, cancel)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "ARCHIVE_CREATE_IO")?;
    for block in bytes.chunks(64 * 1024) {
        check(deadline, cancel)?;
        file.write_all(block).map_err(|_| "ARCHIVE_WRITE_IO")?;
    }
    file.sync_all().map_err(|_| "ARCHIVE_FILE_FLUSH_IO")?;
    check(deadline, cancel)
}

fn read_small(
    path: &Path,
    maximum: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, &'static str> {
    check(deadline, cancel)?;
    plain(path, false)?;
    let file = File::open(path).map_err(|_| "ARCHIVE_READ_IO")?;
    if file.metadata().map_err(|_| "ARCHIVE_READ_IO")?.len() > maximum as u64 {
        return Err("ARCHIVE_HEADER_LIMIT");
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(maximum + 1)
        .map_err(|_| "ARCHIVE_ALLOCATION_FAILED")?;
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "ARCHIVE_READ_IO")?;
    if bytes.len() > maximum {
        return Err("ARCHIVE_HEADER_LIMIT");
    }
    check(deadline, cancel)?;
    Ok(bytes)
}

fn marker(header: &Header) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(MAGIC.len() + 32);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&header.control_digest);
    bytes
}

fn read_header(
    out: &Path,
    complete: bool,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Header, &'static str> {
    let bytes = read_small(&out.join(HEADER), HEADER_BYTES, deadline, cancel)?;
    let header: Header = serde_json::from_slice(&bytes).map_err(|_| "ARCHIVE_HEADER_INVALID")?;
    if header.format != "eliot-search-archive"
        || header.version != 1
        || header.control_format != 1
        || header.object_format != 1
        || header.baseline != "served"
        || header.control_bytes == 0
    {
        return Err("ARCHIVE_FORMAT_UNSUPPORTED");
    }
    if complete
        && read_small(&out.join(ARCHIVE_COMPLETE_MARKER), 256, deadline, cancel)? != marker(&header)
    {
        return Err("ARCHIVE_COMPLETION_INVALID");
    }
    Ok(header)
}

fn layout(
    path: &Path,
    expected: &[(&str, bool)],
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    plain(path, true)?;
    let mut count = 0_usize;
    for entry in fs::read_dir(path).map_err(|_| "ARCHIVE_DIRECTORY_IO")? {
        check(deadline, cancel)?;
        let entry = entry.map_err(|_| "ARCHIVE_DIRECTORY_IO")?;
        let name = entry.file_name();
        let (_, directory) = expected
            .iter()
            .find(|(key, _)| name == std::ffi::OsStr::new(key))
            .ok_or("ARCHIVE_LAYOUT_UNSUPPORTED")?;
        plain(&entry.path(), *directory)?;
        count += 1;
        if count > expected.len() {
            return Err("ARCHIVE_LAYOUT_UNSUPPORTED");
        }
    }
    if count != expected.len() {
        return Err("ARCHIVE_LAYOUT_INCOMPLETE");
    }
    Ok(())
}

fn verify_copy(out: &Path, deadline: Instant, cancel: &AtomicBool) -> Result<Header, &'static str> {
    layout(
        out,
        &[(HEADER, false), ("control", true), ("objects", true)],
        deadline,
        cancel,
    )?;
    layout(
        &out.join("control"),
        &[("control.redb", false)],
        deadline,
        cancel,
    )?;
    let header = read_header(out, false, deadline, cancel)?;
    let control = out.join("control/control.redb");
    let described =
        describe_plain_control(&control, deadline, cancel).map_err(PlainControlError::code)?;
    if described
        != (ControlCopy {
            byte_length: header.control_bytes,
            digest: header.control_digest,
        })
    {
        return Err("ARCHIVE_CONTROL_MISMATCH");
    }
    let marks = retained_objects::verify(
        &control,
        &out.join("objects"),
        out.parent().ok_or("ARCHIVE_PATH_INVALID")?,
        deadline,
        cancel,
    )?;
    let checked = (|| {
        if marks.served_generation() != Some(&header.served_generation)
            || marks.len() != header.closure_objects
        {
            return Err("ARCHIVE_SNAPSHOT_MISMATCH");
        }
        // Verification proved each distinct required canonical name exists. An
        // exact directory count then excludes extras without a RAM name set.
        let mut count = 0_u64;
        for entry in fs::read_dir(out.join("objects")).map_err(|_| "ARCHIVE_DIRECTORY_IO")? {
            check(deadline, cancel)?;
            let entry = entry.map_err(|_| "ARCHIVE_DIRECTORY_IO")?;
            plain(&entry.path(), false)?;
            count = count.checked_add(1).ok_or("ARCHIVE_INPUT_LIMIT")?;
            if count > marks.len() {
                return Err("ARCHIVE_LAYOUT_UNSUPPORTED");
            }
        }
        if count != marks.len() {
            return Err("ARCHIVE_LAYOUT_INCOMPLETE");
        }
        Ok(())
    })();
    let closed = marks.close(deadline, cancel).map_err(MarkError::code);
    closed?;
    checked?;
    Ok(header)
}

fn copy_objects(
    root: &PlainRootGuard,
    out: &Path,
    marks: &ObjectMarks,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    let mut previous = None;
    while let Some(reference) = marks
        .next(previous, deadline, cancel)
        .map_err(MarkError::code)?
    {
        let maximum = match reference.kind {
            ObjectKind::Source => MAX_SOURCE_OBJECT_BYTES,
            ObjectKind::Prepared => 64 * 1024 * 1024,
        };
        let bytes = read_object(root, reference, maximum, deadline, cancel)
            .map_err(ObjectStoreError::code)?;
        let name = object_name(reference, maximum).map_err(ObjectStoreError::code)?;
        write_new(&out.join("objects").join(name), &bytes, deadline, cancel)?;
        previous = Some(reference);
    }
    Ok(())
}

fn export(
    root: &PlainRootGuard,
    out: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Header, &'static str> {
    let out = destination(root, out)?;
    let marks = retained_objects::verify(
        &root.control_path(),
        &root.object_dir(),
        root.path().parent().ok_or("ARCHIVE_PATH_INVALID")?,
        deadline,
        cancel,
    )?;
    let exported = (|| {
        let served = marks
            .served_generation()
            .ok_or("ARCHIVE_NOT_PUBLISHED")?
            .clone();
        check(deadline, cancel)?;
        fs::create_dir(&out).map_err(|_| "ARCHIVE_CREATE_IO")?;
        fs::create_dir(out.join("control")).map_err(|_| "ARCHIVE_CREATE_IO")?;
        fs::create_dir(out.join("objects")).map_err(|_| "ARCHIVE_CREATE_IO")?;
        // No runnable manifest, initialization marker, lock or backend data is copied.
        let mut control = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(out.join("control/control.redb"))
            .map_err(|_| "ARCHIVE_CREATE_IO")?;
        let copy = copy_plain_control(&root.control_path(), &mut control, deadline, cancel)
            .map_err(PlainControlError::code)?;
        drop(control);
        let header = Header {
            format: "eliot-search-archive".to_owned(),
            version: 1,
            control_format: 1,
            object_format: 1,
            baseline: "served".to_owned(),
            served_generation: served,
            control_bytes: copy.byte_length,
            control_digest: copy.digest,
            closure_objects: marks.len(),
        };
        let bytes = serde_json::to_vec(&header).map_err(|_| "ARCHIVE_HEADER_INVALID")?;
        if bytes.len() > HEADER_BYTES {
            return Err("ARCHIVE_HEADER_LIMIT");
        }
        write_new(&out.join(HEADER), &bytes, deadline, cancel)?;
        copy_objects(root, &out, &marks, deadline, cancel)?;
        if verify_copy(&out, deadline, cancel)? != header {
            return Err("ARCHIVE_SNAPSHOT_MISMATCH");
        }
        for directory in [
            out.join("control"),
            out.join("objects"),
            out.clone(),
            out.parent().ok_or("ARCHIVE_PATH_INVALID")?.to_path_buf(),
        ] {
            sync_directory(&directory, deadline, cancel).map_err(PlainRootError::code)?;
        }
        write_new(
            &out.join(ARCHIVE_COMPLETE_MARKER),
            &marker(&header),
            deadline,
            cancel,
        )?;
        sync_directory(&out, deadline, cancel).map_err(PlainRootError::code)?;
        layout(
            &out,
            &[
                (HEADER, false),
                ("control", true),
                ("objects", true),
                (ARCHIVE_COMPLETE_MARKER, false),
            ],
            deadline,
            cancel,
        )?;
        if read_header(&out, true, deadline, cancel)? != header {
            return Err("ARCHIVE_COMPLETION_INVALID");
        }
        Ok(header)
    })();
    // Preserve the source and every owned partial archive; only disposable marks close.
    marks.close(deadline, cancel).map_err(MarkError::code)?;
    exported
}

pub fn run(source: &Path, out: &Path, deadline: Instant, cancel: &Arc<AtomicBool>) -> ExitCode {
    let result = (|| {
        let root =
            open_plain_with_recovery(source, deadline, cancel).map_err(PlainRootError::code)?;
        let header = export(&root, out, deadline, cancel)?;
        check(deadline, cancel)?;
        let bytes = format!(
            "{{\"operation\":\"archive-export\",\"state\":\"complete\",\"generation\":{},\"objects\":{},\"control_bytes\":{}}}\n",
            header.served_generation.sequence, header.closure_objects, header.control_bytes
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
