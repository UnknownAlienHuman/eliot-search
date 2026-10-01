//! Authenticated publication owner for one native standalone endpoint.
//!
//! The public descriptor contains no secret and grants no authority. It is
//! signed with the exact registered pairing key, published only after loopback
//! bind succeeds, and removed before the process/root owner may be released.

use core::fmt;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use search_contracts::{
    Blake3Digest32, OpaqueRef, ProtocolRange, protocol::PeerRole,
};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{
    MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES, NativeEndpointDescriptorV1, ProofDigest,
    ProtocolError, TransportPeer, encode_native_endpoint_descriptor,
    native_endpoint_descriptor_transcript,
};

use super::super::{
    NativeBindingError, NativePairingCredentialError, StandaloneProcessOwner,
};

const RUNTIME_DIRECTORY: &str = "runtime";
const NATIVE_ENDPOINT_FILE: &str = "native-endpoint.v1";
const TEMPORARY_ATTEMPTS: usize = 32;
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Non-secret hello coordinates authenticated by the published descriptor.
///
/// The pairing reference is copied unchanged into the client's canonical hello.
/// The requested capability digest remains a request and never becomes a grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneEndpointAdvertisement {
    pairing_proof_ref: OpaqueRef,
    requested_capability_digest: Option<Blake3Digest32>,
}

impl StandaloneEndpointAdvertisement {
    /// Create the exact non-secret hello advertisement for this listener.
    #[must_use]
    pub const fn new(
        pairing_proof_ref: OpaqueRef,
        requested_capability_digest: Option<Blake3Digest32>,
    ) -> Self {
        Self {
            pairing_proof_ref,
            requested_capability_digest,
        }
    }

    /// Opaque pairing reference copied into the canonical hello.
    #[must_use]
    pub const fn pairing_proof_ref(&self) -> &OpaqueRef {
        &self.pairing_proof_ref
    }

    /// Optional requested capability digest; never an authorization decision.
    #[must_use]
    pub const fn requested_capability_digest(&self) -> Option<Blake3Digest32> {
        self.requested_capability_digest
    }
}

/// Closed failure while signing, publishing or removing the native descriptor.
#[derive(Debug)]
pub enum StandaloneEndpointError {
    /// Bound endpoint was not a nonzero IPv4 loopback address.
    InvalidAddress,
    /// Current durable registration no longer matched bootstrap evidence.
    Binding(NativeBindingError),
    /// Exact registered-generation pairing key could not be loaded.
    Credential(NativePairingCredentialError),
    /// Canonical descriptor construction or encoding failed.
    Protocol(ProtocolError),
    /// Runtime directory, descriptor or temporary object had an unsafe shape.
    UnsafeRuntimeObject,
    /// The original setup operation was cancelled or its deadline elapsed.
    Interrupted,
    /// Bounded local filesystem publication failed.
    Io(io::Error),
    /// A final descriptor may remain and root ownership must not be released.
    CleanupUnresolved,
}

impl fmt::Display for StandaloneEndpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAddress => formatter.write_str("NATIVE_ENDPOINT_ADDRESS_INVALID"),
            Self::Binding(error) => fmt::Display::fmt(error, formatter),
            Self::Credential(error) => fmt::Display::fmt(error, formatter),
            Self::Protocol(error) => fmt::Display::fmt(error, formatter),
            Self::UnsafeRuntimeObject => {
                formatter.write_str("NATIVE_ENDPOINT_RUNTIME_OBJECT_INVALID")
            }
            Self::Interrupted => formatter.write_str("NATIVE_ENDPOINT_PUBLICATION_INTERRUPTED"),
            Self::Io(_) => formatter.write_str("NATIVE_ENDPOINT_PUBLICATION_IO_ERROR"),
            Self::CleanupUnresolved => {
                formatter.write_str("NATIVE_ENDPOINT_CLEANUP_UNRESOLVED")
            }
        }
    }
}

impl std::error::Error for StandaloneEndpointError {}

impl StandaloneEndpointError {
    /// Whether the root owner must remain retained until process exit.
    #[must_use]
    pub(super) const fn requires_process_retention(&self) -> bool {
        matches!(self, Self::CleanupUnresolved)
    }
}

impl From<NativeBindingError> for StandaloneEndpointError {
    fn from(error: NativeBindingError) -> Self {
        Self::Binding(error)
    }
}

impl From<NativePairingCredentialError> for StandaloneEndpointError {
    fn from(error: NativePairingCredentialError) -> Self {
        Self::Credential(error)
    }
}

impl From<ProtocolError> for StandaloneEndpointError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl From<io::Error> for StandaloneEndpointError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Exact published file retained for the listener lifetime.
///
/// The original bytes are bounded public data and are retained so shutdown
/// removes only the descriptor this owner actually published. A replaced or
/// malformed path fails closed instead of deleting an unrelated object.
pub(super) struct PublishedNativeEndpoint {
    runtime_directory: PathBuf,
    path: PathBuf,
    bytes: Vec<u8>,
    namespace_removed: bool,
    cleanup_complete: bool,
}

