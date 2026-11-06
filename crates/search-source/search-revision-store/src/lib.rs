//! Immutable plain-object storage for exact source bytes.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::fs::{self, File};
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_contracts::Blake3Digest32;
use search_runtime_owner::{PlainRootError, PlainRootGuard};
pub use search_source_registry::MAX_SOURCE_OBJECT_BYTES;
use search_source_registry::{ObjectKind, ObjectRef, SOURCE_OBJECT_FORMAT_VERSION};

/// Closed immutable-object storage failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectStoreError {
    /// The object reference is unsupported or exceeds the current bound.
    InvalidReference,
    /// The source object is absent.
    Missing,
    /// The final object is not a regular file.
    NotRegularFile,
    /// The retained bytes failed length or BLAKE3 verification.
    IntegrityFailure,
    /// Allocation for a bounded retained read failed.
    AllocationFailed,
    /// No-clobber publication encountered different bytes.
    ImmutableConflict,
    /// The root publisher refused the file operation.
    PublishFailed,
    /// An object read failed.
    Io,
    /// The operation deadline expired.
    Deadline,
    /// Cancellation was observed.
    Cancelled,
}

impl ObjectStoreError {
    /// Stable content-free machine error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidReference => "OBJECT_REFERENCE_INVALID",
            Self::Missing => "OBJECT_MISSING",
            Self::NotRegularFile => "OBJECT_NOT_REGULAR_FILE",
            Self::IntegrityFailure => "OBJECT_INTEGRITY_FAILURE",
            Self::AllocationFailed => "OBJECT_ALLOCATION_FAILED",
            Self::ImmutableConflict => "OBJECT_IMMUTABLE_CONFLICT",
            Self::PublishFailed => "OBJECT_PUBLISH_FAILED",
            Self::Io => "OBJECT_IO",
            Self::Deadline => "OBJECT_DEADLINE",
            Self::Cancelled => "OBJECT_CANCELLED",
        }
    }
}

fn check(deadline: Instant, cancellation: &AtomicBool) -> Result<(), ObjectStoreError> {
    if cancellation.load(Ordering::Relaxed) {
        return Err(ObjectStoreError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ObjectStoreError::Deadline);
    }
    Ok(())
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

fn object_name(reference: ObjectRef) -> Result<String, ObjectStoreError> {
    if reference.format_version != SOURCE_OBJECT_FORMAT_VERSION
        || reference.kind != ObjectKind::Source
        || reference.byte_length > MAX_SOURCE_OBJECT_BYTES
    {
        return Err(ObjectStoreError::InvalidReference);
    }
    Ok(format!(
        "v{}-source-{}",
        reference.format_version,
        hex(reference.digest.as_bytes())
    ))
}

fn reference(bytes: &[u8]) -> Result<ObjectRef, ObjectStoreError> {
    let byte_length = u64::try_from(bytes.len()).map_err(|_| ObjectStoreError::InvalidReference)?;
    if byte_length > MAX_SOURCE_OBJECT_BYTES {
        return Err(ObjectStoreError::InvalidReference);
    }
    let digest = Blake3Digest32::from_bytes(*blake3::hash(bytes).as_bytes());
    Ok(ObjectRef::source(digest, byte_length))
}

/// Publishes bytes from the acquired buffer and independently verifies the final object.
///
/// # Errors
///
/// Returns an [`ObjectStoreError`] if the bytes exceed the object bound,
/// no-clobber publication fails, read-back verification fails, or cancellation
/// or the deadline is observed.
pub fn store_source_bytes(
    root: &PlainRootGuard,
    bytes: &[u8],
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<ObjectRef, ObjectStoreError> {
    check(deadline, cancellation)?;
    let reference = reference(bytes)?;
    let name = object_name(reference)?;
    let stage = format!(".{name}.stage");
    root.publish_object_noclobber(
        &name,
        &stage,
        bytes,
        MAX_SOURCE_OBJECT_BYTES,
        deadline,
        cancellation,
    )
    .map_err(|error| match error {
        PlainRootError::AlreadyExists => ObjectStoreError::ImmutableConflict,
        PlainRootError::Deadline => ObjectStoreError::Deadline,
        PlainRootError::Cancelled => ObjectStoreError::Cancelled,
        _ => ObjectStoreError::PublishFailed,
    })?;
    let readback = read_source_object(root, reference, deadline, cancellation)?;
    if u64::try_from(readback.len()).ok() != Some(reference.byte_length) {
        return Err(ObjectStoreError::IntegrityFailure);
    }
    Ok(reference)
}

/// Reads retained source bytes only after verifying their declared length and BLAKE3 digest.
///
/// # Errors
///
/// Returns an [`ObjectStoreError`] if the reference is invalid, the object is
/// missing or not a regular file, its bytes fail integrity verification, a
/// bounded read or allocation fails, or cancellation or the deadline is observed.
pub fn read_source_object(
    root: &PlainRootGuard,
    reference: ObjectRef,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<Vec<u8>, ObjectStoreError> {
    check(deadline, cancellation)?;
    let path = root.object_dir().join(object_name(reference)?);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            ObjectStoreError::Missing
        } else {
            ObjectStoreError::Io
        }
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ObjectStoreError::NotRegularFile);
    }
    if metadata.len() != reference.byte_length || metadata.len() > MAX_SOURCE_OBJECT_BYTES {
        return Err(ObjectStoreError::IntegrityFailure);
    }
    let file = File::open(path).map_err(|_| ObjectStoreError::Io)?;
    let mut reader = file.take(
        reference
            .byte_length
            .checked_add(1)
            .ok_or(ObjectStoreError::InvalidReference)?,
    );
    let mut bytes = Vec::new();
    let capacity =
        usize::try_from(reference.byte_length).map_err(|_| ObjectStoreError::InvalidReference)?;
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| ObjectStoreError::AllocationFailed)?;
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| ObjectStoreError::Io)?;
    if u64::try_from(bytes.len()).ok() != Some(reference.byte_length)
        || blake3::hash(&bytes).as_bytes() != reference.digest.as_bytes()
        || reader
            .get_ref()
            .metadata()
            .map_err(|_| ObjectStoreError::Io)?
            .len()
            != reference.byte_length
    {
        return Err(ObjectStoreError::IntegrityFailure);
    }
    check(deadline, cancellation)?;
    Ok(bytes)
}
