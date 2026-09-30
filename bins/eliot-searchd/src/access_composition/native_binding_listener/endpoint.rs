//! Authenticated native endpoint publication bound to the root-owned listener.
//!
//! The descriptor is public bootstrap metadata, not authority. Publication occurs
//! only after durable standalone registration bootstrap and loopback bind. The
//! exact registered-generation key signs every descriptor field, while the key
//! itself never leaves the existing native credential owner.

use core::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use search_contracts::{
    Blake3Digest32, OpaqueRef, ProtocolRange, ProtocolVersion, protocol::PeerRole,
};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{
    NativeEndpointDescriptorV1, ProofDigest, ProtocolError, TransportPeer,
    MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES, encode_native_endpoint_descriptor,
    native_endpoint_descriptor_transcript,
};

use super::super::{
    NativeBindingError, NativeBindingExpectation, NativePairingCredentialError,
    ProviderBindingRecord, StandaloneProcessOwner,
};

const RUNTIME_DIRECTORY: &str = "runtime";
const DESCRIPTOR_NAME: &str = "native-endpoint.v1";
const TEMP_ATTEMPTS: usize = 32;
const NATIVE_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Closed failure while signing, publishing or removing one native endpoint descriptor.
///
/// Filesystem variants intentionally retain no path or raw operating-system text.
pub enum StandaloneEndpointPublicationError {
    /// The listener address or protocol range cannot represent the native endpoint.
    InvalidConfiguration,
    /// The original cancellation/deadline budget was exhausted.
    Interrupted,
    /// Current durable registration did not match the retained native expectation.
    Binding(NativeBindingError),
    /// The exact registered-generation credential could not be read.
    Credential(NativePairingCredentialError),
    /// Canonical descriptor construction or encoding failed.
    Protocol(ProtocolError),
    /// The runtime directory or descriptor could not be observed or created.
    FilesystemUnavailable,
    /// A runtime path component or descriptor was a link, reparse point or wrong type.
    FilesystemObjectInvalid,
    /// A runtime path escaped the canonical root protected by this process.
    OutsideRoot,
    /// The descriptor exceeded its fixed protocol ceiling.
    TooLarge,
    /// Atomic replacement or its durability acknowledgement had an unknown outcome.
    PublicationOutcomeUnknown,
    /// Exact post-publication bytes did not match the signed descriptor.
    ReadbackMismatch,
    /// Shutdown found a different descriptor at the owned path and did not remove it.
    CleanupConflict,
    /// Descriptor removal or its durability acknowledgement had an unknown outcome.
    CleanupOutcomeUnknown,
}

impl StandaloneEndpointPublicationError {
    /// Stable content-free reason code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "NATIVE_ENDPOINT_PUBLICATION_INVALID",
            Self::Interrupted => "NATIVE_ENDPOINT_PUBLICATION_INTERRUPTED",
            Self::Binding(_) => "NATIVE_ENDPOINT_PUBLICATION_BINDING_REFUSED",
            Self::Credential(_) => "NATIVE_ENDPOINT_PUBLICATION_CREDENTIAL_REFUSED",
            Self::Protocol(_) => "NATIVE_ENDPOINT_PUBLICATION_PROTOCOL_INVALID",
            Self::FilesystemUnavailable => "NATIVE_ENDPOINT_PUBLICATION_FILESYSTEM_UNAVAILABLE",
            Self::FilesystemObjectInvalid => "NATIVE_ENDPOINT_PUBLICATION_OBJECT_INVALID",
            Self::OutsideRoot => "NATIVE_ENDPOINT_PUBLICATION_OUTSIDE_ROOT",
            Self::TooLarge => "NATIVE_ENDPOINT_PUBLICATION_TOO_LARGE",
            Self::PublicationOutcomeUnknown => "NATIVE_ENDPOINT_PUBLICATION_OUTCOME_UNKNOWN",
            Self::ReadbackMismatch => "NATIVE_ENDPOINT_PUBLICATION_READBACK_MISMATCH",
            Self::CleanupConflict => "NATIVE_ENDPOINT_CLEANUP_CONFLICT",
            Self::CleanupOutcomeUnknown => "NATIVE_ENDPOINT_CLEANUP_OUTCOME_UNKNOWN",
        }
    }
}

impl fmt::Debug for StandaloneEndpointPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("StandaloneEndpointPublicationError")
            .field(&self.code())
            .finish()
    }
}

impl fmt::Display for StandaloneEndpointPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for StandaloneEndpointPublicationError {}

impl From<NativeBindingError> for StandaloneEndpointPublicationError {
    fn from(error: NativeBindingError) -> Self {
        Self::Binding(error)
    }
}

impl From<NativePairingCredentialError> for StandaloneEndpointPublicationError {
    fn from(error: NativePairingCredentialError) -> Self {
        Self::Credential(error)
    }
}

