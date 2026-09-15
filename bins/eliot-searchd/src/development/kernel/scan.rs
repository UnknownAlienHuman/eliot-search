//! Bounded one-shot literal scanning and safe-reader translation.

use std::io::{self, Read};
use std::path::Path;

pub const MAX_SCAN_INPUT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_SCAN_QUERY_BYTES: usize = 64 * 1024;
pub const MAX_SCAN_MATCHES: usize = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanMatch {
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) line: usize,
    pub(crate) column_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanCoverage {
    pub(crate) input_bytes: usize,
    pub(crate) complete: bool,
    pub(crate) match_limit_reached: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanResult {
    pub(crate) matches: Vec<ScanMatch>,
    pub(crate) coverage: ScanCoverage,
}

/// Uses the same bounded literal engine as retained DIRECT preparation.
/// Only the historical one-shot LF/byte-coordinate projection belongs here.
pub fn scan_text(
    text: &str,
    query: &str,
    ascii_insensitive: bool,
) -> Result<ScanResult, String> {
    use search_exact::literal::{LiteralError, LiteralLimits, scan_chunks};

    let result = scan_chunks(
        &[text],
        query,
        ascii_insensitive,
        LiteralLimits {
            max_query_bytes: MAX_SCAN_QUERY_BYTES,
            max_input_bytes: MAX_SCAN_INPUT_BYTES,
            max_chunks: 1,
            max_matches: MAX_SCAN_MATCHES,
        },
    )
    .map_err(|error| {
        match error {
            LiteralError::EmptyQuery => "SCAN_QUERY_EMPTY",
            LiteralError::QueryTooLarge => "SCAN_QUERY_TOO_LARGE",
            LiteralError::InputTooLarge => "SCAN_INPUT_TOO_LARGE",
            other => other.code(),
        }
        .to_owned()
    })?;
    let coverage = ScanCoverage {
        input_bytes: result.input_bytes,
        complete: result.complete(),
        match_limit_reached: result.match_limit_reached,
    };
    let (mut consumed, mut line_start, mut line) = (0, 0, 0);
    let matches = result
        .matches
        .into_iter()
        .map(|range| {
            // Ranges arrive in increasing start order, including overlaps. Count
            // each prefix byte only once; do not allocate a table for every newline.
            // LF alone advances this legacy API's line coordinate. CR/NUL remain
            // source bytes; retained preparation keeps its own materializer policy.
            for (offset, byte) in text.as_bytes()[consumed..range.start].iter().enumerate() {
                if *byte == b'\n' {
                    line += 1;
                    line_start = consumed + offset + 1;
                }
            }
            consumed = range.start;
            ScanMatch {
                byte_start: range.start,
                byte_end: range.end,
                line,
                column_bytes: range.start - line_start,
            }
        })
        .collect();
    Ok(ScanResult { matches, coverage })
}

/// Reads bounded UTF-8 from standard input.
pub fn read_stdin_bounded() -> Result<String, String> {
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

/// Reads one regular non-link file through the shared safe-reader kernel.
///
/// The platform adapter proves final-object/ancestor containment on the
/// opened handle under the file's admitted parent directory and the kernel
/// revalidates the same handle after the read. Bytes are inert copies and
/// are never executed; failures carry closed codes without paths or raw OS
/// text.
pub fn read_file_bounded(path: &Path) -> Result<String, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| "SCAN_FILE_ACCESS_DENIED".to_owned())?
            .join(path)
    };
    let parent = absolute
        .parent()
        .ok_or_else(|| "SCAN_FILE_PATH_DENIED".to_owned())?;
    let full = crate::safe_reader_adapter::read_full_file_via_kernel(
        &absolute,
        parent,
        MAX_SCAN_INPUT_BYTES,
    )
    .map_err(map_full_read_error)?;
    if full.bytes.len() > MAX_SCAN_INPUT_BYTES
        || u64::try_from(full.bytes.len()).unwrap_or(u64::MAX) != full.source_bytes
    {
        return Err("SCAN_FILE_CHANGED_DURING_READ".to_owned());
    }
    String::from_utf8(full.bytes).map_err(|_| "SCAN_INPUT_INVALID_UTF8".to_owned())
}

/// Maps a kernel-verified read failure to the SCAN namespace without paths,
/// bytes or raw OS error text.
fn map_full_read_error(error: crate::safe_reader_adapter::FullReadError) -> String {
    use crate::safe_reader_adapter::{AdapterError, FullReadError};
    use search_safe_reader::SafeReadError;

    match error {
        FullReadError::Adapter(adapter) => match adapter {
            AdapterError::PathDenied => "SCAN_FILE_PATH_DENIED".to_owned(),
            AdapterError::LinkDenied | AdapterError::AncestorReparseDenied => {
                "SCAN_FILE_LINK_DENIED".to_owned()
            }
            AdapterError::EscapeDenied => "SCAN_FILE_ESCAPE_DENIED".to_owned(),
            AdapterError::RootRelocated => "SCAN_FILE_ROOT_RELOCATED".to_owned(),
            AdapterError::NotRegular => "SCAN_FILE_NOT_REGULAR".to_owned(),
            AdapterError::FinalObjectInvalid | AdapterError::DeviceDenied => {
                "SCAN_FILE_FINAL_OBJECT_INVALID".to_owned()
            }
            AdapterError::HardlinkDenied => "SCAN_FILE_HARDLINK_DENIED".to_owned(),
            AdapterError::AccessDenied => "SCAN_FILE_ACCESS_DENIED".to_owned(),
            AdapterError::TooLarge => "SCAN_INPUT_TOO_LARGE".to_owned(),
            AdapterError::ReceiptDenied => {
                "SCAN_FILE_METADATA_ERROR:SAFE_ADAPTER_RECEIPT_DENIED".to_owned()
            }
        },
        FullReadError::Kernel(kernel) => match kernel {
            SafeReadError::RangeOutsideSource
            | SafeReadError::EofMismatch
            | SafeReadError::ReadLengthMismatch
            | SafeReadError::StableIdentityMismatch
            | SafeReadError::HandleChangedDuringRead
            | SafeReadError::BackendFailure => "SCAN_FILE_CHANGED_DURING_READ".to_owned(),
            SafeReadError::RootIdentityMismatch => "SCAN_FILE_ESCAPE_DENIED".to_owned(),
            SafeReadError::UnsupportedFileKind => "SCAN_FILE_FINAL_OBJECT_INVALID".to_owned(),
            SafeReadError::ReparseBoundaryDenied => "SCAN_FILE_LINK_DENIED".to_owned(),
            SafeReadError::SecurityDenied | SafeReadError::SecurityRevisionMismatch => {
                "SCAN_FILE_ACCESS_DENIED".to_owned()
            }
            SafeReadError::SourceSizeInvalid => "SCAN_INPUT_TOO_LARGE".to_owned(),
            SafeReadError::Cancelled => "SCAN_FILE_READ_CANCELLED".to_owned(),
            SafeReadError::InvalidLimits
            | SafeReadError::InvalidPathToken
            | SafeReadError::InvalidReadLength
            | SafeReadError::RangeOverflow
            | SafeReadError::InvalidRetryPolicy
            | SafeReadError::ReceiptMissing => "SCAN_FILE_READ_INVALID".to_owned(),
        },
    }
}
