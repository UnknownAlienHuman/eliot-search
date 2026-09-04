//! Concrete bounded runtime helpers for the primary daemon.
//!
//! Data-root exclusion uses the operating-system file-lock API. One-shot file
//! scanning reads one final handle and verifies identity and metadata before and
//! after the read. These helpers own no source-catalog or revision semantics.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const MAX_SCAN_INPUT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_SCAN_QUERY_BYTES: usize = 64 * 1024;
pub(crate) const MAX_SCAN_MATCHES: usize = 100_000;

/// Truthful capability summary for one daemon composition state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Health {
    pub(crate) configuration_ready: bool,
    pub(crate) runtime_owner_ready: bool,
    pub(crate) control_store_ready: bool,
    pub(crate) secret_store_ready: bool,
    pub(crate) endpoint_ready: bool,
    pub(crate) direct_store_ready: bool,
    pub(crate) source_backed_search_available: bool,
    pub(crate) development_stdin_scan_available: bool,
    pub(crate) development_file_scan_available: bool,
}

impl Health {
    /// Process shell before a data root is opened.
    pub(crate) const SHELL: Self = Self {
        configuration_ready: true,
        runtime_owner_ready: false,
        control_store_ready: false,
        secret_store_ready: false,
        endpoint_ready: true,
        direct_store_ready: false,
        source_backed_search_available: false,
        development_stdin_scan_available: true,
        development_file_scan_available: true,
    };

    /// Owner lock is held, but no persistent source store was admitted.
    pub(crate) const OWNED_SHELL: Self = Self {
        runtime_owner_ready: true,
        ..Self::SHELL
    };

    /// Owner-fenced persistent DIRECT source store is open and verified.
    pub(crate) const DIRECT_STORE: Self = Self {
        configuration_ready: true,
        runtime_owner_ready: true,
        control_store_ready: true,
        secret_store_ready: cfg!(windows),
        endpoint_ready: true,
        direct_store_ready: true,
        source_backed_search_available: true,
        development_stdin_scan_available: true,
        development_file_scan_available: true,
    };

