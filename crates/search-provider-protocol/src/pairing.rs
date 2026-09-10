//! Pairing kernel (#73): mutual-authentication ceremony without I/O.
//!
//! This layer owns the pairing state machine, the opaque non-zero ceremony
//! material (session ID, client nonce, provider challenge), the
//! domain-separated proof transcripts and the single-use challenge ledger.
//! It owns no entropy source, transport I/O, credential storage, network,
//! process or filesystem behavior: nonces, challenges and key material are
//! supplied by the secret-owning daemon adapter, which computes the actual
//! keyed digests over the exact transcripts built here.
//!
//! Digest-dependency decision: no new `Cargo.toml` dependency is introduced
//! (a keyed-hash dependency requires an ADR plus boundary review). The
//! transcript layout is fixed and golden-tested, so the daemon adapter —
//! which already pins `blake3 v1.8.2`, the exact version #89 requires —
//! computes `keyed_blake3(key, transcript_bytes)` with zero redesign. This
//! mirrors the `search-contracts` precedent where
//! `domain_separated_preimage` builds the exact hashed bytes while
//! "cryptographic hashing remains an explicit caller operation".

use core::fmt;
use std::collections::BTreeSet;

use search_contracts::ProtocolVersion;

use crate::error::ProtocolError;

/// Domain separator bound into every client proof transcript.
pub const PAIRING_CLIENT_DOMAIN: &str = "ELIOT-PAIRING-CLIENT-v1";
/// Domain separator bound into every server proof transcript.
pub const PAIRING_SERVER_DOMAIN: &str = "ELIOT-PAIRING-SERVER-v1";

/// Fixed-size cryptographic binding-proof digest.
///
/// The digest is computed by the secret-owning adapter over the exact
/// transcripts built in this module; this package compares digests in
/// fixed-work time and binds them to ceremony state. Formatting is always
/// redacted.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProofDigest([u8; 32]);

impl ProofDigest {
    /// Creates a proof digest produced by the secret-owning adapter.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Exact bytes for framing or platform cryptographic verification.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for ProofDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProofDigest(<redacted>)")
    }
}

/// Performs constant-work equality over fixed-size proof digests.
#[must_use]
pub fn verify_proof(expected: &ProofDigest, observed: &ProofDigest) -> bool {
    let mut difference = 0_u8;
    for (left, right) in expected.0.iter().zip(observed.0.iter()) {
        difference |= left ^ right;
    }
    difference == 0
}

/// Opaque 32-byte binding key with redacted formatting and zero-on-drop.
///
/// The type is non-clone: key material is exposed only for the duration of a
/// caller-supplied callback (mirroring `search-os-secrets::SecretLease`),
/// and the owned bytes are overwritten on drop.
pub struct BindingKey {
    bytes: [u8; 32],
}

impl BindingKey {
    /// Accepts externally supplied key material; all-zero keys are malformed.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, ProtocolError> {
        if is_all_zero(&bytes) {
            return Err(ProtocolError::InvalidBindingKey);
        }
        Ok(Self { bytes })
    }

    /// Exposes key material only for the duration of the supplied callback,
    /// so the secret-owning adapter can compute keyed proofs without the key
    /// ever escaping by value.
    pub fn with_bytes<T>(&self, use_key: impl FnOnce(&[u8; 32]) -> T) -> T {
        use_key(&self.bytes)
    }
}

impl fmt::Debug for BindingKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingKey")
            .field("bytes", &"<redacted>")
            .finish()
    }
}

