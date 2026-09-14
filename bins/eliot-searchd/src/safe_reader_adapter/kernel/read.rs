//! Whole-file translation through the shared safe-reader kernel.

use std::path::{Path, PathBuf};

use search_contracts::{Blake3Digest32, NonZeroRevision};
use search_safe_reader::{SafeReadBackend, SafeReadError};

use super::backend::FinalHandleBackend;
use super::identity::file_identity_digest;
use super::path::{DerivedLocator, derive_locator};
use super::spec::{
    ADAPTER_MAX_ATTEMPTS, ADAPTER_SINGLE_READ_BYTES, AdapterError,
};

/// Exact full-file product of one kernel-verified read.
#[derive(Debug)]
pub struct FullFileRead {
    /// Inert source bytes; never executed.
    pub bytes: Vec<u8>,
    /// Stable identity material in the historical DIRECT encoding, used by
    /// the caller to derive its retained identity digests.
    pub identity_material: Vec<u8>,
    /// Whether the material is a native stable identity rather than a
    /// path-bound fallback.
    pub identity_native: bool,
    /// Canonical final path proven inside the admitted root.
    pub canonical_final: PathBuf,
    /// Exact source byte size observed before and after the read.
    pub source_bytes: u64,
}

/// Translation failure: precise adapter denial or race-phase kernel denial.
///
/// Both sides are content-free; neither carries paths, bytes or raw OS text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FullReadError {
    /// Precheck/probe denial with the exact sub-cause.
    Adapter(AdapterError),
    /// Kernel denial from the race-phase re-open/read/revalidation.
    Kernel(SafeReadError),
}

impl FullReadError {
    /// Stable machine-readable reason code without content.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Adapter(error) => error.code(),
            Self::Kernel(error) => error.code(),
        }
    }
}

impl core::fmt::Display for FullReadError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for FullReadError {}

fn open_request(
    backend: &FinalHandleBackend,
    token: &str,
) -> Result<search_safe_reader::FinalHandleOpenRequest, FullReadError> {
    let relative_path = search_safe_reader::RelativePathToken::new(
        token,
        search_safe_reader::DEFAULT_SAFE_READ_LIMITS,
    )
    .map_err(|_| FullReadError::Adapter(AdapterError::PathDenied))?;
    Ok(search_safe_reader::FinalHandleOpenRequest {
        relative_path,
        expected_root_identity_digest: backend.root_digest(),
    })
}

fn kernel_limits(max_bytes: usize) -> Result<search_safe_reader::SafeReadLimits, FullReadError> {
    let ceiling = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    search_safe_reader::SafeReadLimits {
        max_source_bytes: ceiling,
        max_single_read_bytes: max_bytes,
        max_path_token_bytes: 32_768,
    }
    .validate()
    .map_err(|_| FullReadError::Adapter(AdapterError::TooLarge))
}

fn barrier() -> NonZeroRevision {
    NonZeroRevision::new(1).expect("non-zero barrier revision")
}

const fn map_factory_error(error: AdapterError) -> SafeReadError {
    match error {
        AdapterError::TooLarge => SafeReadError::SourceSizeInvalid,
        _ => SafeReadError::BackendFailure,
    }
}