impl PublishedNativeEndpoint {
    pub(super) fn publish<C>(
        process: &StandaloneProcessOwner,
        address: SocketAddr,
        advertisement: &StandaloneEndpointAdvertisement,
        local_protocols: ProtocolRange,
        context: &OperationContext<C>,
    ) -> Result<Self, StandaloneEndpointError>
    where
        C: CancellationProbe + Clone,
    {
        let budget = AbsoluteBudget::new(context)?;
        let SocketAddr::V4(address) = address else {
            return Err(StandaloneEndpointError::InvalidAddress);
        };
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(StandaloneEndpointError::InvalidAddress);
        }

        let ready = process.readiness();
        let record = ready.record();
        let expected = ready.expectation();
        if record.peer_role != PeerRole::StandaloneCli {
            return Err(NativeBindingError::Unavailable.into());
        }
        let peer = TransportPeer {
            role: record.peer_role,
            incarnation: record.installation_incarnation_id,
            binding: record.binding_id,
        };
        expected.validate_registration(record, &peer)?;
        budget.check(context)?;

        let unsigned = descriptor(
            address,
            process,
            advertisement,
            ProofDigest::from_bytes([0_u8; 32]),
        )?;
        if !local_protocols.contains(unsigned.protocol_version()) {
            return Err(ProtocolError::NoCompatibleVersion.into());
        }
        let transcript = native_endpoint_descriptor_transcript(&unsigned)?;

        let credential_context = budget.remaining_context(context)?;
        let key = expected.load_pairing_key(&peer, &credential_context)?;
        budget.check(context)?;
        let proof = key.with_bytes(|bytes| {
            crate::secret_composition::pairing_keyed_proof_raw(bytes, &transcript)
        });
        drop(key);
        let signed = descriptor(address, process, advertisement, proof)?;
        let bytes = encode_native_endpoint_descriptor(&signed)?;
        budget.check(context)?;

        let runtime_directory = prepare_runtime_directory(process.canonical_root())?;
        let path = runtime_directory.join(NATIVE_ENDPOINT_FILE);
        let (mut temporary_file, temporary_path) = create_temporary(&runtime_directory)?;
        let mut final_published = false;
        let publication = (|| {
            temporary_file.write_all(&bytes)?;
            temporary_file.sync_all()?;
            let metadata = temporary_file.metadata()?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || is_reparse(&metadata)
            {
                return Err(StandaloneEndpointError::UnsafeRuntimeObject);
            }
            drop(temporary_file);
            budget.check(context)?;

            remove_stale_descriptor(&path)?;
            match fs::rename(&temporary_path, &path) {
                Ok(()) => final_published = true,
                Err(error) => match read_descriptor_file(&path) {
                    Ok(observed) if observed == bytes => {
                        final_published = true;
                        let _temporary_cleanup = fs::remove_file(&temporary_path);
                    }
                    Err(StandaloneEndpointError::Io(read_error))
                        if read_error.kind() == io::ErrorKind::NotFound =>
                    {
                        return Err(error.into());
                    }
                    Ok(_) | Err(_) => {
                        return Err(StandaloneEndpointError::CleanupUnresolved);
                    }
                },
            }
            sync_directory(&runtime_directory)?;
            verify_published_descriptor(&path, &bytes)?;
            budget.check(context)?;
            Ok(())
        })();

        if let Err(error) = publication {
            let _temporary_cleanup = fs::remove_file(&temporary_path);
            if final_published
                && cleanup_failed_publication(&runtime_directory, &path, &bytes).is_err()
            {
                return Err(StandaloneEndpointError::CleanupUnresolved);
            }
            return Err(error);
        }

        Ok(Self {
            runtime_directory,
            path,
            bytes,
            namespace_removed: false,
            cleanup_complete: false,
        })
    }

    /// Remove exactly this owner's descriptor before releasing process/root state.
    pub(super) fn remove(&mut self) -> Result<(), StandaloneEndpointError> {
        if self.cleanup_complete {
            return Ok(());
        }
        if !self.namespace_removed {
            match read_descriptor_file(&self.path) {
                Ok(observed) if observed == self.bytes => {
                    fs::remove_file(&self.path)?;
                    self.namespace_removed = true;
                }
                Ok(_) => return Err(StandaloneEndpointError::UnsafeRuntimeObject),
                Err(StandaloneEndpointError::Io(error))
                    if error.kind() == io::ErrorKind::NotFound =>
                {
                    self.namespace_removed = true;
                }
                Err(error) => return Err(error),
            }
        }
        sync_directory(&self.runtime_directory)?;
        self.cleanup_complete = true;
        Ok(())
    }
}

impl Drop for PublishedNativeEndpoint {
    fn drop(&mut self) {
        let _result = self.remove();
    }
}

fn descriptor(
    address: SocketAddrV4,
    process: &StandaloneProcessOwner,
    advertisement: &StandaloneEndpointAdvertisement,
    proof: ProofDigest,
) -> Result<NativeEndpointDescriptorV1, ProtocolError> {
    let ready = process.readiness();
    let record = ready.record();
    let expected = ready.expectation();
    NativeEndpointDescriptorV1::new(
        address,
        expected.installation_id,
        record.installation_incarnation_id,
        record.binding_id,
        expected.peer_identity_digest,
        record.pairing_generation,
        expected.profile_id.clone(),
        expected.disclosure_ceiling_ref.clone(),
        advertisement.pairing_proof_ref.clone(),
        advertisement.requested_capability_digest,
        proof,
    )
}