impl Drop for BindingKey {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

const fn is_all_zero(bytes: &[u8]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

macro_rules! nonzero_opaque {
    ($name:ident, $size:expr, $doc:literal) => {
        #[doc = $doc]
        ///
        /// All-zero values are malformed and rejected at construction.
        /// Formatting is redacted so ceremony material never reaches logs.
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; $size]);

        impl $name {
            /// Accepts adapter-supplied material; all-zero input fails.
            pub fn from_bytes(bytes: [u8; $size]) -> Result<Self, ProtocolError> {
                if is_all_zero(&bytes) {
                    return Err(ProtocolError::InvalidNonce);
                }
                Ok(Self(bytes))
            }

            /// Exact opaque bytes for transcript binding.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; $size] {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    };
}

nonzero_opaque!(
    SessionId,
    16,
    "Non-zero pairing session identifier supplied by the daemon adapter."
);
nonzero_opaque!(
    ClientNonce,
    16,
    "Non-zero client nonce supplied by the daemon adapter."
);
nonzero_opaque!(
    ServerNonce,
    16,
    "Non-zero per-incarnation server nonce supplied by the daemon adapter."
);
nonzero_opaque!(
    PairingChallenge,
    32,
    "Non-zero provider challenge supplied by the daemon adapter."
);

/// Exact domain-separated bytes bound into one pairing proof.
///
/// Layout (fixed order, fixed sizes): `domain || 0x00 || major LE ||
/// minor LE || binding digest (32) || session (16) || nonce (16) ||
/// challenge (32)`. The secret-owning adapter computes the keyed digest over
/// these bytes; this package verifies the ceremony around them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingTranscript {
    bytes: Vec<u8>,
}

impl PairingTranscript {
    /// Exact transcript bytes for the keyed-digest computation.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Transcript length; fixed per domain by construction.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Transcripts are never empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

fn build_transcript(
    domain: &str,
    version: ProtocolVersion,
    binding: &ProofDigest,
    session: SessionId,
    nonce: &ClientNonce,
    challenge: &PairingChallenge,
) -> PairingTranscript {
    let mut bytes = Vec::with_capacity(domain.len() + 1 + 2 + 2 + 32 + 16 + 16 + 32);
    bytes.extend_from_slice(domain.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&version.major.to_le_bytes());
    bytes.extend_from_slice(&version.minor.to_le_bytes());
    bytes.extend_from_slice(binding.as_bytes());
    bytes.extend_from_slice(session.as_bytes());
    bytes.extend_from_slice(nonce.as_bytes());
    bytes.extend_from_slice(challenge.as_bytes());
    PairingTranscript { bytes }
}

/// Builds the exact bytes the client keyed proof must bind.
#[must_use]
pub fn client_proof_transcript(
    version: ProtocolVersion,
    binding: &ProofDigest,
    session: SessionId,
    nonce: &ClientNonce,
    challenge: &PairingChallenge,
) -> PairingTranscript {
    build_transcript(
        PAIRING_CLIENT_DOMAIN,
        version,
        binding,
        session,
        nonce,
        challenge,
    )
}

/// Builds the exact bytes the provider keyed proof must bind.
#[must_use]
pub fn server_proof_transcript(
    version: ProtocolVersion,
    binding: &ProofDigest,
    session: SessionId,
    nonce: &ClientNonce,
    challenge: &PairingChallenge,
) -> PairingTranscript {
    build_transcript(
        PAIRING_SERVER_DOMAIN,
        version,
        binding,
        session,
        nonce,
        challenge,
    )
}

/// Closed pairing ceremony state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PairingState {
    /// Ceremony created; no challenge issued yet.
    Offered,
    /// Challenge issued; client proof outstanding.
    ChallengeIssued,
    /// Client proof verified; provider proof not yet issued.
    ClientVerified,
    /// Mutual verification complete; admission prerequisite satisfied.
    MutuallyVerified,
    /// Terminal failure; the ceremony cannot continue or retry.
    Failed,
}

/// Closed pairing state machine: offered → challenge issued → client
/// verified → mutually verified, with terminal failure.
///
/// A provider proof cannot be issued before exact client verification: the
/// transition is rejected in code, not by convention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingMachine {
    state: PairingState,
    version: ProtocolVersion,
    binding: ProofDigest,
    session: Option<SessionId>,
    client_nonce: Option<ClientNonce>,
    challenge: Option<PairingChallenge>,
    provider_proof: Option<ProofDigest>,
}

impl PairingMachine {
    /// Creates an offered ceremony for one negotiated version and binding.
    #[must_use]
    pub const fn new(version: ProtocolVersion, binding: ProofDigest) -> Self {
        Self {
            state: PairingState::Offered,
            version,
            binding,
            session: None,
            client_nonce: None,
            challenge: None,
            provider_proof: None,
        }
    }

    /// Current ceremony state.
    #[must_use]
    pub const fn state(&self) -> PairingState {
        self.state
    }

    /// Negotiated version bound into both proof transcripts.
    #[must_use]
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Issues one ceremony challenge; single-use per machine.
    pub fn issue_challenge(
        &mut self,
        session: SessionId,
        client_nonce: ClientNonce,
        challenge: PairingChallenge,
    ) -> Result<(), ProtocolError> {
        if self.state == PairingState::Failed {
            return Err(ProtocolError::PairingFailed);
        }
        if self.state != PairingState::Offered {
            return Err(ProtocolError::InvalidPairingTransition);
        }
        self.session = Some(session);
        self.client_nonce = Some(client_nonce);
        self.challenge = Some(challenge);
        self.state = PairingState::ChallengeIssued;
        Ok(())
    }

