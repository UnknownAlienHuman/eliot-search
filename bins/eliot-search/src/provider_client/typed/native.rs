//! Canonical native hello and mutual-pairing client over one original stream.
//!
//! The caller supplies a pairing key already resolved by the native credential
//! owner for this exact binding generation. This module never opens a token file,
//! derives a development key, scans endpoints, retries pairing or falls back to
//! the legacy line protocol.

use core::fmt;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use search_contracts::{
    BindingId, Blake3Digest32, HelloBody, InstallationIncarnationId, MessageKind,
    OpaqueRef, ProtocolRange, ProtocolVersion, ProviderBodyV1, ProviderEnvelope,
    protocol::PeerRole,
};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{
    BindingContext, BindingKey, ClientEnvelopeCodec, NativeEndpointNameV1,
    PairingChallengeFrame, PairingMachine, PairingProofFrame, ProofDigest,
    ProtocolError, ProtocolLimits, ServerNonce, TransportPeer,
    authenticate_binding, decode_pairing_challenge, decode_pairing_verified,
    encode_pairing_proof, verify_proof, PAIRING_CHALLENGE_BYTES,
    PAIRING_VERIFIED_BYTES,
};

use super::io::{LocalByteStream, SetupBudget, SocketIo};
use super::{TypedClientError, TypedProviderSession};

const NATIVE_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };
const CONNECT_QUANTUM: Duration = Duration::from_millis(250);

// Must remain byte-equal to the daemon canonical local binding derivation.
// Legacy loopback compatibility deliberately retains its previous digest and
// cannot silently authenticate this local-provider path.
const BINDING_DOMAIN: &[u8] = b"eliot-search/local-provider-binding/v1\0";
const BINDING_ROLE: &[u8] = b"standalone-local-client";

/// Exact public registration coordinates used to construct the native hello.
///
/// This value contains no pairing key, credential locator, grant or source
/// authority. The pairing-proof reference is copied into the canonical hello
/// unchanged; the server still validates its own published registration and
/// native credential before opening the typed session.
#[derive(Clone)]
pub struct NativeClientBinding {
    installation_incarnation_id: InstallationIncarnationId,
    binding_id: BindingId,
    pairing_proof_ref: OpaqueRef,
    supported_protocol_range: ProtocolRange,
    requested_capability_digest: Option<Blake3Digest32>,
}

impl NativeClientBinding {
    /// Construct one standalone-client binding offer.
    ///
    /// The current typed codec implements protocol 1.0 exactly; any wider,
    /// narrower or different range is rejected before transport or credential use.
    pub fn new(
        installation_incarnation_id: InstallationIncarnationId,
        binding_id: BindingId,
        pairing_proof_ref: OpaqueRef,
        supported_protocol_range: ProtocolRange,
        requested_capability_digest: Option<Blake3Digest32>,
    ) -> Result<Self, TypedClientError> {
        let supported_protocol_range = ProtocolRange::new(
            supported_protocol_range.minimum,
            supported_protocol_range.maximum,
        )
        .map_err(|_| ProtocolError::InvalidVersion)?;
        if supported_protocol_range.minimum != NATIVE_PROTOCOL_VERSION
            || supported_protocol_range.maximum != NATIVE_PROTOCOL_VERSION
        {
            return Err(ProtocolError::NoCompatibleVersion.into());
        }
        Ok(Self {
            installation_incarnation_id,
            binding_id,
            pairing_proof_ref,
            supported_protocol_range,
            requested_capability_digest,
        })
    }

    /// Installation incarnation expected from the registered daemon.
    #[must_use]
    pub const fn installation_incarnation_id(&self) -> InstallationIncarnationId {
        self.installation_incarnation_id
    }

    /// Durable provider binding expected by this client.
    #[must_use]
    pub const fn binding_id(&self) -> BindingId { self.binding_id }

    /// Exact protocol range offered by this client.
    #[must_use]
    pub const fn supported_protocol_range(&self) -> ProtocolRange {
        self.supported_protocol_range
    }
}

