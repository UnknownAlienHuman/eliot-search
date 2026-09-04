//! Bounded same-handle source reading for the W2 direct-source spine.
//!
//! This package performs no filesystem I/O directly. A platform adapter opens
//! the final handle and supplies metadata/read operations. The algorithm checks
//! root, type, reparse/security state, stable identity, size, and range before
//! allocation, reads only from that handle, then re-inspects the same handle and
//! rejects any load-bearing change.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

use core::fmt;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

/// Conservative finite reader limits.
pub const DEFAULT_SAFE_READ_LIMITS: SafeReadLimits = SafeReadLimits {
    max_source_bytes: 8 * 1024 * 1024 * 1024,
    max_single_read_bytes: 8 * 1024 * 1024,
    max_path_token_bytes: 32_768,
};

/// Closed content-free safe-read failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SafeReadError {
    /// Reader limits are zero or internally inconsistent.
    InvalidLimits,
    /// Opaque relative path token is empty or exceeds its finite ceiling.
    InvalidPathToken,
    /// Requested read length is zero or exceeds its finite ceiling.
    InvalidReadLength,
    /// Offset plus length overflowed.
    RangeOverflow,
    /// Requested range exceeds exact pre-read file size.
    RangeOutsideSource,
    /// EOF was required but the requested range does not end at exact EOF.
    EofMismatch,
    /// Final handle does not belong to the expected logical root.
    RootIdentityMismatch,
    /// Final handle is not an ordinary file.
    UnsupportedFileKind,
    /// Final handle or an authoritative ancestor is a reparse/symlink boundary.
    ReparseBoundaryDenied,
    /// Live restrictive-security state denies the read.
    SecurityDenied,
    /// Security-barrier revision is stale.
    SecurityRevisionMismatch,
    /// Stable file identity differs from the admitted source binding.
    StableIdentityMismatch,
    /// Final source size is zero or exceeds the finite source ceiling.
    SourceSizeInvalid,
    /// Adapter returned fewer or more bytes than the exact requested length.
    ReadLengthMismatch,
    /// Same-handle metadata changed across the read.
    HandleChangedDuringRead,
    /// Adapter returned a malformed content-free receipt.
    ReceiptMissing,
    /// Platform adapter failed before a safe result existed.
    BackendFailure,
}

impl SafeReadError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "SAFE_READ_INVALID_LIMITS",
            Self::InvalidPathToken => "SAFE_READ_INVALID_PATH_TOKEN",
            Self::InvalidReadLength => "SAFE_READ_INVALID_LENGTH",
            Self::RangeOverflow => "SAFE_READ_RANGE_OVERFLOW",
            Self::RangeOutsideSource => "SAFE_READ_RANGE_OUTSIDE_SOURCE",
            Self::EofMismatch => "SAFE_READ_EOF_MISMATCH",
            Self::RootIdentityMismatch => "SAFE_READ_ROOT_IDENTITY_MISMATCH",
            Self::UnsupportedFileKind => "SAFE_READ_UNSUPPORTED_FILE_KIND",
            Self::ReparseBoundaryDenied => "SAFE_READ_REPARSE_BOUNDARY_DENIED",
            Self::SecurityDenied => "SAFE_READ_SECURITY_DENIED",
            Self::SecurityRevisionMismatch => "SAFE_READ_SECURITY_REVISION_MISMATCH",
            Self::StableIdentityMismatch => "SAFE_READ_STABLE_IDENTITY_MISMATCH",
            Self::SourceSizeInvalid => "SAFE_READ_SOURCE_SIZE_INVALID",
            Self::ReadLengthMismatch => "SAFE_READ_LENGTH_MISMATCH",
            Self::HandleChangedDuringRead => "SAFE_READ_HANDLE_CHANGED",
            Self::ReceiptMissing => "SAFE_READ_RECEIPT_MISSING",
            Self::BackendFailure => "SAFE_READ_BACKEND_FAILURE",
        }
    }
}

impl fmt::Display for SafeReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SafeReadError {}

/// Finite same-handle reader limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SafeReadLimits {
    /// Maximum accepted exact source size.
    pub max_source_bytes: u64,
    /// Maximum bytes returned by one read operation.
    pub max_single_read_bytes: usize,
    /// Maximum bytes in an opaque normalized relative-path token.
    pub max_path_token_bytes: usize,
}

