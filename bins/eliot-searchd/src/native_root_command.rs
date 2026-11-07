//! Executes one plain-root operation and emits its observed final state.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use ignore::WalkBuilder;
use search_control_redb::current_source::{
    MAX_CURRENT_SOURCES, capture_current_source, capture_current_sources, commit_current_source,
    remove_current_sources, source_locator_bytes, source_path_from_locator,
};
use search_revision_store::store_object;
use search_runtime_owner::{
    PlainRootError, PlainRootGuard, PlainRootStatus, discard_incomplete_plain, initialize_plain,
    inspect_plain, open_plain,
};
use search_safe_reader::{MAX_SOURCE_FILE_BYTES, acquire_file};
use search_source_registry::{CurrentSourceRecord, ObjectKind};

use crate::app::{RootCommand, RootInvocation};

static CANCELLED: AtomicBool = AtomicBool::new(false);

const fn status_name(status: PlainRootStatus) -> &'static str {
    match status {
        PlainRootStatus::Absent => "absent",
        PlainRootStatus::Empty => "empty",
        PlainRootStatus::IncompleteInitialization => "incomplete_initialization",
        PlainRootStatus::Current => "current",
        PlainRootStatus::Corrupt => "corrupt",
        PlainRootStatus::Unsupported => "unsupported",
        PlainRootStatus::Busy => "busy",
        PlainRootStatus::Archive => "archive",
        PlainRootStatus::IncompleteImport => "incomplete_import",
    }
}

fn emit_snapshot(command: RootCommand, status: PlainRootStatus) {
    println!(
        "{{\"operation\":\"{}\",\"root_state\":\"{}\"}}",
        command.operation(),
        status_name(status)
    );
}

fn emit_error(error: PlainRootError) -> ExitCode {
    eprintln!("{{\"error\":\"{}\"}}", error.code());
    ExitCode::from(2)
}

fn emit_guarded(command: RootCommand, guard: PlainRootGuard) -> ExitCode {
    if !guard.is_current() {
        drop(guard);
        return emit_error(PlainRootError::Corrupt);
    }
    emit_snapshot(command, PlainRootStatus::Current);
    drop(guard);
    ExitCode::SUCCESS
}

fn emit_source_error(code: &str) -> ExitCode {
    eprintln!("{{\"error\":\"{code}\"}}");
    ExitCode::from(2)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn emit_source_record(record: &CurrentSourceRecord) {
    let object = record.object_ref;
    println!(
        "{{\"operation\":\"retain-file\",\"source_namespace_id\":\"{}\",\"source_id\":\"{}\",\"source_row_revision\":{},\"registration_revision\":{},\"object_version\":{},\"object_kind\":\"source\",\"object_digest\":\"{}\",\"object_bytes\":{},\"native_file_key\":\"{}\",\"readback\":\"verified\"}}",
        record.source_namespace_id,
        record.source_id,
        record.row_revision,
        record.registration_revision,
        object.format_version,
        hex(object.digest.as_bytes()),
        object.byte_length,
        hex(&record.file_key.to_bytes()),
    );
}

fn retain_file(
    root: &PlainRootGuard,
    file_path: &Path,
    deadline: Instant,
) -> Result<CurrentSourceRecord, &'static str> {
    let capture = capture_current_source(&root.control_path(), file_path, deadline, &CANCELLED)
        .map_err(|error| error.code())?;
    let acquired = acquire_file(file_path, MAX_SOURCE_FILE_BYTES, deadline, &CANCELLED)
        .map_err(|error| error.code())?;
    let (bytes, file_key, acquired_length) = acquired.into_parts();
    let object_ref = match store_object(
        root,
        ObjectKind::Source,
        bytes.as_slice(),
        MAX_SOURCE_FILE_BYTES,
        deadline,
        &CANCELLED,
    ) {
        Ok(object_ref) if object_ref.byte_length == acquired_length => object_ref,
        Ok(_) => return Err("SOURCE_LENGTH_MISMATCH"),
        Err(error) => return Err(error.code()),
    };
    drop(bytes);
    commit_current_source(
        &root.control_path(),
        capture,
        file_key,
        object_ref,
        deadline,
        &CANCELLED,
    )
    .map_err(|error| error.code())
}

fn retain_file_command(data_root: &Path, file_path: &Path, deadline: Instant) -> ExitCode {
    let root = match open_plain(data_root, deadline, &CANCELLED) {
        Ok(root) => root,
        Err(error) => return emit_error(error),
    };
    match retain_file(&root, file_path, deadline) {
        Ok(record) => {
            emit_source_record(&record);
            drop(root);
            ExitCode::SUCCESS
        }
        Err(code) => emit_source_error(code),
    }
}

