//! Concrete native mutual pairing over the restricted accepted-socket view.

use core::fmt;

use search_contracts::{
    HelloBody, MessageKind, ProtocolRange, ProtocolVersion, ProviderBodyV1,
};
use search_ports::CancellationProbe;
use search_provider_protocol::{
    BindingSession, ClientEnvelopeCodec, ClientNonce, PairingChallenge,
    PairingChallengeFrame, PairingProofFrame, PairingVerifiedFrame, ProtocolLimits,
    ServerNonce, SessionId, TransportPeer, authenticate_binding, decode_pairing_proof,
    encode_pairing_challenge, encode_pairing_verified, FRAME_PREFIX_BYTES,
    PAIRING_PROOF_BYTES,
};

use super::{CompletedStandalonePairing, StandalonePairingIo, StandalonePairingIoError};
use super::super::{
    NativeBindingError, NativeBindingExpectation, NativePairingCredentialError,
    ProviderBindingRecord,
};

const CEREMONY_ENTROPY_BYTES: usize = 80;

/// Closed native pairing-driver failure.
///
/// No variant contains proof, challenge, nonce, key, profile, path or peer
/// identity bytes.
#[derive(Debug)]
pub enum StandaloneNativePairingError {
    /// Restricted socket I/O or original setup budget failed.
    Io(StandalonePairingIoError),
    /// Canonical pairing/binding protocol validation failed.
    Protocol(search_provider_protocol::ProtocolError),
    /// Current registration did not match the trusted native expectation.
    Binding(NativeBindingError),
    /// Exact registered-generation credential could not be resolved.
    Credential(NativePairingCredentialError),
    /// Qualified operating-system entropy was unavailable.
    EntropyUnavailable,
}

impl fmt::Display for StandaloneNativePairingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => fmt::Display::fmt(error, formatter),
            Self::Protocol(error) => fmt::Display::fmt(error, formatter),
            Self::Binding(error) => fmt::Display::fmt(error, formatter),
            Self::Credential(error) => fmt::Display::fmt(error, formatter),
            Self::EntropyUnavailable => {
                formatter.write_str("STANDALONE_PAIRING_ENTROPY_UNAVAILABLE")
            }
        }
    }
}

impl std::error::Error for StandaloneNativePairingError {}

impl From<StandalonePairingIoError> for StandaloneNativePairingError {
    fn from(error: StandalonePairingIoError) -> Self { Self::Io(error) }
}

