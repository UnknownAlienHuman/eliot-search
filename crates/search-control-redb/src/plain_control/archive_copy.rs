//! Identity-checked bounded read-only copies of a closed, guarded committed CONTROL.

use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use super::{
    PlainControlError, check, copy_recovery_source, inspect_plain_control, map_io_error,
    open_recovery_source, verify_recovery_source,
};

/// Exact copied CONTROL bytes, independent of root paths or writable authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlCopy {
    /// Complete committed file length.
    pub byte_length: u64,
    /// BLAKE3 of every copied file byte, including the engine's committed representation.
    pub digest: [u8; 32],
}

fn hash_file(
    file: &mut File,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ControlCopy, PlainControlError> {
    check(deadline, cancel)?;
    file.rewind().map_err(|error| map_io_error(&error))?;
    let mut hash = blake3::Hasher::new();
    let mut bytes = 0_u64;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(64 * 1024)
        .map_err(|_| PlainControlError::Unavailable)?;
    buffer.resize(64 * 1024, 0_u8);
    loop {
        check(deadline, cancel)?;
        let count = file
            .read(&mut buffer)
            .map_err(|error| map_io_error(&error))?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(u64::try_from(count).map_err(|_| PlainControlError::Unavailable)?)
            .ok_or(PlainControlError::Unavailable)?;
        hash.update(&buffer[..count]);
    }
    check(deadline, cancel)?;
    Ok(ControlCopy {
        byte_length: bytes,
        digest: *hash.finalize().as_bytes(),
    })
}

/// Copies supported committed CONTROL into the caller's fresh empty owned file.
/// The caller holds exclusive source-root ownership and has closed its normal stores.
///
/// # Errors
/// Refuses unsupported/dirty CONTROL, a nonempty destination, changed source identity,
/// incomplete copies, I/O, cancellation and the original deadline. Never repairs either file.
pub fn copy_plain_control(
    source: &Path,
    destination: &mut File,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ControlCopy, PlainControlError> {
    inspect_plain_control(source, deadline, cancel)?;
    let mut original = open_recovery_source(source)?;
    let snapshot = original.metadata().map_err(|error| map_io_error(&error))?;
    verify_recovery_source(source, &original, &snapshot, deadline, cancel)?;
    copy_recovery_source(&mut original, destination, snapshot.len(), deadline, cancel)?;
    verify_recovery_source(source, &original, &snapshot, deadline, cancel)?;
    let expected = hash_file(&mut original, deadline, cancel)?;
    verify_recovery_source(source, &original, &snapshot, deadline, cancel)?;
    let copy = hash_file(destination, deadline, cancel)?;
    if copy.byte_length != snapshot.len() || copy != expected {
        return Err(PlainControlError::Busy);
    }
    Ok(copy)
}

/// Describes a copied CONTROL from its own bytes using only native read-only inspection.
///
/// # Errors
/// Refuses format/recovery-required state, changed identities, I/O or original-budget failure.
pub fn describe_plain_control(
    path: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ControlCopy, PlainControlError> {
    inspect_plain_control(path, deadline, cancel)?;
    let mut file = open_recovery_source(path)?;
    let snapshot = file.metadata().map_err(|error| map_io_error(&error))?;
    verify_recovery_source(path, &file, &snapshot, deadline, cancel)?;
    let copy = hash_file(&mut file, deadline, cancel)?;
    verify_recovery_source(path, &file, &snapshot, deadline, cancel)?;
    if copy.byte_length != snapshot.len() {
        return Err(PlainControlError::Busy);
    }
    Ok(copy)
}