fn prepare_runtime_directory(root: &Path) -> Result<PathBuf, StandaloneEndpointError> {
    let runtime = root.join(RUNTIME_DIRECTORY);
    let created = match fs::create_dir(&runtime) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error.into()),
    };
    let metadata = fs::symlink_metadata(&runtime)?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err(StandaloneEndpointError::UnsafeRuntimeObject);
    }
    let canonical = fs::canonicalize(&runtime)?;
    if !canonical.starts_with(root) {
        return Err(StandaloneEndpointError::UnsafeRuntimeObject);
    }
    if created {
        sync_directory(root)?;
    }
    Ok(canonical)
}

fn create_temporary(
    runtime_directory: &Path,
) -> Result<(File, PathBuf), StandaloneEndpointError> {
    for _ in 0..TEMPORARY_ATTEMPTS {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = runtime_directory.join(format!(
            ".{NATIVE_ENDPOINT_FILE}.{}.{}.tmp",
            std::process::id(),
            sequence,
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "native endpoint temporary namespace exhausted",
    )
    .into())
}

fn remove_stale_descriptor(path: &Path) -> Result<(), StandaloneEndpointError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
                return Err(StandaloneEndpointError::UnsafeRuntimeObject);
            }
            fs::remove_file(path)?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn verify_published_descriptor(
    path: &Path,
    expected: &[u8],
) -> Result<(), StandaloneEndpointError> {
    let observed = read_descriptor_file(path)?;
    if observed != expected {
        return Err(StandaloneEndpointError::UnsafeRuntimeObject);
    }
    Ok(())
}

fn read_descriptor_file(path: &Path) -> Result<Vec<u8>, StandaloneEndpointError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err(StandaloneEndpointError::UnsafeRuntimeObject);
    }
    let maximum = u64::try_from(MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES)
        .map_err(|_| StandaloneEndpointError::UnsafeRuntimeObject)?;
    if metadata.len() > maximum {
        return Err(ProtocolError::FrameTooLarge.into());
    }

    let file = File::open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || is_reparse(&opened) || opened.len() > maximum {
        return Err(StandaloneEndpointError::UnsafeRuntimeObject);
    }
    let allowance = MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES
        .checked_add(1)
        .ok_or(ProtocolError::FrameTooLarge)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(allowance)
        .map_err(|_| ProtocolError::ResourceExhausted)?;
    let allowance = u64::try_from(allowance).map_err(|_| ProtocolError::FrameTooLarge)?;
    file.take(allowance).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES {
        return Err(ProtocolError::FrameTooLarge.into());
    }
    Ok(bytes)
}

fn cleanup_failed_publication(
    runtime_directory: &Path,
    path: &Path,
    expected: &[u8],
) -> Result<(), StandaloneEndpointError> {
    remove_matching_descriptor(path, expected)?;
    sync_directory(runtime_directory)
}

fn remove_matching_descriptor(path: &Path, expected: &[u8]) -> Result<(), StandaloneEndpointError> {
    match read_descriptor_file(path) {
        Ok(observed) if observed == expected => {
            fs::remove_file(path)?;
            Ok(())
        }
        Ok(_) => Err(StandaloneEndpointError::UnsafeRuntimeObject),
        Err(StandaloneEndpointError::Io(error))
            if error.kind() == io::ErrorKind::NotFound =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

struct AbsoluteBudget {
    deadline: Instant,
}

impl AbsoluteBudget {
    fn new<C: CancellationProbe>(
        context: &OperationContext<C>,
    ) -> Result<Self, StandaloneEndpointError> {
        if context.cancellation().is_cancelled() {
            return Err(StandaloneEndpointError::Interrupted);
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(context.relative_deadline_ms().get()))
            .ok_or(StandaloneEndpointError::Interrupted)?;
        Ok(Self { deadline })
    }

    fn check<C: CancellationProbe>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<Duration, StandaloneEndpointError> {
        if context.cancellation().is_cancelled() {
            return Err(StandaloneEndpointError::Interrupted);
        }
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(StandaloneEndpointError::Interrupted)
    }

    fn remaining_context<C>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<OperationContext<C>, StandaloneEndpointError>
    where
        C: CancellationProbe + Clone,
    {
        let remaining = self.check(context)?;
        let milliseconds = u64::try_from(remaining.as_millis())
            .ok()
            .filter(|value| *value > 0)
            .ok_or(StandaloneEndpointError::Interrupted)?;
        OperationContext::new(
            context.request_id(),
            milliseconds,
            context.cancellation().clone(),
            context.budget_ref().clone(),
        )
        .map_err(|_| StandaloneEndpointError::Interrupted)
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

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), StandaloneEndpointError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), StandaloneEndpointError> {
    // Windows directory durability requires a separately qualified native
    // handle. The file itself is synced and exact readback is still mandatory.
    Ok(())
}