impl From<ProtocolError> for StandaloneEndpointPublicationError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

/// One absolute setup budget shared by bind, credential read and descriptor publication.
pub(super) struct NativeEndpointPublicationBudget {
    deadline: Instant,
}

impl NativeEndpointPublicationBudget {
    pub(super) fn new<C: CancellationProbe>(
        context: &OperationContext<C>,
    ) -> Result<Self, StandaloneEndpointPublicationError> {
        if context.cancellation().is_cancelled() {
            return Err(StandaloneEndpointPublicationError::Interrupted);
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(context.relative_deadline_ms().get()))
            .ok_or(StandaloneEndpointPublicationError::Interrupted)?;
        Ok(Self { deadline })
    }

    pub(super) fn check<C: CancellationProbe>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<Duration, StandaloneEndpointPublicationError> {
        if context.cancellation().is_cancelled() {
            return Err(StandaloneEndpointPublicationError::Interrupted);
        }
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
            .ok_or(StandaloneEndpointPublicationError::Interrupted)
    }

    fn remaining_context<C: CancellationProbe + Clone>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<OperationContext<C>, StandaloneEndpointPublicationError> {
        let remaining = self.check(context)?;
        let millis = u64::try_from(remaining.as_millis())
            .ok()
            .filter(|value| *value > 0)
            .ok_or(StandaloneEndpointPublicationError::Interrupted)?;
        OperationContext::new(
            context.request_id(),
            millis,
            context.cancellation().clone(),
            context.budget_ref().clone(),
        )
        .map_err(|_| StandaloneEndpointPublicationError::Interrupted)
    }
}

/// Owned exact descriptor path and bytes, removed before the process/root owner drops.
pub(super) struct PublishedNativeEndpoint {
    runtime_directory: PathBuf,
    path: PathBuf,
    bytes: Vec<u8>,
    pairing_proof_ref: OpaqueRef,
    requested_capability_digest: Option<Blake3Digest32>,
    removed: bool,
}

impl PublishedNativeEndpoint {
    pub(super) fn pairing_proof_ref(&self) -> &OpaqueRef {
        &self.pairing_proof_ref
    }

    pub(super) const fn requested_capability_digest(&self) -> Option<Blake3Digest32> {
        self.requested_capability_digest
    }

    pub(super) fn remove(
        mut self,
    ) -> Result<(), StandaloneEndpointPublicationError> {
        let result = remove_exact_descriptor(
            &self.runtime_directory,
            &self.path,
            &self.bytes,
        );
        if result.is_ok() {
            self.removed = true;
        }
        result
    }
}

impl Drop for PublishedNativeEndpoint {
    fn drop(&mut self) {
        if !self.removed
            && remove_exact_descriptor(
                &self.runtime_directory,
                &self.path,
                &self.bytes,
            )
            .is_ok()
        {
            self.removed = true;
        }
    }
}

impl fmt::Debug for PublishedNativeEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublishedNativeEndpoint")
            .field("published", &!self.removed)
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

/// Sign and atomically publish the descriptor for the already-bound listener.
#[allow(clippy::too_many_arguments)]
pub(super) fn publish_native_endpoint<C: CancellationProbe + Clone>(
    process: &StandaloneProcessOwner,
    address: SocketAddr,
    local_protocols: ProtocolRange,
    pairing_proof_ref: OpaqueRef,
    requested_capability_digest: Option<Blake3Digest32>,
    budget: &NativeEndpointPublicationBudget,
    context: &OperationContext<C>,
) -> Result<PublishedNativeEndpoint, StandaloneEndpointPublicationError> {
    budget.check(context)?;
    if NATIVE_PROTOCOL_VERSION < local_protocols.minimum
        || NATIVE_PROTOCOL_VERSION > local_protocols.maximum
    {
        return Err(StandaloneEndpointPublicationError::InvalidConfiguration);
    }
    let SocketAddr::V4(address) = address else {
        return Err(StandaloneEndpointPublicationError::InvalidConfiguration);
    };
    if !address.ip().is_loopback() || address.port() == 0 {
        return Err(StandaloneEndpointPublicationError::InvalidConfiguration);
    }

    let record = process.readiness().record();
    let expected = process.readiness().expectation();
    let peer = TransportPeer {
        role: record.peer_role,
        incarnation: record.installation_incarnation_id,
        binding: record.binding_id,
    };
    if record.peer_role != PeerRole::StandaloneCli {
        return Err(StandaloneEndpointPublicationError::InvalidConfiguration);
    }
    expected.validate_registration(record, &peer)?;

    let key_context = budget.remaining_context(context)?;
    let key = expected.load_pairing_key(&peer, &key_context)?;
    budget.check(context)?;

    let unsigned = descriptor(
        address,
        record,
        expected,
        pairing_proof_ref.clone(),
        requested_capability_digest,
        ProofDigest::from_bytes([0; 32]),
    )?;
    let transcript = native_endpoint_descriptor_transcript(&unsigned)?;
    let proof = key.with_bytes(|bytes| {
        crate::secret_composition::pairing_keyed_proof_raw(bytes, &transcript)
    });
    drop(key);
    let signed = descriptor(
        address,
        record,
        expected,
        pairing_proof_ref.clone(),
        requested_capability_digest,
        proof,
    )?;
    let bytes = encode_native_endpoint_descriptor(&signed)?;
    if bytes.len() > MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES {
        return Err(StandaloneEndpointPublicationError::TooLarge);
    }
    budget.check(context)?;

    let canonical_root = process.canonical_root();
    let runtime_directory = ensure_runtime_directory(canonical_root)?;
    let path = runtime_directory.join(DESCRIPTOR_NAME);
    publish_exact_descriptor(
        canonical_root,
        &runtime_directory,
        &path,
        &bytes,
        budget,
        context,
    )?;

    Ok(PublishedNativeEndpoint {
        runtime_directory,
        path,
        bytes,
        pairing_proof_ref,
        requested_capability_digest,
        removed: false,
    })
}