impl SafeReadLimits {
    /// Validates every finite dimension as non-zero and representable.
    pub const fn validate(self) -> Result<Self, SafeReadError> {
        if self.max_source_bytes == 0
            || self.max_single_read_bytes == 0
            || self.max_path_token_bytes == 0
        {
            Err(SafeReadError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Opaque normalized relative-path token consumed by the platform adapter.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct RelativePathToken(String);

impl RelativePathToken {
    /// Creates a non-empty finite path token.
    ///
    /// Path normalization and platform path parsing are owned by the source
    /// identity/admission layer and concrete final-handle adapter. This type
    /// merely prevents empty or unbounded input at this boundary.
    pub fn new(value: impl Into<String>, limits: SafeReadLimits) -> Result<Self, SafeReadError> {
        let limits = limits.validate()?;
        let value = value.into();
        if value.is_empty()
            || value.len() > limits.max_path_token_bytes
            || value.as_bytes().contains(&0)
        {
            return Err(SafeReadError::InvalidPathToken);
        }
        Ok(Self(value))
    }

    /// Exact token text supplied to the platform adapter.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RelativePathToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RelativePathToken")
            .field(&self.0)
            .finish()
    }
}

/// Closed final-handle object kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FinalHandleKind {
    /// Ordinary file that may be read.
    RegularFile,
    /// Directory handle.
    Directory,
    /// Symlink or reparse-point object.
    ReparsePoint,
    /// Device, socket, pipe, or another unsupported object.
    Other,
}

/// Live restrictive-security disposition.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReadSecurityDisposition {
    /// Current barrier permits source reading.
    Permitted,
    /// Current barrier denies source reading.
    Restricted,
    /// Security evidence is contradictory or unresolved.
    Quarantined,
}

/// Complete content-free metadata for one final opened handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalHandleMetadata {
    /// Digest of stable logical root identity.
    pub root_identity_digest: Blake3Digest32,
    /// Stable final-handle identity digest.
    pub stable_file_identity_digest: Blake3Digest32,
    /// Closed handle kind.
    pub kind: FinalHandleKind,
    /// Exact source byte size.
    pub source_bytes: u64,
    /// Whether final object itself is a reparse/symlink boundary.
    pub final_object_is_reparse: bool,
    /// Whether an authoritative ancestor traversal crossed a reparse boundary.
    pub ancestor_reparse_observed: bool,
    /// Live security disposition.
    pub security_disposition: ReadSecurityDisposition,
    /// Exact restrictive-security barrier revision.
    pub security_barrier_revision: NonZeroRevision,
    /// Platform change token covering all metadata used by the adapter.
    pub change_token: OpaqueId,
    /// Content-free same-handle metadata receipt.
    pub metadata_receipt: Option<ReceiptRef>,
}

/// Exact finite range requested from the final handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadRange {
    /// Zero-based byte offset.
    pub offset: u64,
    /// Exact byte length.
    pub length: usize,
    /// Whether the range must end at exact EOF.
    pub require_eof: bool,
}

impl ReadRange {
    /// Validates finite length and checked range arithmetic.
    pub fn end(self, limits: SafeReadLimits) -> Result<u64, SafeReadError> {
        let limits = limits.validate()?;
        if self.length == 0 || self.length > limits.max_single_read_bytes {
            return Err(SafeReadError::InvalidReadLength);
        }
        let length = u64::try_from(self.length).map_err(|_| SafeReadError::RangeOverflow)?;
        self.offset
            .checked_add(length)
            .ok_or(SafeReadError::RangeOverflow)
    }
}

/// Complete exact source-read request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeReadRequest {
    /// Opaque normalized relative-path token.
    pub relative_path: RelativePathToken,
    /// Admitted stable logical-root digest.
    pub expected_root_identity_digest: Blake3Digest32,
    /// Admitted stable final-file identity digest.
    pub expected_stable_file_identity_digest: Blake3Digest32,
    /// Current restrictive-security barrier revision.
    pub expected_security_barrier_revision: NonZeroRevision,
    /// Exact finite byte range.
    pub range: ReadRange,
}