    /// Verifies the client proof in constant-work time. A mismatch fails the
    /// ceremony terminally: the machine moves to `Failed` and can never
    /// verify, issue or complete afterwards.
    pub fn verify_client_proof(
        &mut self,
        expected: &ProofDigest,
        observed: &ProofDigest,
    ) -> Result<(), ProtocolError> {
        if self.state == PairingState::Failed {
            return Err(ProtocolError::PairingFailed);
        }
        if self.state != PairingState::ChallengeIssued {
            return Err(ProtocolError::InvalidPairingTransition);
        }
        if !verify_proof(expected, observed) {
            self.state = PairingState::Failed;
            return Err(ProtocolError::PairingProofInvalid);
        }
        self.state = PairingState::ClientVerified;
        Ok(())
    }

    /// Issues the provider proof after exact client verification. Calling
    /// before verification — including immediately after the challenge —
    /// fails without changing state.
    pub fn issue_provider_proof(
        &mut self,
        proof: ProofDigest,
    ) -> Result<ProofDigest, ProtocolError> {
        if self.state == PairingState::Failed {
            return Err(ProtocolError::PairingFailed);
        }
        if self.state != PairingState::ClientVerified {
            return Err(ProtocolError::InvalidPairingTransition);
        }
        self.provider_proof = Some(proof);
        self.state = PairingState::MutuallyVerified;
        Ok(proof)
    }

    /// Fails the ceremony terminally; idempotent.
    pub fn fail(&mut self) {
        self.state = PairingState::Failed;
    }

    /// Whether mutual verification completed.
    #[must_use]
    pub const fn is_mutually_verified(&self) -> bool {
        matches!(self.state, PairingState::MutuallyVerified)
    }

    fn ceremony_material(
        &self,
    ) -> Result<(SessionId, ClientNonce, PairingChallenge), ProtocolError> {
        match (self.session, self.client_nonce, self.challenge) {
            (Some(session), Some(nonce), Some(challenge)) => Ok((session, nonce, challenge)),
            _ => Err(ProtocolError::InvalidPairingTransition),
        }
    }

    /// Exact client transcript for the adapter keyed-digest computation.
    pub fn client_transcript(&self) -> Result<PairingTranscript, ProtocolError> {
        let (session, nonce, challenge) = self.ceremony_material()?;
        Ok(client_proof_transcript(
            self.version,
            &self.binding,
            session,
            &nonce,
            &challenge,
        ))
    }

    /// Exact provider transcript for the adapter keyed-digest computation.
    pub fn server_transcript(&self) -> Result<PairingTranscript, ProtocolError> {
        let (session, nonce, challenge) = self.ceremony_material()?;
        Ok(server_proof_transcript(
            self.version,
            &self.binding,
            session,
            &nonce,
            &challenge,
        ))
    }

    /// Consumes a mutually verified ceremony into its admission token.
    pub fn into_verified(self) -> Result<VerifiedPairing, ProtocolError> {
        if self.state == PairingState::Failed {
            return Err(ProtocolError::PairingFailed);
        }
        if self.state != PairingState::MutuallyVerified {
            return Err(ProtocolError::InvalidPairingTransition);
        }
        let (session, _, _) = self
            .ceremony_material()
            .map_err(|_| ProtocolError::InvalidPairingTransition)?;
        Ok(VerifiedPairing {
            version: self.version,
            binding: self.binding,
            session,
            provider_proof: self
                .provider_proof
                .ok_or(ProtocolError::InvalidPairingTransition)?,
        })
    }
}

/// Non-secret proof that a pairing ceremony completed mutually verified.
///
/// This token is the sequencing prerequisite for envelope admission: the
/// envelope layer accepts only requests accompanied by a `VerifiedPairing`,
/// so pairing-first ordering is enforced by construction, not convention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedPairing {
    version: ProtocolVersion,
    binding: ProofDigest,
    session: SessionId,
    provider_proof: ProofDigest,
}

impl VerifiedPairing {
    /// Negotiated version bound into the ceremony proofs.
    #[must_use]
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Binding digest bound into the ceremony proofs.
    #[must_use]
    pub const fn binding(&self) -> ProofDigest {
        self.binding
    }

    /// Ceremony session identifier.
    #[must_use]
    pub const fn session(&self) -> SessionId {
        self.session
    }

    /// Issued provider proof bound to this ceremony.
    #[must_use]
    pub const fn provider_proof(&self) -> ProofDigest {
        self.provider_proof
    }
}

/// Bounded single-use ledger for pairing challenges.
///
/// Verification consumes each `(session, challenge)` pair exactly once, which
/// gives replay resistance across session IDs and challenges. The ledger is
/// finite: a full ledger fails closed rather than evicting history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingLedger {
    consumed: BTreeSet<([u8; 16], [u8; 32])>,
    capacity: usize,
}