fn descriptor(
    address: SocketAddrV4,
    record: &ProviderBindingRecord,
    expected: &NativeBindingExpectation,
    pairing_proof_ref: OpaqueRef,
    requested_capability_digest: Option<Blake3Digest32>,
    proof: ProofDigest,
) -> Result<NativeEndpointDescriptorV1, ProtocolError> {
    NativeEndpointDescriptorV1::new(
        address,
        record.installation_id,
        record.installation_incarnation_id,
        record.binding_id,
        record.peer_identity_digest,
        record.pairing_generation,
        expected.profile_id.clone(),
        expected.disclosure_ceiling_ref.clone(),
        pairing_proof_ref,
        requested_capability_digest,
        proof,
    )
}

fn ensure_runtime_directory(
    canonical_root: &Path,
) -> Result<PathBuf, StandaloneEndpointPublicationError> {
    let path = canonical_root.join(RUNTIME_DIRECTORY);
    let created = match fs::create_dir(&path) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(_) => return Err(StandaloneEndpointPublicationError::FilesystemUnavailable),
    };
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| StandaloneEndpointPublicationError::FilesystemUnavailable)?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err(StandaloneEndpointPublicationError::FilesystemObjectInvalid);
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|_| StandaloneEndpointPublicationError::FilesystemUnavailable)?;
    if canonical.parent() != Some(canonical_root) || !canonical.starts_with(canonical_root) {
        return Err(StandaloneEndpointPublicationError::OutsideRoot);
    }
    if created {
        sync_directory(canonical_root)
            .map_err(|_| StandaloneEndpointPublicationError::FilesystemUnavailable)?;
    }
    Ok(canonical)
}

fn publish_exact_descriptor<C: CancellationProbe>(
    canonical_root: &Path,
    runtime_directory: &Path,
    path: &Path,
    bytes: &[u8],
    budget: &NativeEndpointPublicationBudget,
    context: &OperationContext<C>,
) -> Result<(), StandaloneEndpointPublicationError> {
    budget.check(context)?;
    if descriptor_matches_for_publication(canonical_root, path, bytes)? {
        budget.check(context)?;
        return Ok(());
    }

    let (temporary, mut file) = create_temporary(runtime_directory)?;
    let mut temporary_guard = TemporaryDescriptor::new(temporary.clone());
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| StandaloneEndpointPublicationError::PublicationOutcomeUnknown)?;
    drop(file);
    budget.check(context)?;

    let installed = match fs::rename(&temporary, path) {
        Ok(()) => {
            temporary_guard.disarm();
            true
        }
        Err(_) => descriptor_matches_for_publication(canonical_root, path, bytes)
            .unwrap_or(false),
    };
    if !installed {
        return Err(StandaloneEndpointPublicationError::PublicationOutcomeUnknown);
    }

    let completed = (|| {
        sync_directory(runtime_directory)
            .map_err(|_| StandaloneEndpointPublicationError::PublicationOutcomeUnknown)?;
        budget.check(context)?;
        let observed = read_exact_descriptor(canonical_root, path)?;
        budget.check(context)?;
        match observed {
            Some(stored) if stored.as_slice() == bytes => Ok(()),
            Some(_) | None => Err(StandaloneEndpointPublicationError::ReadbackMismatch),
        }
    })();
    if completed.is_err() {
        let _ = remove_exact_descriptor(runtime_directory, path, bytes);
    }
    completed
}

