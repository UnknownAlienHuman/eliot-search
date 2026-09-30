//! Authenticated native endpoint resolution and credential-owner handoff.
//!
//! The descriptor file contains no secret. A caller-selected platform owner
//! receives only the canonical credential locator digest and returns a
//! non-clonable [`BindingKey`]. The descriptor proof is verified before the
//! socket is opened, so file contents alone cannot select trusted coordinates.

#![allow(dead_code)]

use core::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use search_ports::{CancellationProbe, OperationContext};
use search_contracts::{
    BindingId, Blake3Digest32, InstallationId, InstallationIncarnationId,
    NonZeroRevision, OpaqueRef, ProfileId,
};
use search_provider_protocol::{
    BindingKey, NativeEndpointDescriptorV1, ProofDigest, ProtocolError, ProtocolLimits,
    MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES, decode_native_endpoint_descriptor,
    native_endpoint_descriptor_transcript, verify_proof,
};

use crate::provider_client::{NativeClientBinding, TypedClientError, TypedProviderSession};

const NATIVE_DESCRIPTOR_PATH: [&str; 2] = ["runtime", "native-endpoint.v1"];

/// Independently trusted registration coordinates for one local endpoint.
///
/// This value must come from the client configuration/installation owner, not
/// from the descriptor being checked. It prevents a valid descriptor copied
/// from another local registration from selecting that registration's key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeEndpointExpectation {
    installation_id: InstallationId,
    installation_incarnation_id: InstallationIncarnationId,
    binding_id: BindingId,
    peer_identity_digest: Blake3Digest32,
    pairing_generation: NonZeroRevision,
    profile_id: ProfileId,
    disclosure_ceiling_ref: OpaqueRef,
}

impl NativeEndpointExpectation {
    /// Retain one exact expected native registration.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        installation_id: InstallationId,
        installation_incarnation_id: InstallationIncarnationId,
        binding_id: BindingId,
        peer_identity_digest: Blake3Digest32,
        pairing_generation: NonZeroRevision,
        profile_id: ProfileId,
        disclosure_ceiling_ref: OpaqueRef,
    ) -> Self {
        Self {
            installation_id,
            installation_incarnation_id,
            binding_id,
            peer_identity_digest,
            pairing_generation,
            profile_id,
            disclosure_ceiling_ref,
        }
    }

    fn matches(&self, descriptor: &NativeEndpointDescriptorV1) -> bool {
        self.installation_id == descriptor.installation_id()
            && self.installation_incarnation_id
                == descriptor.installation_incarnation_id()
            && self.binding_id == descriptor.binding_id()
            && self.peer_identity_digest == descriptor.peer_identity_digest()
            && self.pairing_generation == descriptor.pairing_generation()
            && &self.profile_id == descriptor.profile_id()
            && &self.disclosure_ceiling_ref == descriptor.disclosure_ceiling_ref()
    }
}

/// Platform-owned key resolver for one immutable provider credential locator.
///
/// The locator is a digest of exact registration coordinates, not a credential
/// name supplied by the descriptor or peer. Implementations must perform one
/// bounded read only: absence never generates, replaces or adopts a key.
pub trait NativePairingKeySource {
    /// Content-free platform failure.
    type Error;

    /// Load the exact registered-generation pairing key.
    fn load_key(
        &mut self,
        locator: &[u8; 32],
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<Option<BindingKey>, Self::Error>;
}

/// Failure while reading the bounded authenticated endpoint descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeDescriptorError {
    /// Root, runtime directory or descriptor could not be read.
    Unavailable,
    /// Descriptor or an ancestor resolved outside the selected data root.
    OutsideRoot,
    /// Final descriptor is not a regular non-symlink file.
    NotRegular,
    /// Descriptor exceeded its protocol ceiling.
    TooLarge,
    /// Canonical descriptor decoding failed.
    Invalid(ProtocolError),
}

impl NativeDescriptorError {
    /// Stable content-free reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "NATIVE_ENDPOINT_DESCRIPTOR_UNAVAILABLE",
            Self::OutsideRoot => "NATIVE_ENDPOINT_DESCRIPTOR_OUTSIDE_ROOT",
            Self::NotRegular => "NATIVE_ENDPOINT_DESCRIPTOR_NOT_REGULAR",
            Self::TooLarge => "NATIVE_ENDPOINT_DESCRIPTOR_TOO_LARGE",
            Self::Invalid(_) => "NATIVE_ENDPOINT_DESCRIPTOR_INVALID",
        }
    }
}

