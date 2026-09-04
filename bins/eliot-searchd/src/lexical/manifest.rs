//! Frozen snapshot-manifest parsing and retained-revision readback for lexical indexing.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Read};
use std::path::{Path, PathBuf};

use crate::snapshot::{fingerprint, hex32};

const MANIFEST_HEADER: &str = "ELIOT_SEARCH_SNAPSHOT_V1";
const FINGERPRINT_ALGORITHM: &str = "eliot-fnv4-v1";
const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
const MAX_RELATIVE_PATH_BYTES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ManifestDocument {
    pub(super) root_index: usize,
    pub(super) relative_path: String,
    pub(super) revision_fingerprint: [u8; 32],
    pub(super) revision_path: PathBuf,
    pub(super) byte_length: u64,
    pub(super) line_count: usize,
}

pub(super) fn load_manifest_documents(
    data_root: &Path,
    manifest_path: &Path,
    expected_snapshot_id: &str,
    expected_manifest_fingerprint: [u8; 32],
    maximum_file_bytes: u64,
) -> io::Result<Vec<ManifestDocument>> {
    let bytes = read_bounded_file(
        manifest_path,
        u64::try_from(MAX_MANIFEST_BYTES).unwrap_or(u64::MAX),
    )?;
    if fingerprint(&bytes) != expected_manifest_fingerprint {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "snapshot manifest fingerprint mismatch",
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        io::Error::new(ErrorKind::InvalidData, "snapshot manifest is not UTF-8")
    })?;
    let mut lines = text.lines();
    if lines.next() != Some(MANIFEST_HEADER) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "snapshot manifest header mismatch",
        ));
    }

    let mut snapshot_id = None;
    let mut fingerprint_algorithm = None;
    let mut entry_count = None;
    let mut separator_seen = false;
    let mut entry_lines = Vec::new();
    for line in lines {
        if separator_seen {
            entry_lines.push(line);
            continue;
        }
        if line == "--" {
            separator_seen = true;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "malformed snapshot manifest field",
            ));
        };
        match key {
            "snapshot_id" => set_once(&mut snapshot_id, value.to_owned())?,
            "fingerprint_algorithm" => {
                set_once(&mut fingerprint_algorithm, value.to_owned())?;
            }
            "entries" => {
                set_once(
                    &mut entry_count,
                    value.parse::<usize>().map_err(|_| {
                        io::Error::new(
                            ErrorKind::InvalidData,
                            "invalid snapshot manifest entry count",
                        )
                    })?,
                )?;
            }
            "source_roots"
            | "total_bytes"
            | "capture_truncated"
            | "skipped_links"
            | "skipped_policy"
            | "skipped_binary"
            | "unreadable_files"
            | "unstable_files" => {}
            _ => {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "unknown snapshot manifest field",
                ));
            }
        }
    }
    if !separator_seen
        || snapshot_id.as_deref() != Some(expected_snapshot_id)
        || fingerprint_algorithm.as_deref() != Some(FINGERPRINT_ALGORITHM)
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "snapshot manifest identity mismatch",
        ));
    }
    let expected_count = entry_count.ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidData,
            "snapshot manifest entry count is missing",
        )
    })?;
    if entry_lines.len() != expected_count {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "snapshot manifest entry count mismatch",
        ));
    }

    let revisions_root = data_root.join("revisions").join(FINGERPRINT_ALGORITHM);
    let mut documents = Vec::with_capacity(expected_count);
    let mut identities = BTreeSet::new();
    for line in entry_lines {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 5 {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "malformed snapshot manifest entry",
            ));
        }
        let root_index = fields[0].parse::<usize>().map_err(|_| {
            io::Error::new(ErrorKind::InvalidData, "invalid root index")
        })?;
        let relative_path_bytes = decode_hex(fields[1])?;
        if relative_path_bytes.is_empty()
            || relative_path_bytes.len() > MAX_RELATIVE_PATH_BYTES
        {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "relative path exceeds its finite bounds",
            ));
        }
        let relative_path = String::from_utf8(relative_path_bytes).map_err(|_| {
            io::Error::new(ErrorKind::InvalidData, "relative path is not UTF-8")
        })?;
        if relative_path.starts_with('/')
            || relative_path.contains("../")
            || relative_path.contains("..\\")
            || relative_path.contains('\0')
        {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "relative path is not a safe locator",
            ));
        }
        let revision_fingerprint = decode_hex_32(fields[2])?;
        let byte_length = fields[3].parse::<u64>().map_err(|_| {
            io::Error::new(ErrorKind::InvalidData, "invalid revision length")
        })?;
        let line_count = fields[4].parse::<usize>().map_err(|_| {
            io::Error::new(ErrorKind::InvalidData, "invalid line count")
        })?;
        if byte_length > maximum_file_bytes {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "revision exceeds configured file ceiling",
            ));
        }
        let digest = hex32(revision_fingerprint);
        let revision_path = revisions_root
            .join(&digest[..2])
            .join(format!("{digest}.utf8"));
        let identity = (root_index, relative_path.clone());
        if !identities.insert(identity) {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "snapshot manifest repeats a source locator",
            ));
        }
        documents.push(ManifestDocument {
            root_index,
            relative_path,
            revision_fingerprint,
            revision_path,
            byte_length,
            line_count,
        });
    }
    if documents.windows(2).any(|pair| {
        (pair[0].root_index, pair[0].relative_path.as_str())
            >= (pair[1].root_index, pair[1].relative_path.as_str())
    }) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "snapshot manifest entries are not canonically ordered",
        ));
    }
    Ok(documents)
}

pub(super) fn read_retained_document(
    document: &ManifestDocument,
    maximum_file_bytes: u64,
) -> io::Result<String> {
    let bytes = read_bounded_file(&document.revision_path, maximum_file_bytes)?;
    if u64::try_from(bytes.len()).ok() != Some(document.byte_length)
        || fingerprint(&bytes) != document.revision_fingerprint
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "retained revision readback mismatch",
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| {
        io::Error::new(ErrorKind::InvalidData, "retained revision is not UTF-8")
    })?;
    let actual_lines = if text.is_empty() {
        0
    } else {
        text.as_bytes()
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            .saturating_add(1)
    };
    if actual_lines != document.line_count {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "retained revision line accounting mismatch",
        ));
    }
    Ok(text)
}

fn read_bounded_file(path: &Path, maximum_bytes: u64) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if is_link_or_reparse(&metadata)
        || !metadata.is_file()
        || metadata.len() > maximum_bytes
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "invalid retained lexical object",
        ));
    }
    let mut file = File::open(path)?;
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    (&mut file)
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "retained lexical object exceeds its finite ceiling",
        ));
    }
    Ok(bytes)
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> io::Result<()> {
    if slot.replace(value).is_some() {
        Err(io::Error::new(
            ErrorKind::InvalidData,
            "snapshot manifest repeats a singleton field",
        ))
    } else {
        Ok(())
    }
}

fn decode_hex_32(value: &str) -> io::Result<[u8; 32]> {
    let bytes = decode_hex(value)?;
    bytes.try_into().map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidData,
            "revision fingerprint must contain 32 bytes",
        )
    })
}

fn decode_hex(value: &str) -> io::Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "invalid hexadecimal field",
        ));
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        output.push((nibble(pair[0])? << 4) | nibble(pair[1])?);
    }
    Ok(output)
}

fn nibble(value: u8) -> io::Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(io::Error::new(
            ErrorKind::InvalidData,
            "invalid hexadecimal field",
        )),
    }
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    {
        false
    }
}