impl From<search_provider_protocol::ProtocolError> for StandaloneNativePairingError {
    fn from(error: search_provider_protocol::ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl From<NativeBindingError> for StandaloneNativePairingError {
    fn from(error: NativeBindingError) -> Self { Self::Binding(error) }
}

impl From<NativePairingCredentialError> for StandaloneNativePairingError {
    fn from(error: NativePairingCredentialError) -> Self {
        Self::Credential(error)
    }
}

/// Run canonical hello/challenge/mutual-proof pairing on one exact socket.
///
/// The current published registration is checked before credential access. One
/// fresh CSPRNG draw supplies the session, nonces and challenge. The challenge is
/// consumed in the process ledger before output, so disconnect or invalid proof
/// abandons it rather than permitting reuse. All I/O and credential reads use the
/// original absolute setup deadline through [`StandalonePairingIo`].
pub(super) fn perform_native_pairing<C: CancellationProbe + Clone>(
    io: &mut StandalonePairingIo<'_, C>,
    connection_sequence: u64,
    binding_session: &mut BindingSession,
    local_protocols: ProtocolRange,
    limits: ProtocolLimits,
    record: &ProviderBindingRecord,
    expected: &NativeBindingExpectation,
) -> Result<CompletedStandalonePairing, StandaloneNativePairingError> {
    let received = read_hello(io, local_protocols, limits, record)?;
    expected.validate_registration(record, &received.peer)?;
    let negotiated = binding_session.accept_hello(&received.hello, &received.peer)?;
    if negotiated.version() != received.version {
        return Err(search_provider_protocol::ProtocolError::NoCompatibleVersion.into());
    }

    let credential_context = io.remaining_context()?;
    let key = expected.load_pairing_key(&received.peer, &credential_context)?;
    let binding_digest = key.with_bytes(crate::secret_composition::derive_binding_digest);

    let material = ceremony_material()?;
    let mut machine = negotiated.begin_pairing(binding_digest);
    machine.issue_challenge(
        material.session_id,
        material.client_nonce,
        material.challenge,
    )?;
    binding_session.consume_challenge(material.session_id, &material.challenge)?;

    let challenge = PairingChallengeFrame::new(
        negotiated.version(),
        connection_sequence,
        material.session_id,
        material.client_nonce,
        material.challenge,
        binding_digest,
        material.server_nonce,
    )?;
    io.write_all(&encode_pairing_challenge(challenge))?;
    io.flush()?;

    let observed = read_client_proof(io)?;
    if observed.connection_sequence() != connection_sequence {
        machine.fail();
        return Err(search_provider_protocol::ProtocolError::SequenceGap.into());
    }
    let client_transcript = machine.client_transcript()?;
    let expected_client = key.with_bytes(|bytes| {
        crate::secret_composition::pairing_keyed_proof(bytes, &client_transcript)
    });
    machine.verify_client_proof(&expected_client, &observed.proof())?;

    let provider_transcript = machine.server_transcript()?;
    let provider_proof = key.with_bytes(|bytes| {
        crate::secret_composition::pairing_keyed_proof(bytes, &provider_transcript)
    });
    machine.issue_provider_proof(provider_proof)?;
    let verified = machine.clone().into_verified()?;
    let binding = authenticate_binding(
        &received.hello,
        &verified,
        &record.installation_incarnation_id,
        &received.peer,
    )?;
    let completed =
        CompletedStandalonePairing::new(binding, machine, material.server_nonce)?;

    let response = PairingVerifiedFrame::new(connection_sequence, provider_proof)?;
    io.write_all(&encode_pairing_verified(response))?;
    io.flush()?;
    Ok(completed)
}

struct ReceivedHello {
    version: ProtocolVersion,
    hello: HelloBody,
    peer: TransportPeer,
}

fn read_hello<C: CancellationProbe>(
    io: &mut StandalonePairingIo<'_, C>,
    local_protocols: ProtocolRange,
    limits: ProtocolLimits,
    record: &ProviderBindingRecord,
) -> Result<ReceivedHello, StandaloneNativePairingError> {
    let limits = limits.validate()?;
    let mut prefix = [0_u8; FRAME_PREFIX_BYTES];
    io.read_exact(&mut prefix)?;
    let body_bytes = usize::try_from(u32::from_le_bytes(prefix))
        .map_err(|_| search_provider_protocol::ProtocolError::FrameTooLarge)?;
    let total_bytes = FRAME_PREFIX_BYTES
        .checked_add(body_bytes)
        .ok_or(search_provider_protocol::ProtocolError::FrameTooLarge)?;
    if body_bytes == 0 {
        return Err(search_provider_protocol::ProtocolError::InvalidEnvelope.into());
    }
    if body_bytes > limits.max_body_bytes || total_bytes > limits.max_frame_bytes {
        return Err(search_provider_protocol::ProtocolError::FrameTooLarge.into());
    }

    let mut frame = Vec::new();
    frame
        .try_reserve_exact(total_bytes)
        .map_err(|_| search_provider_protocol::ProtocolError::ResourceExhausted)?;
    frame.extend_from_slice(&prefix);
    frame.resize(total_bytes, 0);
    io.read_exact(&mut frame[FRAME_PREFIX_BYTES..])?;
    let envelope = ClientEnvelopeCodec::decode(&frame, limits, local_protocols)?;
    let version = envelope.protocol_version();
    if envelope.message_kind != MessageKind::Hello
        || envelope.relative_deadline_ms.is_some()
        || envelope.connection_sequence != 0
        || envelope.installation_incarnation_id != record.installation_incarnation_id
        || envelope.binding_id != record.binding_id
    {
        return Err(search_provider_protocol::ProtocolError::AuthenticationFailed.into());
    }
    let ProviderBodyV1::Hello(hello) = envelope.body else {
        return Err(search_provider_protocol::ProtocolError::InvalidEnvelope.into());
    };
    let peer = TransportPeer {
        role: hello.peer_role,
        incarnation: envelope.installation_incarnation_id,
        binding: envelope.binding_id,
    };
    Ok(ReceivedHello { version, hello, peer })
}

fn read_client_proof<C: CancellationProbe>(
    io: &mut StandalonePairingIo<'_, C>,
) -> Result<PairingProofFrame, StandaloneNativePairingError> {
    let mut bytes = [0_u8; PAIRING_PROOF_BYTES];
    io.read_exact(&mut bytes)?;
    decode_pairing_proof(&bytes).map_err(Into::into)
}

struct CeremonyMaterial {
    session_id: SessionId,
    client_nonce: ClientNonce,
    challenge: PairingChallenge,
    server_nonce: ServerNonce,
}

fn ceremony_material() -> Result<CeremonyMaterial, StandaloneNativePairingError> {
    let mut bytes = [0_u8; CEREMONY_ENTROPY_BYTES];
    crate::qualified_entropy::fill_qualified_entropy(&mut bytes)
        .map_err(|_| StandaloneNativePairingError::EntropyUnavailable)?;

    let mut session = [0_u8; 16];
    session.copy_from_slice(&bytes[..16]);
    let mut client_nonce = [0_u8; 16];
    client_nonce.copy_from_slice(&bytes[16..32]);
    let mut challenge = [0_u8; 32];
    challenge.copy_from_slice(&bytes[32..64]);
    let mut server_nonce = [0_u8; 16];
    server_nonce.copy_from_slice(&bytes[64..80]);
    bytes.fill(0);

    Ok(CeremonyMaterial {
        session_id: SessionId::from_bytes(session)?,
        client_nonce: ClientNonce::from_bytes(client_nonce)?,
        challenge: PairingChallenge::from_bytes(challenge)?,
        server_nonce: ServerNonce::from_bytes(server_nonce)?,
    })
}
