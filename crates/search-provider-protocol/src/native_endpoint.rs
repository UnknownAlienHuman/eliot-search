//! Canonical authenticated descriptor for one native standalone endpoint.
//!
//! This is a bounded local bootstrap record, not a provider request and not an
//! authorization decision. It carries only public registration coordinates plus
//! a keyed proof produced with the exact registered pairing key. Secret-store I/O,
//! filesystem publication and keyed hashing remain adapter responsibilities.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use search_contracts::{
    BindingId, Blake3Digest32, InstallationId, InstallationIncarnationId,
    NonZeroRevision, OpaqueRef, ProfileId, ProtocolVersion,
    protocol::PeerRole,
};

use crate::{ProofDigest, ProtocolError};

const DESCRIPTOR_MAGIC: &[u8; 8] = b"ELNEP001";
const DESCRIPTOR_DOMAIN: &[u8] = b"ELIOT-NATIVE-ENDPOINT-DESCRIPTOR-v1\0";
const CREDENTIAL_LOCATOR_DOMAIN: &[u8] =
    b"ELIOT-NATIVE-PROVIDER-PAIRING-CREDENTIAL-v1\0";
const NATIVE_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };
const FIXED_WITHOUT_TEXT_OR_PROOF: usize =
    8 + 2 + 2 + 4 + 2 + 16 + 16 + 16 + 32 + 8 + 2 + 2 + 2 + 1;
const PROOF_BYTES: usize = 32;

/// Maximum complete native endpoint descriptor bytes.
pub const MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES: usize = 16 * 1024;

/// Public coordinates for one authenticated native standalone endpoint.
///
/// Possession of this value grants nothing. Consumers must load the exact
/// generation credential through an independently selected platform owner and
/// verify [`native_endpoint_descriptor_transcript`] before connecting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeEndpointDescriptorV1 {
    address: SocketAddrV4,
    installation_id: InstallationId,
    installation_incarnation_id: InstallationIncarnationId,
    binding_id: BindingId,
    peer_identity_digest: Blake3Digest32,
    pairing_generation: NonZeroRevision,
    profile_id: ProfileId,
    disclosure_ceiling_ref: OpaqueRef,
    pairing_proof_ref: OpaqueRef,
    requested_capability_digest: Option<Blake3Digest32>,
    proof: ProofDigest,
}

impl NativeEndpointDescriptorV1 {
    /// Creates one validated protocol-1.0 IPv4-loopback descriptor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: SocketAddrV4,
        installation_id: InstallationId,
        installation_incarnation_id: InstallationIncarnationId,
        binding_id: BindingId,
        peer_identity_digest: Blake3Digest32,
        pairing_generation: NonZeroRevision,
        profile_id: ProfileId,
        disclosure_ceiling_ref: OpaqueRef,
        pairing_proof_ref: OpaqueRef,
        requested_capability_digest: Option<Blake3Digest32>,
        proof: ProofDigest,
    ) -> Result<Self, ProtocolError> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(ProtocolError::InvalidEnvelope);
        }
        let value = Self {
            address,
            installation_id,
            installation_incarnation_id,
            binding_id,
            peer_identity_digest,
            pairing_generation,
            profile_id,
            disclosure_ceiling_ref,
            pairing_proof_ref,
            requested_capability_digest,
            proof,
        };
        // Prove every variable-width field and the complete record fit before
        // this value can be published or passed to a credential owner.
        let _ = encode_descriptor_body(&value)?;
        Ok(value)
    }

    /// Exact loopback socket address.
    #[must_use]
    pub const fn address(&self) -> SocketAddr {
        SocketAddr::V4(self.address)
    }

    /// Owning installation identity.
    #[must_use]
    pub const fn installation_id(&self) -> InstallationId {
        self.installation_id
    }

    /// Owning installation incarnation.
    #[must_use]
    pub const fn installation_incarnation_id(&self) -> InstallationIncarnationId {
        self.installation_incarnation_id
    }

    /// Durable provider binding.
    #[must_use]
    pub const fn binding_id(&self) -> BindingId {
        self.binding_id
    }

    /// Independently registered peer identity digest.
    #[must_use]
    pub const fn peer_identity_digest(&self) -> Blake3Digest32 {
        self.peer_identity_digest
    }

    /// Registered pairing generation.
    #[must_use]
    pub const fn pairing_generation(&self) -> NonZeroRevision {
        self.pairing_generation
    }

    /// Expected standalone client profile.
    #[must_use]
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    /// Expected disclosure-policy reference.
    #[must_use]
    pub fn disclosure_ceiling_ref(&self) -> &OpaqueRef {
        &self.disclosure_ceiling_ref
    }

    /// Opaque pairing reference copied into the canonical hello.
    #[must_use]
    pub fn pairing_proof_ref(&self) -> &OpaqueRef {
        &self.pairing_proof_ref
    }

    /// Optional requested capability digest; never a permit.
    #[must_use]
    pub const fn requested_capability_digest(&self) -> Option<Blake3Digest32> {
        self.requested_capability_digest
    }

    /// Exact protocol version represented by this descriptor.
    #[must_use]
    pub const fn protocol_version(&self) -> ProtocolVersion {
        NATIVE_VERSION
    }

    /// Keyed proof over the canonical descriptor transcript.
    #[must_use]
    pub const fn proof(&self) -> ProofDigest {
        self.proof
    }

    /// Canonical credential-locator material for this standalone registration.
    pub fn credential_locator_material(&self) -> Result<Vec<u8>, ProtocolError> {
        provider_pairing_credential_locator_material(
            self.installation_id,
            self.installation_incarnation_id,
            self.binding_id,
            self.peer_identity_digest,
            self.pairing_generation,
            PeerRole::StandaloneCli,
        )
    }
}