impl PairingLedger {
    /// Creates a finite challenge ledger.
    pub fn new(capacity: usize) -> Result<Self, ProtocolError> {
        if capacity == 0 {
            return Err(ProtocolError::InvalidLimits);
        }
        Ok(Self {
            consumed: BTreeSet::new(),
            capacity,
        })
    }

    /// Consumes one challenge exactly once; replays fail closed.
    pub fn consume(
        &mut self,
        session: SessionId,
        challenge: &PairingChallenge,
    ) -> Result<(), ProtocolError> {
        let entry = (*session.as_bytes(), *challenge.as_bytes());
        if self.consumed.contains(&entry) {
            return Err(ProtocolError::ReplayDetected);
        }
        if self.consumed.len() >= self.capacity {
            return Err(ProtocolError::ReplayCapacityExceeded);
        }
        self.consumed.insert(entry);
        Ok(())
    }

    /// Whether this exact challenge was already consumed.
    #[must_use]
    pub fn contains(&self, session: SessionId, challenge: &PairingChallenge) -> bool {
        self.consumed
            .contains(&(*session.as_bytes(), *challenge.as_bytes()))
    }

    /// Number of consumed challenges retained.
    #[must_use]
    pub fn len(&self) -> usize {
        self.consumed.len()
    }

    /// Whether the ledger is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.consumed.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u16, minor: u16) -> ProtocolVersion {
        ProtocolVersion { major, minor }
    }

    fn nonzero16(seed: u8) -> [u8; 16] {
        let mut bytes = [0_u8; 16];
        for (index, slot) in bytes.iter_mut().enumerate() {
            let offset = u8::try_from(index).expect("fixed 16-byte nonce");
            *slot = seed.wrapping_add(offset).max(1);
        }
        bytes
    }

    #[test]
    fn proof_comparison_processes_fixed_width_values() {
        let expected = ProofDigest::from_bytes([1; 32]);
        assert!(verify_proof(&expected, &ProofDigest::from_bytes([1; 32])));
        assert!(!verify_proof(&expected, &ProofDigest::from_bytes([2; 32])));
        assert!(!format!("{expected:?}").contains('1'));
    }

    #[test]
    fn ceremony_material_is_nonzero_and_redacted() {
        assert_eq!(
            SessionId::from_bytes([0; 16]),
            Err(ProtocolError::InvalidNonce)
        );
        let session = SessionId::from_bytes(nonzero16(3)).expect("session");
        assert!(!format!("{session:?}").contains('3'));
        let key = BindingKey::from_bytes([0xAB; 32]).expect("key");
        assert!(format!("{key:?}").contains("<redacted>"));
        assert_eq!(key.with_bytes(|bytes| bytes[0]), 0xAB);
    }

    #[test]
    fn challenge_issue_is_single_use_per_ceremony() {
        let binding = ProofDigest::from_bytes([9; 32]);
        let mut machine = PairingMachine::new(version(1, 0), binding);
        assert!(machine.client_transcript().is_err());
        machine
            .issue_challenge(
                SessionId::from_bytes(nonzero16(1)).expect("session"),
                ClientNonce::from_bytes(nonzero16(2)).expect("nonce"),
                PairingChallenge::from_bytes([3; 32]).expect("challenge"),
            )
            .expect("issue");
        assert_eq!(
            machine.issue_challenge(
                SessionId::from_bytes(nonzero16(4)).expect("session"),
                ClientNonce::from_bytes(nonzero16(5)).expect("nonce"),
                PairingChallenge::from_bytes([6; 32]).expect("challenge"),
            ),
            Err(ProtocolError::InvalidPairingTransition)
        );
    }

    #[test]
    fn full_ceremony_yields_verified_token() {
        let binding = ProofDigest::from_bytes([9; 32]);
        let mut machine = PairingMachine::new(version(1, 0), binding);
        machine
            .issue_challenge(
                SessionId::from_bytes(nonzero16(1)).expect("session"),
                ClientNonce::from_bytes(nonzero16(2)).expect("nonce"),
                PairingChallenge::from_bytes([3; 32]).expect("challenge"),
            )
            .expect("issue");
        let proof = ProofDigest::from_bytes([7; 32]);
        machine.verify_client_proof(&proof, &proof).expect("verify");
        machine
            .issue_provider_proof(ProofDigest::from_bytes([8; 32]))
            .expect("provider");
        assert!(machine.is_mutually_verified());
        let verified = machine.into_verified().expect("token");
        assert_eq!(verified.version(), version(1, 0));
        assert_eq!(verified.binding(), binding);
    }
}