impl fmt::Debug for NativeClientBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeClientBinding")
            .field(
                "installation_incarnation_id",
                &self.installation_incarnation_id,
            )
            .field("binding_id", &self.binding_id)
            .field("supported_protocol_range", &self.supported_protocol_range)
            .field(
                "requested_capability_digest",
                &self.requested_capability_digest.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl TypedProviderSession {
    /// Connect the compatibility TCP adapter, perform canonical mutual pairing
    /// and negotiate the typed transport profile under one deadline.
    ///
    /// The exact I/O and session framing below accept a transport-neutral local
    /// byte stream. This address-taking entry is retained only while the product
    /// path migrates to the architecture-required installation-scoped named pipe.
    pub fn connect_native<C: CancellationProbe>(
        address: SocketAddr,
        binding: &NativeClientBinding,
        key: BindingKey,
        limits: ProtocolLimits,
        context: &OperationContext<C>,
    ) -> Result<Self, TypedClientError> {
        if !address.ip().is_loopback() {
            return Err(TypedClientError::NonLoopback);
        }
        let cancelled = || context.cancellation().is_cancelled();
        let budget = SetupBudget::new(
            Duration::from_millis(context.relative_deadline_ms().get()),
            &cancelled,
        )?;
        let connect_timeout = budget.remaining()?.min(CONNECT_QUANTUM);
        let stream = TcpStream::connect_timeout(&address, connect_timeout)?;
        Self::open_connected_local_stream(
            stream,
            binding,
            key,
            limits,
            context,
            &budget,
        )
    }

    /// Connect one exact canonical local endpoint, then run the same pairing and
    /// typed-profile engine used by the compatibility adapter.
    ///
    /// The platform connector receives only the protocol-owned endpoint name and
    /// one finite connect timeout from the caller's original setup budget. It is
    /// invoked exactly once and may not scan, rewrite the name, reconnect, choose
    /// TCP or renew the deadline. Endpoint success remains non-authenticating.
    pub(super) fn connect_local<C, S>(
        endpoint_name: NativeEndpointNameV1,
        binding: &NativeClientBinding,
        key: BindingKey,
        limits: ProtocolLimits,
        context: &OperationContext<C>,
        connect: impl FnOnce(NativeEndpointNameV1, Duration) -> std::io::Result<S>,
    ) -> Result<Self, TypedClientError>
    where
        C: CancellationProbe,
        S: LocalByteStream + 'static,
    {
        let cancelled = || context.cancellation().is_cancelled();
        let budget = SetupBudget::new(
            Duration::from_millis(context.relative_deadline_ms().get()),
            &cancelled,
        )?;
        let connect_timeout = budget.remaining()?.min(CONNECT_QUANTUM);
        let stream = connect(endpoint_name, connect_timeout)?;
        budget.remaining()?;
        Self::open_connected_local_stream(
            stream,
            binding,
            key,
            limits,
            context,
            &budget,
        )
    }

    /// Run pairing and typed-profile negotiation over one already-connected
    /// local stream without introducing transport-specific framing or state.
    ///
    /// The platform connector must spend its connect/open work from `budget`
    /// before handing the stream here. This method never retries, reconnects,
    /// scans another endpoint or renews the original deadline.
    pub(super) fn open_connected_local_stream<C, S>(
        stream: S,
        binding: &NativeClientBinding,
        key: BindingKey,
        limits: ProtocolLimits,
        context: &OperationContext<C>,
        budget: &SetupBudget<'_>,
    ) -> Result<Self, TypedClientError>
    where
        C: CancellationProbe,
        S: LocalByteStream + 'static,
    {
        let mut socket = SocketIo::from_local_stream(stream);
        socket.configure()?;
        budget.remaining()?;

        let (binding_context, ceremony, server_nonce) = perform_pairing(
            &mut socket,
            binding,
            &key,
            limits,
            context,
            budget,
        )?;
        Self::from_paired_socket(
            socket,
            binding_context,
            ceremony,
            key,
            server_nonce,
            limits,
            budget,
        )
    }
}

fn perform_pairing<C: CancellationProbe>(
    socket: &mut SocketIo,
    offered: &NativeClientBinding,
    key: &BindingKey,
    limits: ProtocolLimits,
    context: &OperationContext<C>,
    budget: &SetupBudget<'_>,
) -> Result<(BindingContext, PairingMachine, ServerNonce), TypedClientError> {
    let limits = limits.validate()?;
    budget.remaining()?;
    let hello = HelloBody {
        peer_role: PeerRole::StandaloneCli,
        pairing_proof_ref: offered.pairing_proof_ref.clone(),
        supported_protocol_range: offered.supported_protocol_range,
        requested_capability_digest: offered.requested_capability_digest,
    };
    let envelope = ProviderEnvelope {
        protocol_major: NATIVE_PROTOCOL_VERSION.major,
        protocol_minor: NATIVE_PROTOCOL_VERSION.minor,
        installation_incarnation_id: offered.installation_incarnation_id,
        binding_id: offered.binding_id,
        connection_sequence: 0,
        request_id: context.request_id(),
        message_kind: MessageKind::Hello,
        relative_deadline_ms: None,
        body: ProviderBodyV1::Hello(hello.clone()),
    };
    let frame = ClientEnvelopeCodec::encode(
        &envelope,
        limits,
        offered.supported_protocol_range,
    )?;
    socket.write_parts_setup(&[frame.as_slice()], budget)?;

    let mut challenge_bytes = [0_u8; PAIRING_CHALLENGE_BYTES];
    socket.read_exact_setup(&mut challenge_bytes, budget)?;
    let challenge = decode_pairing_challenge(&challenge_bytes)?;
    validate_challenge(challenge, offered, key)?;

    let mut ceremony = PairingMachine::new(
        challenge.version(),
        challenge.binding_digest(),
    );
    ceremony.issue_challenge(
        challenge.session_id(),
        challenge.client_nonce(),
        challenge.challenge(),
    )?;
    let client_transcript = ceremony.client_transcript()?;
    let client_proof = keyed_proof(key, client_transcript.as_bytes());
    let proof = PairingProofFrame::new(
        challenge.connection_sequence(),
        client_proof,
    )?;
    let proof_bytes = encode_pairing_proof(proof);
    socket.write_parts_setup(&[&proof_bytes], budget)?;

    let mut verified_bytes = [0_u8; PAIRING_VERIFIED_BYTES];
    socket.read_exact_setup(&mut verified_bytes, budget)?;
    let verified = decode_pairing_verified(&verified_bytes)?;
    if verified.connection_sequence() != challenge.connection_sequence() {
        ceremony.fail();
        return Err(ProtocolError::SequenceGap.into());
    }
    let provider_transcript = ceremony.server_transcript()?;
    let expected_provider = keyed_proof(key, provider_transcript.as_bytes());
    if !verify_proof(&expected_provider, &verified.proof()) {
        ceremony.fail();
        return Err(ProtocolError::AuthenticationFailed.into());
    }
    ceremony.issue_provider_proof(verified.proof())?;
    let pairing = ceremony.clone().into_verified()?;
    let peer = TransportPeer {
        role: PeerRole::StandaloneCli,
        incarnation: offered.installation_incarnation_id,
        binding: offered.binding_id,
    };
    let binding_context = authenticate_binding(
        &hello,
        &pairing,
        &offered.installation_incarnation_id,
        &peer,
    )?;
    budget.remaining()?;
    Ok((binding_context, ceremony, challenge.server_nonce()))
}

fn validate_challenge(
    challenge: PairingChallengeFrame,
    offered: &NativeClientBinding,
    key: &BindingKey,
) -> Result<(), TypedClientError> {
    if challenge.version() != NATIVE_PROTOCOL_VERSION
        || !offered
            .supported_protocol_range
            .contains(challenge.version())
    {
        return Err(ProtocolError::NoCompatibleVersion.into());
    }
    let expected_binding = binding_digest(key);
    if !verify_proof(&expected_binding, &challenge.binding_digest()) {
        return Err(ProtocolError::AuthenticationFailed.into());
    }
    Ok(())
}

fn binding_digest(key: &BindingKey) -> ProofDigest {
    key.with_bytes(|bytes| {
        let mut hasher = blake3::Hasher::new();
        hasher.update(BINDING_DOMAIN);
        hasher.update(BINDING_ROLE);
        hasher.update(&[0]);
        hasher.update(bytes);
        ProofDigest::from_bytes(*hasher.finalize().as_bytes())
    })
}

fn keyed_proof(key: &BindingKey, transcript: &[u8]) -> ProofDigest {
    key.with_bytes(|bytes| {
        ProofDigest::from_bytes(*blake3::keyed_hash(bytes, transcript).as_bytes())
    })
}