struct IndexScan {
    root: PathBuf,
    is_directory: bool,
    missing_file: bool,
    files: Vec<PathBuf>,
    locators: HashSet<Vec<u8>>,
}

fn check_index(deadline: Instant) -> Result<(), &'static str> {
    if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
        Err("INDEX_CANCELLED")
    } else if Instant::now() >= deadline {
        Err("INDEX_DEADLINE")
    } else {
        Ok(())
    }
}

fn is_git_metadata(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == OsStr::new(".git"))
}

fn protected_index_path(path: &Path, data_root: &Path) -> bool {
    is_git_metadata(path) || path.starts_with(data_root)
}

fn scan_index_path(
    index_path: &Path,
    data_root: &Path,
    deadline: Instant,
) -> Result<IndexScan, &'static str> {
    check_index(deadline)?;
    if !index_path.is_absolute() || protected_index_path(index_path, data_root) {
        return Err("INDEX_SCOPE_INVALID");
    }
    let metadata = match fs::symlink_metadata(index_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(IndexScan {
                root: index_path.to_path_buf(),
                is_directory: false,
                missing_file: true,
                files: Vec::new(),
                locators: HashSet::new(),
            });
        }
        Err(_) => return Err("INDEX_SCAN_INCOMPLETE"),
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err("INDEX_REPARSE_REFUSED");
    }
    if !metadata.is_file() && !metadata.is_dir() {
        return Err("INDEX_SCOPE_INVALID");
    }
    let is_directory = metadata.is_dir();
    let mut files = Vec::new();
    if !is_directory {
        files.push(index_path.to_path_buf());
    } else {
        let excluded_root = data_root.to_path_buf();
        let mut builder = WalkBuilder::new(index_path);
        builder
            .standard_filters(true)
            .follow_links(false)
            .filter_entry(move |entry| {
                !is_git_metadata(entry.path()) && !entry.path().starts_with(&excluded_root)
            });
        for entry in builder.build() {
            check_index(deadline)?;
            let entry = entry.map_err(|_| "INDEX_SCAN_INCOMPLETE")?;
            let path = entry.path();
            let file_type = entry.file_type().ok_or("INDEX_SCAN_INCOMPLETE")?;
            if file_type.is_symlink() {
                return Err("INDEX_REPARSE_REFUSED");
            }
            if file_type.is_dir() {
                continue;
            }
            if !file_type.is_file() {
                return Err("INDEX_SCAN_INCOMPLETE");
            }
            if files.len() >= MAX_CURRENT_SOURCES {
                return Err("INDEX_SOURCE_LIMIT");
            }
            files.push(path.to_path_buf());
        }
    }
    files.sort_by(|left, right| {
        left.as_os_str()
            .as_encoded_bytes()
            .cmp(right.as_os_str().as_encoded_bytes())
    });
    let mut locators = HashSet::with_capacity(files.len());
    for path in &files {
        check_index(deadline)?;
        locators.insert(source_locator_bytes(path).map_err(|error| error.code())?);
    }
    Ok(IndexScan {
        root: index_path.to_path_buf(),
        is_directory,
        missing_file: false,
        files,
        locators,
    })
}

fn source_is_in_scope(source_path: &Path, scan: &IndexScan) -> bool {
    if scan.is_directory {
        source_path.starts_with(&scan.root)
    } else {
        source_path == scan.root
    }
}

fn confirmed_removals(
    inventory: &[CurrentSourceRecord],
    scan: &IndexScan,
    deadline: Instant,
) -> Result<Vec<CurrentSourceRecord>, &'static str> {
    let mut removals = Vec::new();
    for source in inventory {
        check_index(deadline)?;
        let path = source_path_from_locator(&source.locator).map_err(|error| error.code())?;
        if !source_is_in_scope(&path, scan) || scan.locators.contains(&source.locator) {
            continue;
        }
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                removals.push(source.clone());
            }
            Err(_) => return Err("INDEX_SCAN_INCOMPLETE"),
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("INDEX_REPARSE_REFUSED");
            }
            Ok(metadata) if metadata.is_file() => {
                // A complete ignore-aware walk omitted a still-present file, so it is
                // outside this refresh's selected source set.
                removals.push(source.clone());
            }
            Ok(_) => return Err("INDEX_SCAN_INCOMPLETE"),
        }
    }
    if scan.missing_file && removals.is_empty() {
        return Err("INDEX_FILE_NOT_FOUND");
    }
    Ok(removals)
}