/// Content-free final-handle open request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalHandleOpenRequest {
    /// Opaque relative-path token.
    pub relative_path: RelativePathToken,
    /// Expected stable logical-root digest.
    pub expected_root_identity_digest: Blake3Digest32,
}

/// Exact bounded read response.
#[derive(Clone, Eq, PartialEq)]
pub struct SafeReadResult {
    /// Requested byte range.
    pub range: ReadRange,
    /// Exact finite bytes read from the same final handle.
    bytes: Vec<u8>,
    /// Stable final-file identity digest.
    pub stable_file_identity_digest: Blake3Digest32,
    /// Exact source byte size observed before and after the read.
    pub source_bytes: u64,
    /// Security-barrier revision that authorized the read.
    pub security_barrier_revision: NonZeroRevision,
    /// Pre-read same-handle metadata receipt.
    pub before_metadata_receipt: ReceiptRef,
    /// Post-read same-handle metadata receipt.
    pub after_metadata_receipt: ReceiptRef,
    /// Content-free exact read receipt supplied by the adapter.
    pub read_receipt: ReceiptRef,
}

impl SafeReadResult {
    /// Exact bounded bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Exact returned byte length.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether no bytes were returned.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl fmt::Debug for SafeReadResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SafeReadResult")
            .field("range", &self.range)
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .field(
                "stable_file_identity_digest",
                &self.stable_file_identity_digest,
            )
            .field("source_bytes", &self.source_bytes)
            .field(
                "security_barrier_revision",
                &self.security_barrier_revision,
            )
            .field("before_metadata_receipt", &self.before_metadata_receipt)
            .field("after_metadata_receipt", &self.after_metadata_receipt)
            .field("read_receipt", &self.read_receipt)
            .finish()
    }
}

/// One exact adapter read payload and content-free receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterRead {
    /// Exact finite bytes returned by the platform adapter.
    pub bytes: Vec<u8>,
    /// Content-free same-handle read receipt.
    pub read_receipt: Option<ReceiptRef>,
}

/// Platform adapter contract for same-final-handle reading.
pub trait SafeReadBackend {
    /// Opaque final handle that cannot be reopened by path inside this package.
    type Handle;
    /// Platform adapter failure.
    type BackendError;

    /// Opens the final object once under the expected logical root.
    fn open_final(
        &mut self,
        request: &FinalHandleOpenRequest,
    ) -> Result<Self::Handle, Self::BackendError>;

    /// Inspects load-bearing metadata from the exact already-open handle.
    fn inspect(
        &mut self,
        handle: &Self::Handle,
    ) -> Result<FinalHandleMetadata, Self::BackendError>;

    /// Reads the exact finite range from the same already-open handle.
    fn read_exact_at(
        &mut self,
        handle: &Self::Handle,
        offset: u64,
        length: usize,
    ) -> Result<AdapterRead, Self::BackendError>;

    /// Maps a platform error to a bounded content-free package failure.
    fn map_backend_error(error: &Self::BackendError) -> SafeReadError;
}

/// Opens once, validates, reads, and revalidates the same final handle.
pub fn safe_read<B: SafeReadBackend>(
    backend: &mut B,
    request: &SafeReadRequest,
    limits: SafeReadLimits,
) -> Result<SafeReadResult, SafeReadError> {
    let limits = limits.validate()?;
    let end = request.range.end(limits)?;
    let handle = backend
        .open_final(&FinalHandleOpenRequest {
            relative_path: request.relative_path.clone(),
            expected_root_identity_digest: request.expected_root_identity_digest,
        })
        .map_err(|error| B::map_backend_error(&error))?;
    let before = backend
        .inspect(&handle)
        .map_err(|error| B::map_backend_error(&error))?;
    validate_metadata(&before, request, end, limits)?;

    let read = backend
        .read_exact_at(&handle, request.range.offset, request.range.length)
        .map_err(|error| B::map_backend_error(&error))?;
    if read.bytes.len() != request.range.length {
        return Err(SafeReadError::ReadLengthMismatch);
    }
    let read_receipt = read.read_receipt.ok_or(SafeReadError::ReceiptMissing)?;

    let after = backend
        .inspect(&handle)
        .map_err(|error| B::map_backend_error(&error))?;
    validate_metadata(&after, request, end, limits)?;
    if before != after {
        return Err(SafeReadError::HandleChangedDuringRead);
    }
    let before_metadata_receipt = before
        .metadata_receipt
        .clone()
        .ok_or(SafeReadError::ReceiptMissing)?;
    let after_metadata_receipt = after
        .metadata_receipt
        .clone()
        .ok_or(SafeReadError::ReceiptMissing)?;

    Ok(SafeReadResult {
        range: request.range,
        bytes: read.bytes,
        stable_file_identity_digest: before.stable_file_identity_digest,
        source_bytes: before.source_bytes,
        security_barrier_revision: before.security_barrier_revision,
        before_metadata_receipt,
        after_metadata_receipt,
        read_receipt,
    })
}

