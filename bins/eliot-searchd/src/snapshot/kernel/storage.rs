use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::fingerprint::{fingerprint, hex32};
use super::spec::TEMP_SEQUENCE;

pub(super) fn store_revision(
    revisions_root: &Path,
    revision_fingerprint: [u8; 32],
    bytes: &[u8],
    max_bytes: u64,
) -> io::Result<(PathBuf, bool)> {
    let digest = hex32(revision_fingerprint);
    let directory = revisions_root.join(&digest[..2]);
    fs::create_dir_all(&directory)?;
    let final_path = directory.join(format!("{digest}.utf8"));
    if final_path.exists() {
        verify_exact_file(&final_path, bytes, revision_fingerprint, max_bytes)?;
        return Ok((final_path, true));
    }

    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = directory.join(format!(
        ".{digest}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    match fs::rename(&temporary, &final_path) {
        Ok(()) => {}
        Err(_) if final_path.exists() => {
            let _ = fs::remove_file(&temporary);
            verify_exact_file(&final_path, bytes, revision_fingerprint, max_bytes)?;
            return Ok((final_path, true));
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    }
    #[cfg(unix)]
    sync_directory(&directory)?;
    #[cfg(not(unix))]
    sync_directory(&directory);
    verify_exact_file(&final_path, bytes, revision_fingerprint, max_bytes)?;
    Ok((final_path, false))
}

fn verify_exact_file(
    path: &Path,
    expected_bytes: &[u8],
    expected_fingerprint: [u8; 32],
    max_bytes: u64,
) -> io::Result<()> {
    let actual = read_bounded_file(path, max_bytes)?;
    if actual != expected_bytes || fingerprint(&actual) != expected_fingerprint {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "revision fingerprint collision or durable readback mismatch",
        ));
    }
    Ok(())
}

pub(super) fn read_verified_revision(
    path: &Path,
    expected_fingerprint: [u8; 32],
    expected_length: u64,
    max_bytes: u64,
) -> io::Result<Vec<u8>> {
    let bytes = read_bounded_file(path, max_bytes)?;
    if u64::try_from(bytes.len()).ok() != Some(expected_length)
        || fingerprint(&bytes) != expected_fingerprint
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "retained revision readback mismatch",
        ));
    }
    Ok(bytes)
}

fn read_bounded_file(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if super::policy::is_link_or_reparse(&metadata)
        || !metadata.is_file()
        || metadata.len() > max_bytes
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "invalid retained revision object",
        ));
    }
    let mut file = File::open(path)?;
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    (&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "retained revision exceeds its finite ceiling",
        ));
    }
    Ok(bytes)
}

pub(super) fn write_unique_verified(
    path: &Path,
    bytes: &[u8],
    max_bytes: u64,
) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(ErrorKind::InvalidInput, "manifest has no parent directory")
    })?;
    #[cfg(unix)]
    sync_directory(parent)?;
    #[cfg(not(unix))]
    sync_directory(parent);
    let readback = read_bounded_file(path, max_bytes)?;
    if readback != bytes {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "manifest durable readback mismatch",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
const fn sync_directory(_path: &Path) {}
