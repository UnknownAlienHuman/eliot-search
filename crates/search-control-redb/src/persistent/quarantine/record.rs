//! Fixed-size administrative marker; no source, query, path or arbitrary text.

use sha2::{Digest, Sha256};
use super::{ControlError, JournalIdentity, MutationId};
use core::fmt;

const MAGIC: &[u8; 8] = b"ELCTQ001";
const PREFIX: usize = 113;
pub(super) const MARKER_BYTES: usize = PREFIX + 32;

/// Closed diagnosis supplied by the current owner when blocking the journal.
/// A reason records a diagnosis, not an independently verified corruption proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlQuarantineReason {
    /// Stored records or their relationships contradict required invariants.
    IntegrityContradiction,
    /// Observed external identities conflict with this journal's binding.
    IdentityContradiction,
    /// A migration cannot be proven complete and compatible.
    MigrationUnverified,
    /// An explicit owner-requested administrative hold.
    AdministrativeHold,
}

impl ControlQuarantineReason {
    const fn tag(self) -> u8 {
        match self {
            Self::IntegrityContradiction => 1,
            Self::IdentityContradiction => 2,
            Self::MigrationUnverified => 3,
            Self::AdministrativeHold => 4,
        }
    }
    fn decode(tag: u8) -> Result<Self, ControlError> {
        match tag {
            1 => Ok(Self::IntegrityContradiction),
            2 => Ok(Self::IdentityContradiction),
            3 => Ok(Self::MigrationUnverified),
            4 => Ok(Self::AdministrativeHold),
            _ => Err(ControlError::StoreCorrupt),
        }
    }
}

/// Exact bounded quarantine request. The owner supplies the observed diagnostic
/// SHA-256, not source content or a relabelled BLAKE3 digest. This command grants
/// no ownership and does not authorize repair or removing an existing hold.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ControlQuarantineRequest {
    operation_id: MutationId,
    expected_generation: u64,
    reason: ControlQuarantineReason,
    observation_sha256: [u8; 32],
}
impl ControlQuarantineRequest {
    /// Binds a stable operation, observed data generation and closed diagnosis.
    #[must_use]
    pub const fn new(operation_id: MutationId, expected_generation: u64,
        reason: ControlQuarantineReason, observation_sha256: [u8; 32]) -> Self {
        Self { operation_id, expected_generation, reason, observation_sha256 }
    }
    /// Exact idempotency identity of this administrative action.
    #[must_use]
    pub const fn operation_id(self) -> MutationId { self.operation_id }
    /// Generation that must still be present when the hold is recorded.
    #[must_use]
    pub const fn expected_generation(self) -> u64 { self.expected_generation }
    /// Owner-supplied closed diagnosis.
    #[must_use]
    pub const fn reason(self) -> ControlQuarantineReason { self.reason }
    /// Caller-supplied diagnostic observation hash; no content is retained here.
    #[must_use]
    pub const fn observation_sha256(self) -> [u8; 32] { self.observation_sha256 }
}
impl fmt::Debug for ControlQuarantineRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControlQuarantineRequest")
            .field("operation_id", &"<opaque>")
            .field("expected_generation", &self.expected_generation)
            .field("reason", &self.reason)
            .field("observation_sha256", &"<digest>").finish()
    }
}

/// Readback-verified durable hold. No constructor is exposed and no receipt
/// unquarantines the database or resolves an earlier uncertain data mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlQuarantineReceipt {
    identity: JournalIdentity,
    request: ControlQuarantineRequest,
}
impl ControlQuarantineReceipt {
    /// Exact journal identity verified while reading the marker.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity { self.identity }
    /// The exact administrative request recovered from durable bytes.
    #[must_use]
    pub const fn request(&self) -> ControlQuarantineRequest { self.request }
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct Marker {
    pub request: ControlQuarantineRequest,
    header_sha256: [u8; 32],
}
impl Marker {
    pub fn new(request: ControlQuarantineRequest, header_bytes: &[u8]) -> Self {
        Self { request, header_sha256: Sha256::digest(header_bytes).into() }
    }
    pub fn encode(&self) -> [u8; MARKER_BYTES] {
        let mut out = [0_u8; MARKER_BYTES];
        out[..8].copy_from_slice(MAGIC);
        out[8..40].copy_from_slice(&self.request.operation_id.0);
        out[40..48].copy_from_slice(&self.request.expected_generation.to_be_bytes());
        out[48] = self.request.reason.tag();
        out[49..81].copy_from_slice(&self.request.observation_sha256);
        out[81..PREFIX].copy_from_slice(&self.header_sha256);
        let checksum = checksum(&out[..PREFIX]);
        out[PREFIX..].copy_from_slice(&checksum);
        out
    }
    pub fn decode(bytes: &[u8], header_bytes: &[u8], generation: u64) -> Result<Self, ControlError> {
        if bytes.len() != MARKER_BYTES || &bytes[..8] != MAGIC
            || bytes[PREFIX..] != checksum(&bytes[..PREFIX]) {
            return Err(ControlError::StoreCorrupt);
        }
        let request = ControlQuarantineRequest::new(
            MutationId(array(&bytes[8..40])?), u64::from_be_bytes(array(&bytes[40..48])?),
            ControlQuarantineReason::decode(bytes[48])?, array(&bytes[49..81])?,
        );
        let marker = Self { request, header_sha256: array(&bytes[81..PREFIX])? };
        let actual: [u8; 32] = Sha256::digest(header_bytes).into();
        if request.expected_generation != generation || marker.header_sha256 != actual {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(marker)
    }
    pub fn receipt(&self, identity: JournalIdentity) -> ControlQuarantineReceipt {
        ControlQuarantineReceipt { identity, request: self.request }
    }
}
fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], ControlError> {
    bytes.try_into().map_err(|_| ControlError::StoreCorrupt)
}
fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"eliot-search/control-quarantine/sha256/v1\0");
    hash.update(bytes);
    hash.finalize().into()
}