fn validate_metadata(
    metadata: &FinalHandleMetadata,
    request: &SafeReadRequest,
    end: u64,
    limits: SafeReadLimits,
) -> Result<(), SafeReadError> {
    if metadata.root_identity_digest != request.expected_root_identity_digest {
        return Err(SafeReadError::RootIdentityMismatch);
    }
    if metadata.kind != FinalHandleKind::RegularFile {
        return Err(SafeReadError::UnsupportedFileKind);
    }
    if metadata.final_object_is_reparse || metadata.ancestor_reparse_observed {
        return Err(SafeReadError::ReparseBoundaryDenied);
    }
    match metadata.security_disposition {
        ReadSecurityDisposition::Permitted => {}
        ReadSecurityDisposition::Restricted | ReadSecurityDisposition::Quarantined => {
            return Err(SafeReadError::SecurityDenied);
        }
    }
    if metadata.security_barrier_revision
        != request.expected_security_barrier_revision
    {
        return Err(SafeReadError::SecurityRevisionMismatch);
    }
    if metadata.stable_file_identity_digest
        != request.expected_stable_file_identity_digest
    {
        return Err(SafeReadError::StableIdentityMismatch);
    }
    if metadata.source_bytes == 0 || metadata.source_bytes > limits.max_source_bytes {
        return Err(SafeReadError::SourceSizeInvalid);
    }
    if end > metadata.source_bytes {
        return Err(SafeReadError::RangeOutsideSource);
    }
    if request.range.require_eof && end != metadata.source_bytes {
        return Err(SafeReadError::EofMismatch);
    }
    if metadata.metadata_receipt.is_none() {
        return Err(SafeReadError::ReceiptMissing);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FakeError {
        Open,
        Inspect,
        Read,
    }

    struct FakeBackend {
        bytes: Vec<u8>,
        before: FinalHandleMetadata,
        after: FinalHandleMetadata,
        inspect_count: usize,
        fail: Option<FakeError>,
    }

    impl SafeReadBackend for FakeBackend {
        type Handle = u8;
        type BackendError = FakeError;

        fn open_final(
            &mut self,
            _request: &FinalHandleOpenRequest,
        ) -> Result<Self::Handle, Self::BackendError> {
            if self.fail == Some(FakeError::Open) {
                Err(FakeError::Open)
            } else {
                Ok(1)
            }
        }

        fn inspect(
            &mut self,
            _handle: &Self::Handle,
        ) -> Result<FinalHandleMetadata, Self::BackendError> {
            if self.fail == Some(FakeError::Inspect) {
                return Err(FakeError::Inspect);
            }
            let metadata = if self.inspect_count == 0 {
                self.before.clone()
            } else {
                self.after.clone()
            };
            self.inspect_count += 1;
            Ok(metadata)
        }

        fn read_exact_at(
            &mut self,
            _handle: &Self::Handle,
            offset: u64,
            length: usize,
        ) -> Result<AdapterRead, Self::BackendError> {
            if self.fail == Some(FakeError::Read) {
                return Err(FakeError::Read);
            }
            let start = usize::try_from(offset).map_err(|_| FakeError::Read)?;
            let end = start.checked_add(length).ok_or(FakeError::Read)?;
            let bytes = self.bytes.get(start..end).ok_or(FakeError::Read)?.to_vec();
            Ok(AdapterRead {
                bytes,
                read_receipt: Some(ReceiptRef::new("receipt:read").expect("receipt")),
            })
        }

        fn map_backend_error(_error: &Self::BackendError) -> SafeReadError {
            SafeReadError::BackendFailure
        }
    }

    fn metadata() -> FinalHandleMetadata {
        FinalHandleMetadata {
            root_identity_digest: Blake3Digest32::from_bytes([1; 32]),
            stable_file_identity_digest: Blake3Digest32::from_bytes([2; 32]),
            kind: FinalHandleKind::RegularFile,
            source_bytes: 6,
            final_object_is_reparse: false,
            ancestor_reparse_observed: false,
            security_disposition: ReadSecurityDisposition::Permitted,
            security_barrier_revision: NonZeroRevision::new(3).expect("revision"),
            change_token: OpaqueId::new("change:one").expect("token"),
            metadata_receipt: Some(
                ReceiptRef::new("receipt:metadata").expect("receipt"),
            ),
        }
    }

    fn request() -> SafeReadRequest {
        SafeReadRequest {
            relative_path: RelativePathToken::new(
                "src/lib.rs",
                DEFAULT_SAFE_READ_LIMITS,
            )
            .expect("path"),
            expected_root_identity_digest: Blake3Digest32::from_bytes([1; 32]),
            expected_stable_file_identity_digest: Blake3Digest32::from_bytes([2; 32]),
            expected_security_barrier_revision: NonZeroRevision::new(3)
                .expect("revision"),
            range: ReadRange {
                offset: 1,
                length: 3,
                require_eof: false,
            },
        }
    }

    fn backend() -> FakeBackend {
        FakeBackend {
            bytes: b"abcdef".to_vec(),
            before: metadata(),
            after: metadata(),
            inspect_count: 0,
            fail: None,
        }
    }

    #[test]
    fn exact_same_handle_read_succeeds() {
        let result = safe_read(&mut backend(), &request(), DEFAULT_SAFE_READ_LIMITS)
            .expect("read");
        assert_eq!(result.bytes(), b"bcd");
    }

    #[test]
    fn stable_identity_mismatch_is_denied_before_read() {
        let mut request = request();
        request.expected_stable_file_identity_digest = Blake3Digest32::from_bytes([9; 32]);
        assert_eq!(
            safe_read(&mut backend(), &request, DEFAULT_SAFE_READ_LIMITS),
            Err(SafeReadError::StableIdentityMismatch)
        );
    }

    #[test]
    fn reparse_boundary_is_denied() {
        let mut backend = backend();
        backend.before.ancestor_reparse_observed = true;
        assert_eq!(
            safe_read(&mut backend, &request(), DEFAULT_SAFE_READ_LIMITS),
            Err(SafeReadError::ReparseBoundaryDenied)
        );
    }

    #[test]
    fn read_range_is_checked_before_adapter_read() {
        let mut request = request();
        request.range = ReadRange {
            offset: 5,
            length: 2,
            require_eof: false,
        };
        assert_eq!(
            safe_read(&mut backend(), &request, DEFAULT_SAFE_READ_LIMITS),
            Err(SafeReadError::RangeOutsideSource)
        );
    }

    #[test]
    fn exact_eof_requirement_is_enforced() {
        let mut request = request();
        request.range.require_eof = true;
        assert_eq!(
            safe_read(&mut backend(), &request, DEFAULT_SAFE_READ_LIMITS),
            Err(SafeReadError::EofMismatch)
        );
    }

    #[test]
    fn same_handle_metadata_change_rejects_bytes() {
        let mut backend = backend();
        backend.after.change_token = OpaqueId::new("change:two").expect("token");
        assert_eq!(
            safe_read(&mut backend, &request(), DEFAULT_SAFE_READ_LIMITS),
            Err(SafeReadError::HandleChangedDuringRead)
        );
    }

    #[test]
    fn backend_failure_is_content_free() {
        let mut backend = backend();
        backend.fail = Some(FakeError::Open);
        assert_eq!(
            safe_read(&mut backend, &request(), DEFAULT_SAFE_READ_LIMITS),
            Err(SafeReadError::BackendFailure)
        );
    }

    #[test]
    fn result_debug_does_not_dump_source_bytes() {
        let result = safe_read(&mut backend(), &request(), DEFAULT_SAFE_READ_LIMITS)
            .expect("read");
        let debug = format!("{result:?}");
        assert!(!debug.contains("bcd"));
        assert!(debug.contains("<3 bytes>"));
    }
}