    pub(crate) fn json(self) -> String {
        format!(
            concat!(
                "{{\"status\":\"development_shell\",",
                "\"configuration_ready\":{},",
                "\"runtime_owner_ready\":{},",
                "\"control_store_ready\":{},",
                "\"secret_store_ready\":{},",
                "\"endpoint_ready\":{},",
                "\"direct_store_ready\":{},",
                "\"source_backed_search_available\":{},",
                "\"development_stdin_scan_available\":{},",
                "\"development_file_scan_available\":{}}}"
            ),
            self.configuration_ready,
            self.runtime_owner_ready,
            self.control_store_ready,
            self.secret_store_ready,
            self.endpoint_ready,
            self.direct_store_ready,
            self.source_backed_search_available,
            self.development_stdin_scan_available,
            self.development_file_scan_available,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScanMatch {
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) line: usize,
    pub(crate) column_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScanCoverage {
    pub(crate) input_bytes: usize,
    pub(crate) complete: bool,
    pub(crate) match_limit_reached: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScanResult {
    pub(crate) matches: Vec<ScanMatch>,
    pub(crate) coverage: ScanCoverage,
}

/// Performs bounded deterministic literal UTF-8 search.
pub(crate) fn scan_text(
    text: &str,
    query: &str,
    ascii_insensitive: bool,
) -> Result<ScanResult, String> {
    if query.is_empty() {
        return Err("SCAN_QUERY_EMPTY".to_owned());
    }
    if query.len() > MAX_SCAN_QUERY_BYTES {
        return Err("SCAN_QUERY_TOO_LARGE".to_owned());
    }
    if text.len() > MAX_SCAN_INPUT_BYTES {
        return Err("SCAN_INPUT_TOO_LARGE".to_owned());
    }
    if query.len() > text.len() {
        return Ok(ScanResult {
            matches: Vec::new(),
            coverage: ScanCoverage {
                input_bytes: text.len(),
                complete: true,
                match_limit_reached: false,
            },
        });
    }

    let text_bytes = text.as_bytes();
    let query_bytes = query.as_bytes();
    let mut line_starts = vec![0_usize];
    for (index, byte) in text_bytes.iter().copied().enumerate() {
        if byte == b'\n' {
            line_starts.push(index.saturating_add(1));
        }
    }

    let mut matches = Vec::new();
    let last_start = text_bytes.len() - query_bytes.len();
    let mut truncated = false;
    for start in 0..=last_start {
        if !text.is_char_boundary(start) {
            continue;
        }
        let end = start + query_bytes.len();
        if !text.is_char_boundary(end) {
            continue;
        }
        let equal = if ascii_insensitive {
            text_bytes[start..end].eq_ignore_ascii_case(query_bytes)
        } else {
            &text_bytes[start..end] == query_bytes
        };
        if !equal {
            continue;
        }
        if matches.len() >= MAX_SCAN_MATCHES {
            truncated = true;
            break;
        }
        let line = line_starts
            .partition_point(|line_start| *line_start <= start)
            .saturating_sub(1);
        matches.push(ScanMatch {
            byte_start: start,
            byte_end: end,
            line,
            column_bytes: start - line_starts[line],
        });
    }
    Ok(ScanResult {
        matches,
        coverage: ScanCoverage {
            input_bytes: text.len(),
            complete: !truncated,
            match_limit_reached: truncated,
        },
    })
}

/// Reads bounded UTF-8 from standard input.
pub(crate) fn read_stdin_bounded() -> Result<String, String> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(u64::try_from(MAX_SCAN_INPUT_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("SCAN_READ_ERROR:{error}"))?;
    if bytes.len() > MAX_SCAN_INPUT_BYTES {
        return Err("SCAN_INPUT_TOO_LARGE".to_owned());
    }
    String::from_utf8(bytes).map_err(|_| "SCAN_INPUT_INVALID_UTF8".to_owned())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileObservation {
    length: u64,
    modified_nanos: Option<u128>,
    platform_identity: PlatformFileIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PlatformFileIdentity {
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
    #[cfg(windows)]
    Windows {
        volume_serial: Option<u32>,
        file_index: Option<u64>,
    },
    #[cfg(not(any(unix, windows)))]
    Portable,
}

fn observe_file(file: &File) -> Result<FileObservation, String> {
    let metadata = file
        .metadata()
        .map_err(|error| format!("SCAN_FILE_METADATA_ERROR:{error}"))?;
    if !metadata.is_file() {
        return Err("SCAN_FILE_NOT_REGULAR".to_owned());
    }
    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos());

    #[cfg(unix)]
    let platform_identity = {
        use std::os::unix::fs::MetadataExt;
        PlatformFileIdentity::Unix {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    };

    #[cfg(windows)]
    let platform_identity = {
        use std::os::windows::fs::MetadataExt;
        PlatformFileIdentity::Windows {
            volume_serial: metadata.volume_serial_number(),
            file_index: metadata.file_index(),
        }
    };

    #[cfg(not(any(unix, windows)))]
    let platform_identity = PlatformFileIdentity::Portable;

    Ok(FileObservation {
        length: metadata.len(),
        modified_nanos,
        platform_identity,
    })
}

/// Reads one regular non-link file through the same open handle before and
/// after identity verification.
pub(crate) fn read_file_bounded(path: &Path) -> Result<String, String> {
    let link_metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("SCAN_FILE_OPEN_ERROR:{error}"))?;
    if link_metadata.file_type().is_symlink() || is_reparse(&link_metadata) {
        return Err("SCAN_FILE_LINK_DENIED".to_owned());
    }
    if !link_metadata.is_file() {
        return Err("SCAN_FILE_NOT_REGULAR".to_owned());
    }

    let mut file = File::open(path)
        .map_err(|error| format!("SCAN_FILE_OPEN_ERROR:{error}"))?;
    let before = observe_file(&file)?;
    if before.length > u64::try_from(MAX_SCAN_INPUT_BYTES).unwrap_or(u64::MAX) {
        return Err("SCAN_INPUT_TOO_LARGE".to_owned());
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(before.length)
            .map_err(|_| "SCAN_INPUT_TOO_LARGE".to_owned())?,
    );
    (&mut file)
        .take(u64::try_from(MAX_SCAN_INPUT_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("SCAN_FILE_READ_ERROR:{error}"))?;
    if bytes.len() > MAX_SCAN_INPUT_BYTES {
        return Err("SCAN_INPUT_TOO_LARGE".to_owned());
    }
    let after = observe_file(&file)?;
    if before != after
        || bytes.len() != usize::try_from(before.length).unwrap_or(usize::MAX)
    {
        return Err("SCAN_FILE_CHANGED_DURING_READ".to_owned());
    }
    String::from_utf8(bytes).map_err(|_| "SCAN_INPUT_INVALID_UTF8".to_owned())
}

/// Process-local exclusive owner guard for one canonical data root.
pub(crate) struct DataRootGuard {
    file: File,
    canonical_root: PathBuf,
}

impl DataRootGuard {
    /// Acquires the OS file lock and writes an exact active-owner observation.
    pub(crate) fn acquire(path: &Path) -> Result<Self, String> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| format!("DATA_ROOT_OPEN_ERROR:{error}"))?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err("DATA_ROOT_LINK_DENIED".to_owned());
        }
        if !metadata.is_dir() {
            return Err("DATA_ROOT_NOT_DIRECTORY".to_owned());
        }
        let canonical_root = fs::canonicalize(path)
            .map_err(|error| format!("DATA_ROOT_CANONICALIZE_ERROR:{error}"))?;
        let canonical_metadata = fs::symlink_metadata(&canonical_root)
            .map_err(|error| format!("DATA_ROOT_OPEN_ERROR:{error}"))?;
        if canonical_metadata.file_type().is_symlink()
            || is_reparse(&canonical_metadata)
            || !canonical_metadata.is_dir()
        {
            return Err("DATA_ROOT_IDENTITY_AMBIGUOUS".to_owned());
        }

        let lock_path = canonical_root.join(".eliot-search-owner.lock");
        if lock_path.exists() {
            let lock_metadata = fs::symlink_metadata(&lock_path)
                .map_err(|error| format!("DATA_ROOT_LOCK_OPEN_ERROR:{error}"))?;
            if lock_metadata.file_type().is_symlink()
                || is_reparse(&lock_metadata)
                || !lock_metadata.is_file()
            {
                return Err("DATA_ROOT_LOCK_OBJECT_INVALID".to_owned());
            }
        }
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| format!("DATA_ROOT_LOCK_OPEN_ERROR:{error}"))?;
        let opened = file
            .metadata()
            .map_err(|error| format!("DATA_ROOT_LOCK_OPEN_ERROR:{error}"))?;
        if !opened.is_file() {
            return Err("DATA_ROOT_LOCK_OBJECT_INVALID".to_owned());
        }
        file.try_lock().map_err(|error| {
            if error.kind() == io::ErrorKind::WouldBlock {
                "DATA_ROOT_ALREADY_OWNED".to_owned()
            } else {
                format!("DATA_ROOT_LOCK_ERROR:{error}")
            }
        })?;

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "SYSTEM_CLOCK_BEFORE_EPOCH".to_owned())?
            .as_millis();
        let record = format!(
            "{{\"schema\":1,\"pid\":{},\"created_at_unix_ms\":{},\"state\":\"ACTIVE\"}}\n",
            std::process::id(),
            created_at,
        );
        if record.len() > 4 * 1024 {
            let _ = file.unlock();
            return Err("DATA_ROOT_OWNER_RECORD_TOO_LARGE".to_owned());
        }
        file.set_len(0)
            .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
            .and_then(|()| file.write_all(record.as_bytes()))
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("DATA_ROOT_OWNER_RECORD_ERROR:{error}"))?;

        Ok(Self {
            file,
            canonical_root,
        })
    }

    /// Canonical local root protected by this guard.
    pub(crate) fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }
}

impl Drop for DataRootGuard {
    fn drop(&mut self) {
        let _ = self.file.set_len(0);
        let _ = self.file.sync_all();
        let _ = self.file.unlock();
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &Metadata) -> bool {
    false
}