fn descriptor_matches_for_publication(
    canonical_root: &Path,
    path: &Path,
    expected: &[u8],
) -> Result<bool, StandaloneEndpointPublicationError> {
    match read_exact_descriptor(canonical_root, path) {
        Ok(Some(stored)) => Ok(stored.as_slice() == expected),
        Ok(None) | Err(StandaloneEndpointPublicationError::TooLarge) => Ok(false),
        Err(error) => Err(error),
    }
}

fn create_temporary(
    runtime_directory: &Path,
) -> Result<(PathBuf, File), StandaloneEndpointPublicationError> {
    for _ in 0..TEMP_ATTEMPTS {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(
            ".{DESCRIPTOR_NAME}.tmp-{}-{sequence:016x}",
            std::process::id(),
        );
        let path = runtime_directory.join(name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => {
                return Err(StandaloneEndpointPublicationError::FilesystemUnavailable);
            }
        }
    }
    Err(StandaloneEndpointPublicationError::FilesystemUnavailable)
}

fn read_exact_descriptor(
    canonical_root: &Path,
    path: &Path,
) -> Result<Option<Vec<u8>>, StandaloneEndpointPublicationError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(StandaloneEndpointPublicationError::FilesystemUnavailable),
    };
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err(StandaloneEndpointPublicationError::FilesystemObjectInvalid);
    }
    let canonical = fs::canonicalize(path)
        .map_err(|_| StandaloneEndpointPublicationError::FilesystemUnavailable)?;
    if !canonical.starts_with(canonical_root) {
        return Err(StandaloneEndpointPublicationError::OutsideRoot);
    }
    let maximum = u64::try_from(MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES)
        .map_err(|_| StandaloneEndpointPublicationError::TooLarge)?;
    if metadata.len() > maximum {
        return Err(StandaloneEndpointPublicationError::TooLarge);
    }

    let mut file = File::open(&canonical)
        .map_err(|_| StandaloneEndpointPublicationError::FilesystemUnavailable)?;
    let opened = file
        .metadata()
        .map_err(|_| StandaloneEndpointPublicationError::FilesystemUnavailable)?;
    if !opened.is_file() {
        return Err(StandaloneEndpointPublicationError::FilesystemObjectInvalid);
    }
    if opened.len() > maximum {
        return Err(StandaloneEndpointPublicationError::TooLarge);
    }
    let allowance = MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES
        .checked_add(1)
        .ok_or(StandaloneEndpointPublicationError::TooLarge)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(allowance)
        .map_err(|_| StandaloneEndpointPublicationError::TooLarge)?;
    file.take(
        u64::try_from(allowance)
            .map_err(|_| StandaloneEndpointPublicationError::TooLarge)?,
    )
    .read_to_end(&mut bytes)
    .map_err(|_| StandaloneEndpointPublicationError::FilesystemUnavailable)?;
    if bytes.len() > MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES {
        return Err(StandaloneEndpointPublicationError::TooLarge);
    }
    Ok(Some(bytes))
}

fn remove_exact_descriptor(
    runtime_directory: &Path,
    path: &Path,
    expected: &[u8],
) -> Result<(), StandaloneEndpointPublicationError> {
    let canonical_root = runtime_directory
        .parent()
        .ok_or(StandaloneEndpointPublicationError::OutsideRoot)?;
    let observed = match read_exact_descriptor(canonical_root, path) {
        Ok(observed) => observed,
        Err(
            StandaloneEndpointPublicationError::FilesystemObjectInvalid
            | StandaloneEndpointPublicationError::OutsideRoot
            | StandaloneEndpointPublicationError::TooLarge,
        ) => return Err(StandaloneEndpointPublicationError::CleanupConflict),
        Err(_) => return Err(StandaloneEndpointPublicationError::CleanupOutcomeUnknown),
    };
    match observed {
        None => return Ok(()),
        Some(stored) if stored.as_slice() == expected => {}
        Some(_) => return Err(StandaloneEndpointPublicationError::CleanupConflict),
    }
    if fs::remove_file(path).is_err() {
        return match read_exact_descriptor(canonical_root, path) {
            Ok(None) => Ok(()),
            Ok(Some(stored)) if stored.as_slice() == expected => {
                Err(StandaloneEndpointPublicationError::CleanupOutcomeUnknown)
            }
            Ok(Some(_)) => Err(StandaloneEndpointPublicationError::CleanupConflict),
            Err(_) => Err(StandaloneEndpointPublicationError::CleanupOutcomeUnknown),
        };
    }
    sync_directory(runtime_directory)
        .map_err(|_| StandaloneEndpointPublicationError::CleanupOutcomeUnknown)
}

struct TemporaryDescriptor {
    path: PathBuf,
    armed: bool,
}

impl TemporaryDescriptor {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TemporaryDescriptor {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> std::io::Result<()> {
    // Windows has no safe directory-sync operation through std. Atomic rename
    // plus mandatory exact readback is the bounded publication evidence here.
    Ok(())
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}