impl fmt::Display for NativeDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for NativeDescriptorError {}

/// Native descriptor, key-source or session-opening failure.
#[derive(Debug)]
pub enum RegisteredNativeError<E> {
    /// Credential locator material or descriptor transcript was invalid.
    Protocol(ProtocolError),
    /// The original operation was cancelled.
    Cancelled,
    /// The original operation deadline elapsed.
    DeadlineExpired,
    /// The platform credential owner failed while the budget remained valid.
    Credential(E),
    /// No exact registered-generation key exists.
    CredentialMissing,
    /// Descriptor coordinates did not match independently trusted registration input.
    DescriptorMismatch,
    /// The loaded key did not authenticate every descriptor field.
    DescriptorProofInvalid,
    /// Canonical pairing or typed transport setup failed.
    Client(TypedClientError),
}

impl<E: fmt::Display> fmt::Display for RegisteredNativeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => fmt::Display::fmt(error, formatter),
            Self::Cancelled => formatter.write_str("NATIVE_ENDPOINT_CANCELLED"),
            Self::DeadlineExpired => formatter.write_str("NATIVE_ENDPOINT_DEADLINE_EXPIRED"),
            Self::Credential(error) => fmt::Display::fmt(error, formatter),
            Self::CredentialMissing => formatter.write_str("NATIVE_ENDPOINT_CREDENTIAL_MISSING"),
            Self::DescriptorMismatch => {
                formatter.write_str("NATIVE_ENDPOINT_DESCRIPTOR_MISMATCH")
            }
            Self::DescriptorProofInvalid => {
                formatter.write_str("NATIVE_ENDPOINT_DESCRIPTOR_PROOF_INVALID")
            }
            Self::Client(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for RegisteredNativeError<E> {}

impl<E> From<ProtocolError> for RegisteredNativeError<E> {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl<E> From<TypedClientError> for RegisteredNativeError<E> {
    fn from(error: TypedClientError) -> Self {
        Self::Client(error)
    }
}

/// Read the canonical authenticated descriptor under `runtime/native-endpoint.v1`.
///
/// This operation validates containment and file shape before copying bounded
/// bytes. Authenticity is established later with the registered key by
/// [`open_registered_native`], not by filesystem location or local-user ACL.
pub fn read_native_endpoint_descriptor(
    data_root: &Path,
) -> Result<NativeEndpointDescriptorV1, NativeDescriptorError> {
    let canonical_root =
        fs::canonicalize(data_root).map_err(|_| NativeDescriptorError::Unavailable)?;
    if !canonical_root
        .metadata()
        .map_err(|_| NativeDescriptorError::Unavailable)?
        .is_dir()
    {
        return Err(NativeDescriptorError::NotRegular);
    }
    let path = canonical_root
        .join(NATIVE_DESCRIPTOR_PATH[0])
        .join(NATIVE_DESCRIPTOR_PATH[1]);
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| NativeDescriptorError::Unavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(NativeDescriptorError::NotRegular);
    }
    let canonical_path =
        fs::canonicalize(&path).map_err(|_| NativeDescriptorError::Unavailable)?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(NativeDescriptorError::OutsideRoot);
    }
    let maximum = u64::try_from(MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES)
        .map_err(|_| NativeDescriptorError::TooLarge)?;
    if metadata.len() > maximum {
        return Err(NativeDescriptorError::TooLarge);
    }
    let mut file = File::open(&canonical_path).map_err(|_| NativeDescriptorError::Unavailable)?;
    let opened = file
        .metadata()
        .map_err(|_| NativeDescriptorError::Unavailable)?;
    if !opened.is_file() {
        return Err(NativeDescriptorError::NotRegular);
    }
    if opened.len() > maximum {
        return Err(NativeDescriptorError::TooLarge);
    }
    let allowance = MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES
        .checked_add(1)
        .ok_or(NativeDescriptorError::TooLarge)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(allowance)
        .map_err(|_| NativeDescriptorError::TooLarge)?;
    let allowance_u64 =
        u64::try_from(allowance).map_err(|_| NativeDescriptorError::TooLarge)?;
    file.take(allowance_u64)
        .read_to_end(&mut bytes)
        .map_err(|_| NativeDescriptorError::Unavailable)?;
    if bytes.len() > MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES {
        return Err(NativeDescriptorError::TooLarge);
    }
    decode_native_endpoint_descriptor(&bytes).map_err(NativeDescriptorError::Invalid)
}

