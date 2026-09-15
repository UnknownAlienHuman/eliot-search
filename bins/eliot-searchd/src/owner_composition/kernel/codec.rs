//! Canonical scalar codec, digest framing and native path helpers.

use std::fs;
use std::path::Path;

use search_runtime_owner::OwnerError;

pub(super) fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

pub(super) fn push_field(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

pub(super) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn parse_hex_exact(value: Option<&str>, expected_bytes: usize) -> Result<Vec<u8>, OwnerError> {
    let Some(value) = value else {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    };
    if value.len() != expected_bytes * 2
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let mut output = Vec::with_capacity(expected_bytes);
    for pair in value.as_bytes().chunks(2) {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

const fn hex_value(byte: u8) -> Result<u8, OwnerError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(OwnerError::OwnerRecoveryQuarantined),
    }
}

pub(super) fn parse_id16(value: Option<&str>) -> Result<[u8; 16], OwnerError> {
    parse_hex_exact(value, 16)?
        .try_into()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
}

pub(super) fn parse_digest32(value: Option<&str>) -> Result<[u8; 32], OwnerError> {
    parse_hex_exact(value, 32)?
        .try_into()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
}

pub(super) fn parse_u64(value: Option<&str>) -> Result<u64, OwnerError> {
    let Some(value) = value else {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    };
    if value.is_empty()
        || value.starts_with('+')
        || value.starts_with('-')
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    value
        .parse::<u64>()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
}

pub(super) fn parse_u32(value: Option<&str>) -> Result<u32, OwnerError> {
    let Some(value) = value else {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    };
    if value.is_empty()
        || value.starts_with('+')
        || value.starts_with('-')
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    value
        .parse::<u32>()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
}

pub(super) fn blake3_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytes);
    let finalized = hasher.finalize();
    let mut output = [0_u8; 32];
    output.copy_from_slice(finalized.as_bytes());
    output
}

pub(super) fn blake3_hex(bytes: &[u8]) -> String {
    hex(&blake3_bytes(bytes))
}

pub(super) fn domain_digest(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(part);
    }
    let finalized = hasher.finalize();
    let mut output = [0_u8; 32];
    output.copy_from_slice(finalized.as_bytes());
    output
}

pub(super) fn canonical_path_bytes(canonical_root: &Path) -> Vec<u8> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        canonical_root
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect()
    }
    #[cfg(all(unix, not(windows)))]
    {
        use std::os::unix::ffi::OsStrExt;
        canonical_root.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(any(unix, windows)))]
    {
        canonical_root.to_string_lossy().as_bytes().to_vec()
    }
}

pub(super) fn native_volume_material(canonical_root: &Path) -> Result<Vec<u8>, OwnerError> {
    #[cfg(windows)]
    {
        use std::fs::OpenOptions;
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        let handle = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(canonical_root)
            .map_err(|_| OwnerError::DataRootInvalid)?;
        let metadata = handle.metadata().map_err(|_| OwnerError::DataRootInvalid)?;
        if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(OwnerError::DataRootInvalid);
        }
        let observed = eliot_searchd::native_file::observe(&handle)
            .map_err(|_| OwnerError::DataRootInvalid)?;
        Ok(observed.legacy_identity_bytes().to_vec())
    }
    #[cfg(all(unix, not(windows)))]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(canonical_root).map_err(|_| OwnerError::DataRootInvalid)?;
        let mut material = Vec::with_capacity(16);
        material.extend_from_slice(&metadata.dev().to_be_bytes());
        material.extend_from_slice(&metadata.ino().to_be_bytes());
        Ok(material)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = canonical_root;
        Ok(Vec::new())
    }
}

#[cfg(windows)]
pub(super) fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(super) fn is_reparse(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) {
    let _ = fs::File::open(path).and_then(|file| file.sync_all());
}

#[cfg(not(unix))]
pub(super) const fn sync_directory(_path: &Path) {}