fn index_sources(
    root: &PlainRootGuard,
    data_root: &Path,
    index_path: &Path,
    request: &crate::app::PublicationRequest,
    deadline: Instant,
) -> Result<(), &'static str> {
    let control = root.control_path();
    let inventory =
        capture_current_sources(&control, deadline, &CANCELLED).map_err(|error| error.code())?;
    let scan = scan_index_path(index_path, data_root, deadline)?;
    for file in &scan.files {
        check_index(deadline)?;
        let source = retain_file(root, file, deadline)?;
        crate::prepared_source::prepare(root, source.source_id, deadline, &CANCELLED)?;
    }

    let verified_scan = scan_index_path(index_path, data_root, deadline)?;
    if scan.is_directory != verified_scan.is_directory
        || scan.missing_file != verified_scan.missing_file
        || scan.locators != verified_scan.locators
    {
        return Err("INDEX_SCAN_CHANGED");
    }
    let removals = confirmed_removals(&inventory, &scan, deadline)?;
    remove_current_sources(&control, &removals, deadline, &CANCELLED)
        .map_err(|error| error.code())?;

    let view = crate::published_corpus::publish(root, request, deadline, &CANCELLED)?;
    view.close().map_err(|_| "PUBLICATION_CLOSE_FAILED")?;
    Ok(())
}

fn index_command(
    data_root: &Path,
    index_path: &Path,
    request: &crate::app::PublicationRequest,
    deadline: Instant,
) -> ExitCode {
    let root = match open_plain(data_root, deadline, &CANCELLED) {
        Ok(root) => root,
        Err(error) => return emit_error(error),
    };
    match index_sources(&root, data_root, index_path, request, deadline) {
        Ok(()) => {
            println!("{{\"operation\":\"index\",\"state\":\"refreshed\"}}");
            drop(root);
            ExitCode::SUCCESS
        }
        Err(code) => emit_source_error(code),
    }
}

pub fn run(invocation: RootInvocation, started: Instant) -> ExitCode {
    let Some(deadline) = started.checked_add(Duration::from_secs(120)) else {
        return emit_error(PlainRootError::Deadline);
    };
    let RootInvocation {
        command,
        data_root,
        source_file,
        index_path,
        source_id,
        unit,
        publication,
        search_endpoint,
    } = invocation;
    match command {
        RootCommand::Inspect => match inspect_plain(&data_root, deadline, &CANCELLED) {
            Ok(inspection) => {
                emit_snapshot(command, inspection.status);
                ExitCode::SUCCESS
            }
            Err(error) => emit_error(error),
        },
        RootCommand::CheckExistingOwner => match open_plain(&data_root, deadline, &CANCELLED) {
            Ok(guard) => emit_guarded(command, guard),
            Err(error) => emit_error(error),
        },
        RootCommand::Initialize => match initialize_plain(&data_root, deadline, &CANCELLED) {
            Ok(guard) => emit_guarded(command, guard),
            Err(error) => emit_error(error),
        },
        RootCommand::DiscardIncomplete => {
            match discard_incomplete_plain(&data_root, deadline, &CANCELLED)
                .and_then(|()| inspect_plain(&data_root, deadline, &CANCELLED))
            {
                Ok(inspection) => {
                    emit_snapshot(command, inspection.status);
                    ExitCode::SUCCESS
                }
                Err(error) => emit_error(error),
            }
        }
        RootCommand::RetainFile => source_file.map_or_else(
            || emit_source_error("SOURCE_FILE_REQUIRED"),
            |file_path| retain_file_command(&data_root, &file_path, deadline),
        ),
        RootCommand::PrepareSource => source_id.map_or_else(
            || emit_source_error("PREPARATION_SOURCE_ID_REQUIRED"),
            |id| {
                crate::prepared_source::run(&data_root, id, unit.unwrap_or(0), deadline, &CANCELLED)
            },
        ),
        RootCommand::PublishPrepared => publication.map_or_else(
            || emit_source_error("PUBLICATION_ARGUMENT_REQUIRED"),
            |request| crate::published_corpus::run(&data_root, &request, deadline, &CANCELLED),
        ),
        RootCommand::Index => match (index_path, publication) {
            (Some(index_path), Some(request)) => {
                index_command(&data_root, &index_path, &request, deadline)
            }
            _ => emit_source_error("INDEX_ARGUMENT_REQUIRED"),
        },
        RootCommand::Search => search_endpoint.map_or_else(
            || emit_source_error("QUERY_ENDPOINT_REQUIRED"),
            |endpoint| crate::framed_search::run(&data_root, &endpoint),
        ),
    }
}