/// Resolve the exact registered key, authenticate the descriptor and open one session.
///
/// One absolute deadline covers key resolution and the subsequent connect,
/// pairing and typed-profile handshake. The key source receives only the
/// canonical locator digest. A missing/conflicting key, invalid descriptor
/// proof or any setup failure closes without retry or legacy fallback.
pub fn open_registered_native<C, S>(
    descriptor: &NativeEndpointDescriptorV1,
    expected: &NativeEndpointExpectation,
    key_source: &mut S,
    limits: ProtocolLimits,
    context: &OperationContext<C>,
) -> Result<TypedProviderSession, RegisteredNativeError<S::Error>>
where
    C: CancellationProbe + Clone,
    S: NativePairingKeySource,
{
    let budget = AbsoluteBudget::new::<C, S::Error>(context)?;
    if !expected.matches(descriptor) {
        return Err(RegisteredNativeError::DescriptorMismatch);
    }
    let locator_material = descriptor.credential_locator_material()?;
    let locator = *blake3::hash(&locator_material).as_bytes();
    let mut remaining = || {
        if context.cancellation().is_cancelled() {
            None
        } else {
            budget.optional_remaining()
        }
    };
    let key = match key_source.load_key(&locator, &mut remaining) {
        Ok(Some(key)) => key,
        Ok(None) => {
            budget.check(context)?;
            return Err(RegisteredNativeError::CredentialMissing);
        }
        Err(error) => {
            budget.check(context)?;
            return Err(RegisteredNativeError::Credential(error));
        }
    };
    budget.check(context)?;

    let transcript = native_endpoint_descriptor_transcript(descriptor)?;
    let expected = key.with_bytes(|bytes| {
        ProofDigest::from_bytes(*blake3::keyed_hash(bytes, &transcript).as_bytes())
    });
    if !verify_proof(&expected, &descriptor.proof()) {
        return Err(RegisteredNativeError::DescriptorProofInvalid);
    }

    let binding = NativeClientBinding::new(
        descriptor.installation_incarnation_id(),
        descriptor.binding_id(),
        descriptor.pairing_proof_ref().clone(),
        search_contracts::ProtocolRange::new(
            descriptor.protocol_version(),
            descriptor.protocol_version(),
        )
        .map_err(|_| ProtocolError::InvalidVersion)?,
        descriptor.requested_capability_digest(),
    )?;
    let remaining_context = budget.remaining_context(context)?;
    TypedProviderSession::connect_native(
        descriptor.address(),
        &binding,
        key,
        limits,
        &remaining_context,
    )
    .map_err(Into::into)
}

struct AbsoluteBudget {
    deadline: Instant,
}

impl AbsoluteBudget {
    fn new<C: CancellationProbe, E>(
        context: &OperationContext<C>,
    ) -> Result<Self, RegisteredNativeError<E>> {
        if context.cancellation().is_cancelled() {
            return Err(RegisteredNativeError::Cancelled);
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(context.relative_deadline_ms().get()))
            .ok_or(RegisteredNativeError::DeadlineExpired)?;
        Ok(Self { deadline })
    }

    fn optional_remaining(&self) -> Option<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
    }

    fn check<C: CancellationProbe, E>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<Duration, RegisteredNativeError<E>> {
        if context.cancellation().is_cancelled() {
            return Err(RegisteredNativeError::Cancelled);
        }
        self.optional_remaining()
            .ok_or(RegisteredNativeError::DeadlineExpired)
    }

    fn remaining_context<C: CancellationProbe + Clone, E>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<OperationContext<C>, RegisteredNativeError<E>> {
        if context.cancellation().is_cancelled() {
            return Err(RegisteredNativeError::Cancelled);
        }
        let remaining = self.check(context)?;
        let millis = u64::try_from(remaining.as_millis())
            .ok()
            .filter(|value| *value > 0)
            .ok_or(RegisteredNativeError::DeadlineExpired)?;
        OperationContext::new(
            context.request_id(),
            millis,
            context.cancellation().clone(),
            context.budget_ref().clone(),
        )
        .map_err(|_| RegisteredNativeError::DeadlineExpired)
    }
}