/// Encodes one complete canonical descriptor.
pub fn encode_native_endpoint_descriptor(
    descriptor: &NativeEndpointDescriptorV1,
) -> Result<Vec<u8>, ProtocolError> {
    let mut output = encode_descriptor_body(descriptor)?;
    output
        .try_reserve_exact(PROOF_BYTES)
        .map_err(|_| ProtocolError::ResourceExhausted)?;
    output.extend_from_slice(descriptor.proof.as_bytes());
    if output.len() > MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    Ok(output)
}

/// Decodes one exact canonical descriptor and rejects trailing bytes.
pub fn decode_native_endpoint_descriptor(
    bytes: &[u8],
) -> Result<NativeEndpointDescriptorV1, ProtocolError> {
    if bytes.len() > MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    if bytes.len() < FIXED_WITHOUT_TEXT_OR_PROOF + PROOF_BYTES {
        return Err(ProtocolError::InvalidEnvelope);
    }
    let mut input = Input::new(bytes);
    if input.take::<8>()? != *DESCRIPTOR_MAGIC {
        return Err(ProtocolError::InvalidEnvelope);
    }
    let version = ProtocolVersion {
        major: input.u16()?,
        minor: input.u16()?,
    };
    if version != NATIVE_VERSION {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    let address = SocketAddrV4::new(
        Ipv4Addr::from(input.take::<4>()?),
        input.u16()?,
    );
    let installation_id = InstallationId::from_bytes(input.take::<16>()?);
    let installation_incarnation_id =
        InstallationIncarnationId::from_bytes(input.take::<16>()?);
    let binding_id = BindingId::from_bytes(input.take::<16>()?);
    let peer_identity_digest = Blake3Digest32::from_bytes(input.take::<32>()?);
    let pairing_generation =
        NonZeroRevision::new(input.u64()?).map_err(|_| ProtocolError::InvalidEnvelope)?;
    let profile_id =
        ProfileId::new(input.text()?).map_err(|_| ProtocolError::InvalidEnvelope)?;
    let disclosure_ceiling_ref =
        OpaqueRef::new(input.text()?).map_err(|_| ProtocolError::InvalidEnvelope)?;
    let pairing_proof_ref =
        OpaqueRef::new(input.text()?).map_err(|_| ProtocolError::InvalidEnvelope)?;
    let requested_capability_digest = match input.byte()? {
        0 => None,
        1 => Some(Blake3Digest32::from_bytes(input.take::<32>()?)),
        _ => return Err(ProtocolError::InvalidEnvelope),
    };
    let proof = ProofDigest::from_bytes(input.take::<32>()?);
    input.finish()?;
    NativeEndpointDescriptorV1::new(
        address,
        installation_id,
        installation_incarnation_id,
        binding_id,
        peer_identity_digest,
        pairing_generation,
        profile_id,
        disclosure_ceiling_ref,
        pairing_proof_ref,
        requested_capability_digest,
        proof,
    )
}

/// Canonical bytes covered by the descriptor's keyed proof.
pub fn native_endpoint_descriptor_transcript(
    descriptor: &NativeEndpointDescriptorV1,
) -> Result<Vec<u8>, ProtocolError> {
    let body = encode_descriptor_body(descriptor)?;
    let mut transcript = Vec::new();
    transcript
        .try_reserve_exact(DESCRIPTOR_DOMAIN.len() + body.len())
        .map_err(|_| ProtocolError::ResourceExhausted)?;
    transcript.extend_from_slice(DESCRIPTOR_DOMAIN);
    transcript.extend_from_slice(&body);
    Ok(transcript)
}

/// Builds the exact non-secret preimage hashed into a provider-credential locator.
///
/// The adapter performs the actual digest and secret-store lookup. Only
/// `standalone_cli` and `client_adapter` are valid registered peer roles.
pub fn provider_pairing_credential_locator_material(
    installation_id: InstallationId,
    installation_incarnation_id: InstallationIncarnationId,
    binding_id: BindingId,
    peer_identity_digest: Blake3Digest32,
    pairing_generation: NonZeroRevision,
    peer_role: PeerRole,
) -> Result<Vec<u8>, ProtocolError> {
    let role = match peer_role {
        PeerRole::StandaloneCli => 1,
        PeerRole::ClientAdapter => 2,
        _ => return Err(ProtocolError::AuthenticationFailed),
    };
    let capacity = CREDENTIAL_LOCATOR_DOMAIN.len() + 16 + 16 + 16 + 32 + 8 + 1;
    let mut material = Vec::new();
    material
        .try_reserve_exact(capacity)
        .map_err(|_| ProtocolError::ResourceExhausted)?;
    material.extend_from_slice(CREDENTIAL_LOCATOR_DOMAIN);
    material.extend_from_slice(installation_id.as_bytes());
    material.extend_from_slice(installation_incarnation_id.as_bytes());
    material.extend_from_slice(binding_id.as_bytes());
    material.extend_from_slice(peer_identity_digest.as_bytes());
    material.extend_from_slice(&pairing_generation.get().to_be_bytes());
    material.push(role);
    Ok(material)
}

fn encode_descriptor_body(
    descriptor: &NativeEndpointDescriptorV1,
) -> Result<Vec<u8>, ProtocolError> {
    let profile = descriptor.profile_id.as_str().as_bytes();
    let disclosure = descriptor.disclosure_ceiling_ref.as_str().as_bytes();
    let pairing = descriptor.pairing_proof_ref.as_str().as_bytes();
    let variable = profile
        .len()
        .checked_add(disclosure.len())
        .and_then(|value| value.checked_add(pairing.len()))
        .ok_or(ProtocolError::FrameTooLarge)?;
    let capability = if descriptor.requested_capability_digest.is_some() {
        32
    } else {
        0
    };
    let capacity = FIXED_WITHOUT_TEXT_OR_PROOF
        .checked_add(variable)
        .and_then(|value| value.checked_add(capability))
        .ok_or(ProtocolError::FrameTooLarge)?;
    if capacity + PROOF_BYTES > MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| ProtocolError::ResourceExhausted)?;
    output.extend_from_slice(DESCRIPTOR_MAGIC);
    output.extend_from_slice(&NATIVE_VERSION.major.to_be_bytes());
    output.extend_from_slice(&NATIVE_VERSION.minor.to_be_bytes());
    output.extend_from_slice(&descriptor.address.ip().octets());
    output.extend_from_slice(&descriptor.address.port().to_be_bytes());
    output.extend_from_slice(descriptor.installation_id.as_bytes());
    output.extend_from_slice(descriptor.installation_incarnation_id.as_bytes());
    output.extend_from_slice(descriptor.binding_id.as_bytes());
    output.extend_from_slice(descriptor.peer_identity_digest.as_bytes());
    output.extend_from_slice(&descriptor.pairing_generation.get().to_be_bytes());
    put_text(&mut output, profile)?;
    put_text(&mut output, disclosure)?;
    put_text(&mut output, pairing)?;
    match descriptor.requested_capability_digest {
        Some(digest) => {
            output.push(1);
            output.extend_from_slice(digest.as_bytes());
        }
        None => output.push(0),
    }
    debug_assert_eq!(output.len(), capacity);
    Ok(output)
}

fn put_text(output: &mut Vec<u8>, text: &[u8]) -> Result<(), ProtocolError> {
    let length = u16::try_from(text.len()).map_err(|_| ProtocolError::FrameTooLarge)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(text);
    Ok(())
}

struct Input<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Input<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], ProtocolError> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(ProtocolError::InvalidEnvelope)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(ProtocolError::InvalidEnvelope)?
            .try_into()
            .map_err(|_| ProtocolError::InvalidEnvelope)?;
        self.cursor = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        Ok(u16::from_be_bytes(self.take::<2>()?))
    }

    fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_be_bytes(self.take::<8>()?))
    }

    fn text(&mut self) -> Result<String, ProtocolError> {
        let length = usize::from(self.u16()?);
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(ProtocolError::InvalidEnvelope)?;
        let bytes = self
            .bytes
            .get(self.cursor..end)
            .ok_or(ProtocolError::InvalidEnvelope)?;
        let text = core::str::from_utf8(bytes)
            .map_err(|_| ProtocolError::InvalidEnvelope)?
            .to_owned();
        self.cursor = end;
        Ok(text)
    }

    fn finish(self) -> Result<(), ProtocolError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(ProtocolError::InvalidEnvelope)
        }
    }
}