/// Reads one exact file through the shared kernel under an admitted root.
///
/// `admitted_root_hint` is the containing directory the caller discovered
/// the file in. It is re-canonicalized inside. `max_bytes` bounds allocation
/// before any byte is read. Empty sources short-circuit through two agreeing
/// probe inspections; every other source crosses one bounded retry sequence.
pub fn read_full_file_via_kernel(
    source_path: &Path,
    admitted_root_hint: &Path,
    max_bytes: usize,
) -> Result<FullFileRead, FullReadError> {
    let locator =
        derive_locator(source_path, admitted_root_hint).map_err(FullReadError::Adapter)?;
    let (root_digest, file_digest, source_bytes) = probe_binding(&locator)?;
    if source_bytes == 0 {
        let second = probe_binding(&locator)?;
        if second != (root_digest, file_digest, source_bytes) {
            return Err(FullReadError::Kernel(
                SafeReadError::HandleChangedDuringRead,
            ));
        }
        let material = probe_material(&locator, file_digest)?;
        return Ok(FullFileRead {
            bytes: Vec::new(),
            identity_material: material.0,
            identity_native: material.1,
            canonical_final: locator.canonical_final,
            source_bytes: 0,
        });
    }
    if source_bytes > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
        return Err(FullReadError::Adapter(AdapterError::TooLarge));
    }
    let length = usize::try_from(source_bytes)
        .map_err(|_| FullReadError::Adapter(AdapterError::TooLarge))?;
    let token = search_safe_reader::RelativePathToken::new(
        &locator.token,
        search_safe_reader::DEFAULT_SAFE_READ_LIMITS,
    )
    .map_err(|_| FullReadError::Adapter(AdapterError::PathDenied))?;
    let request = search_safe_reader::SafeReadRequest {
        relative_path: token,
        expected_root_identity_digest: root_digest,
        expected_stable_file_identity_digest: file_digest,
        expected_security_barrier_revision: barrier(),
        range: search_safe_reader::ReadRange {
            offset: 0,
            length,
            require_eof: true,
        },
    };
    let limits = kernel_limits(max_bytes)?;
    let policy = search_safe_reader::SafeRetryPolicy::new(ADAPTER_MAX_ATTEMPTS)
        .map_err(|_| FullReadError::Adapter(AdapterError::PathDenied))?;
    let canonical_root = locator.canonical_root.clone();
    let token_text = locator.token.clone();
    let mut make_backend = || {
        FinalHandleBackend::bind(&canonical_root, &token_text, barrier(), max_bytes)
            .map_err(map_factory_error)
    };
    let mut never_cancel = || false;
    let result = search_safe_reader::safe_read_with_retries(
        &mut make_backend,
        &request,
        limits,
        policy,
        &mut never_cancel,
    )
    .map_err(FullReadError::Kernel)?;
    debug_assert_eq!(result.bytes().len(), length);
    let material = probe_material(&locator, file_digest)?;
    Ok(FullFileRead {
        bytes: result.bytes().to_vec(),
        identity_material: material.0,
        identity_native: material.1,
        canonical_final: locator.canonical_final,
        source_bytes,
    })
}

/// Opens once and inspects once to learn the stable binding; reads no bytes.
fn probe_binding(
    locator: &DerivedLocator,
) -> Result<(Blake3Digest32, Blake3Digest32, u64), FullReadError> {
    let mut backend = FinalHandleBackend::bind(
        &locator.canonical_root,
        &locator.token,
        barrier(),
        ADAPTER_SINGLE_READ_BYTES,
    )
    .map_err(FullReadError::Adapter)?;
    let handle = backend
        .open_final(&open_request(&backend, &locator.token)?)
        .map_err(FullReadError::Adapter)?;
    let metadata = backend.inspect(&handle).map_err(FullReadError::Adapter)?;
    if metadata.kind != search_safe_reader::FinalHandleKind::RegularFile {
        return Err(FullReadError::Adapter(AdapterError::FinalObjectInvalid));
    }
    if metadata.final_object_is_reparse || metadata.ancestor_reparse_observed {
        return Err(FullReadError::Adapter(AdapterError::LinkDenied));
    }
    Ok((
        metadata.root_identity_digest,
        metadata.stable_file_identity_digest,
        metadata.source_bytes,
    ))
}

/// Re-opens to fetch historical-encoding identity material after a successful
/// kernel read. A changed identity fails closed as a race-phase denial.
fn probe_material(
    locator: &DerivedLocator,
    expected_file_digest: Blake3Digest32,
) -> Result<(Vec<u8>, bool), FullReadError> {
    let mut backend = FinalHandleBackend::bind(
        &locator.canonical_root,
        &locator.token,
        barrier(),
        ADAPTER_SINGLE_READ_BYTES,
    )
    .map_err(FullReadError::Adapter)?;
    let handle = backend
        .open_final(&open_request(&backend, &locator.token)?)
        .map_err(FullReadError::Adapter)?;
    if file_identity_digest(&handle.identity_material) != expected_file_digest {
        return Err(FullReadError::Kernel(
            SafeReadError::HandleChangedDuringRead,
        ));
    }
    Ok((handle.identity_material.clone(), handle.identity_native))
}
