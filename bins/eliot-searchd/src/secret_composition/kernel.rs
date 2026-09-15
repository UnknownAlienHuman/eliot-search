//! Purpose-bound OS-secret leases for loopback pairing (T18).
//!
//! Daemon-side composition over the pure [`SecretCatalog`] lifecycle: one
//! loopback-pairing reference per composer, bound to an exact
//! installation/incarnation/user-scope/purpose tuple. Plaintext exists only
//! inside short-lived [`SecretLease`] values and is exposed to the endpoint
//! authenticator solely through callbacks; the keyed-BLAKE3 proofs over the
//! approved [`PairingTranscript`]s are computed by this secret-owning side.
//!
//! Platform notes:
//!
//! - This module performs no OS I/O and makes no encryption claim. Durable
//!   protection is the vault's job: on Windows the product vault is the OS
//!   Credential Manager entry itself (OS-encrypted at rest); the catalog
//!   ciphertext stored here is that vault blob, opaque to this layer.
//! - [`MemoryPairingVault`] is an explicit development/test seam only. It
//!   holds key material in process memory and is never an OS secret store;
//!   [`MemoryPairingVault::is_os_backed`] reports `false` so no caller can
//!   mistake it for one. There is no file/environment fallback advertised as
//!   a secret store.
//! - Native Credential Manager coverage lives in
//!   `bins/eliot-searchd/tests/pairing_process.rs`, whose test-local vault
//!   implements [`PairingVault`] with real `CredWrite`/`CredRead`/`CredDelete`
//!   traffic and Drop-based cleanup mirroring `tests/common`.
//!
//! Binding layers (both must hold for a connection to authenticate):
//!
//! 1. Lease layer: [`SecretLease::issue`] enforces the exact [`SecretBinding`]
//!    (installation, incarnation, user scope, closed purpose) and the finite
//!    lease window. A wrong binding or an expired lease fails before any key
//!    byte is exposed.
//! 2. Proof layer: [`derive_binding_digest`] binds the 32-byte key to the
//!    fixed `loopback-operator` role, and [`pairing_keyed_proof`] binds every
//!    proof to the negotiated version, binding digest, session, nonce and
//!    fresh challenge through the canonical transcripts built by
//!    `search-provider-protocol`. Per-request command/body binding composes
//!    the same way over `envelope_transcript` (T19 wire work owns the framing).

// Staging: this module is not yet wired into `entry.rs` (the controller adds
// the `mod` line plus the proxy lease-source wiring). Until then the whole
// public surface is construction-only. Mirrors the `sealed_*` staging pattern
// in `entry.rs`; the controller removes this allow once the module is wired.
#![allow(dead_code)]

use std::collections::BTreeMap;

use search_contracts::{
    Blake3Digest32, InstallationId, InstallationIncarnationId, NonZeroRevision, OpaqueId,
    ProtocolVersion, ReceiptRef,
};
use search_os_secrets::{
    DEFAULT_SECRET_LIMITS, EncryptedPayload, SecretBinding, SecretCatalog, SecretDeleteReadback,
    SecretError, SecretLease, SecretLimits, SecretOperation, SecretRecord, SecretRecordState,
    SecretReference, SecretWriteReadback,
};
use search_ports::{IdempotencyClass, MonotonicInstant, MutationIdentity};
use search_provider_protocol::pairing::{PairingTranscript, ProofDigest, verify_proof};

/// Closed purpose bound into every loopback-pairing reference.
pub const PAIRING_SECRET_PURPOSE: &str = "secret-purpose:loopback-pairing";
/// Exact pairing-key size in bytes; anything else fails closed.
pub const PAIRING_KEY_BYTES: usize = 32;
/// Negotiated loopback-pairing protocol version bound into every transcript.
pub const PAIRING_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };
/// Fixed role bound into the binding digest; substitution changes the digest.
pub const PAIRING_ROLE: &str = "loopback-operator";
/// Default finite lease lifetime in monotonic ticks (5 minutes at 1 tick/ms).
pub const DEFAULT_PAIRING_LEASE_TTL_TICKS: u64 = 300_000;
/// Maximum vault-blob bytes accepted into the catalog.
pub const MAX_VAULT_BLOB_BYTES: usize = 256;
/// Bounded recovery attempts after an ambiguous platform mutation.
pub const MAX_RECOVERY_ATTEMPTS: u8 = 3;

const BINDING_DOMAIN: &[u8] = b"eliot-search/loopback-binding/v1\0";
const OPERATION_DIGEST_DOMAIN: &[u8] = b"eliot-search/loopback-operation/v1\0";
const TEST_RNG_DOMAIN: &[u8] = b"eliot-search/loopback-test-rng/v1\0";

/// Closed pairing-secret composition failure with a stable machine code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretCompositionError {
    /// No pairing reference is provisioned.
    NotFound,
    /// Installation, incarnation, user-scope or purpose binding differs.
    BindingMismatch,
    /// Reference is not active and cannot be leased.
    NotLeaseable,
    /// Lease window is expired or otherwise unusable now.
    LeaseExpired,
    /// Key material is not exactly 32 bytes or is all zero.
    InvalidKeyMaterial,
    /// Requested lease lifetime is zero or overflows.
    InvalidTtl,
    /// No OS vault is available on this platform/configuration.
    VaultUnavailable,
    /// Durable vault write failed without a verifiable outcome.
    VaultWriteFailed,
    /// Durable vault read failed.
    VaultReadFailed,
    /// Expected vault evidence is absent while the record needs it.
    EvidenceMissing,
    /// Durable write/readback differs from the prepared mutation.
    ReadbackMismatch,
    /// A possible external mutation requires exact recovery.
    OutcomeUnknown,
    /// Requested lifecycle transition is invalid.
    InvalidTransition,
    /// Finite catalog or operation capacity was exhausted.
    CapacityExceeded,
    /// A shared version or revision cannot advance.
    ContractExhausted,
    /// Contradictory state requires quarantine.
    Quarantined,
    /// Ceremony material derivation produced a malformed value.
    EntropyInvalid,
}

impl SecretCompositionError {
    /// Stable machine-readable reason code; carries no secret material.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotFound => "PAIRING_SECRET_NOT_FOUND",
            Self::BindingMismatch => "PAIRING_SECRET_BINDING_MISMATCH",
            Self::NotLeaseable => "PAIRING_SECRET_NOT_LEASEABLE",
            Self::LeaseExpired => "PAIRING_SECRET_LEASE_EXPIRED",
            Self::InvalidKeyMaterial => "PAIRING_SECRET_KEY_INVALID",
            Self::InvalidTtl => "PAIRING_SECRET_TTL_INVALID",
            Self::VaultUnavailable => "PAIRING_SECRET_VAULT_UNAVAILABLE",
            Self::VaultWriteFailed => "PAIRING_SECRET_VAULT_WRITE_FAILED",
            Self::VaultReadFailed => "PAIRING_SECRET_VAULT_READ_FAILED",
            Self::EvidenceMissing => "PAIRING_SECRET_EVIDENCE_MISSING",
            Self::ReadbackMismatch => "PAIRING_SECRET_READBACK_MISMATCH",
            Self::OutcomeUnknown => "PAIRING_SECRET_OUTCOME_UNKNOWN",
            Self::InvalidTransition => "PAIRING_SECRET_INVALID_TRANSITION",
            Self::CapacityExceeded => "PAIRING_SECRET_CAPACITY_EXCEEDED",
            Self::ContractExhausted => "PAIRING_SECRET_CONTRACT_EXHAUSTED",
            Self::Quarantined => "PAIRING_SECRET_QUARANTINED",
            Self::EntropyInvalid => "PAIRING_SECRET_ENTROPY_INVALID",
        }
    }
}

impl core::fmt::Display for SecretCompositionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SecretCompositionError {}

impl From<SecretError> for SecretCompositionError {
    fn from(error: SecretError) -> Self {
        match error {
            SecretError::NotFound => Self::NotFound,
            SecretError::BindingMismatch => Self::BindingMismatch,
            SecretError::NotLeaseable => Self::NotLeaseable,
            SecretError::InvalidLeaseWindow => Self::LeaseExpired,
            SecretError::OperationConflict
            | SecretError::AlreadyExists
            | SecretError::VersionMismatch
            | SecretError::RecordRevisionMismatch
            | SecretError::InvalidTransition => Self::InvalidTransition,
            SecretError::CapacityExceeded => Self::CapacityExceeded,
            SecretError::ContractExhausted => Self::ContractExhausted,
            SecretError::EmptySecret | SecretError::SecretTooLarge => Self::InvalidKeyMaterial,
            SecretError::ReadbackMismatch => Self::ReadbackMismatch,
            SecretError::RecoveryEvidenceMissing => Self::EvidenceMissing,
            SecretError::OutcomeUnknown => Self::OutcomeUnknown,
            SecretError::Quarantined => Self::Quarantined,
        }
    }
}

/// Builds the exact purpose-bound authority tuple for loopback pairing.
pub fn pairing_binding(
    installation_id: InstallationId,
    installation_incarnation_id: InstallationIncarnationId,
    user_scope_digest: Blake3Digest32,
) -> Result<SecretBinding, SecretCompositionError> {
    let purpose =
        OpaqueId::new(PAIRING_SECRET_PURPOSE).map_err(|_| SecretCompositionError::Quarantined)?;
    Ok(SecretBinding::new(
        installation_id,
        installation_incarnation_id,
        user_scope_digest,
        purpose,
    ))
}

/// Derives the deterministic reference identity for one mutation.
///
/// The identity names the exact operation digest, so re-provisioning with the
/// same operation is idempotent while a different operation never collides.
pub fn pairing_reference_id(
    operation: &SecretOperation,
) -> Result<OpaqueId, SecretCompositionError> {
    OpaqueId::new(format!(
        "secret:loopback-pairing:{}",
        operation.request_digest()
    ))
    .map_err(|_| SecretCompositionError::Quarantined)
}

/// Mints a replay-fenced mutation identity for one lifecycle step.
///
/// `tag` names the step (`provision`, `rotate`, `revoke`); `nonce` must be
/// fresh per attempt. The request digest binds the closed tag plus the nonce
/// under a fixed domain, so reusing an operation identity with other bytes is
/// detected as [`SecretError::OperationConflict`].
pub fn fresh_operation(
    tag: &str,
    nonce: &[u8; 32],
) -> Result<SecretOperation, SecretCompositionError> {
    if tag.is_empty()
        || tag.len() > 32
        || !tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(SecretCompositionError::InvalidTransition);
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(OPERATION_DIGEST_DOMAIN);
    hasher.update(tag.as_bytes());
    hasher.update(&[0]);
    hasher.update(nonce);
    let digest = Blake3Digest32::from_bytes(*hasher.finalize().as_bytes());
    let operation_id = OpaqueId::new(format!("secret-operation:loopback-pairing:{tag}:{digest}"))
        .map_err(|_| SecretCompositionError::CapacityExceeded)?;
    Ok(SecretOperation::new(
        MutationIdentity::new(operation_id, IdempotencyClass::RetrySameIdentity),
        digest,
    ))
}

/// Content-free evidence of one durable vault write.
///
/// The receipt names the exact blob digest observed on readback, so it is
/// derived from executed evidence rather than fabricated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultWriteEvidence {
    /// Digest of the exact bytes the vault returned on readback.
    pub blob_digest: Blake3Digest32,
    /// Content-free receipt naming the observed digest.
    pub receipt: ReceiptRef,
}

/// Durable key-vault boundary owned by the platform adapter.
///
/// The composer drives provisioning, rotation, revocation and absence
/// readback through this trait; the trait implementation owns all OS I/O.
/// Blobs are opaque to the composer: exactly what `load_blob` returns after
/// `store_blob` is what the catalog lifecycle binds.
pub trait PairingVault {
    /// Draws one fresh 32-byte key. Non-zero by construction.
    fn generate_key(&mut self) -> Result<zeroize::Zeroizing<[u8; 32]>, SecretCompositionError>;
    /// Durably stores the key for `id`, overwriting any previous blob.
    fn store_blob(
        &mut self,
        id: &OpaqueId,
        key: &[u8; 32],
    ) -> Result<VaultWriteEvidence, SecretCompositionError>;
    /// Reads the exact blob for `id`; `None` means verifiably absent.
    fn load_blob(&mut self, id: &OpaqueId) -> Result<Option<Vec<u8>>, SecretCompositionError>;
    /// Deletes the blob for `id`; absence afterwards is verified by the
    /// composer through [`PairingVault::load_blob`], never trusted here.
    fn remove_blob(&mut self, id: &OpaqueId) -> Result<(), SecretCompositionError>;
    /// Whether this vault is OS-backed. The memory seam reports `false`.
    fn is_os_backed(&self) -> bool;
}

/// Explicit development/test vault holding blobs in process memory.
///
/// This is never an OS secret store: blobs sit in heap memory, and
/// [`PairingVault::is_os_backed`] reports `false`. All blobs are zeroized on
/// drop and on overwrite/remove. Deterministic key draws come from a
/// counter-keyed hash (test-only distribution, documented here, never
/// advertised as OS randomness).
#[derive(Debug, Default)]
pub struct MemoryPairingVault {
    blobs: BTreeMap<String, Vec<u8>>,
    draws: u64,
    /// When set, the next `remove_blob` reports success but keeps the blob,
    /// simulating a timeout after a possible external write.
    pub fail_next_remove_ambiguously: bool,
    /// When set, the next `store_blob` persists the blob but reports a write
    /// failure, simulating a timeout after a possible external write.
    pub fail_next_store_ambiguously: bool,
}

impl MemoryPairingVault {
    /// Creates an empty memory vault.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Directly drops the blob without touching the catalog, simulating
    /// external loss. Test-only fault injection.
    pub fn drop_blob_for_test(&mut self, id: &OpaqueId) {
        if let Some(mut blob) = self.blobs.remove(id.as_str()) {
            blob.fill(0);
        }
    }
}

impl Drop for MemoryPairingVault {
    fn drop(&mut self) {
        for blob in self.blobs.values_mut() {
            blob.fill(0);
        }
    }
}

impl PairingVault for MemoryPairingVault {
    fn generate_key(&mut self) -> Result<zeroize::Zeroizing<[u8; 32]>, SecretCompositionError> {
        self.draws = self
            .draws
            .checked_add(1)
            .ok_or(SecretCompositionError::ContractExhausted)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(TEST_RNG_DOMAIN);
        hasher.update(&self.draws.to_be_bytes());
        let mut key = *hasher.finalize().as_bytes();
        if key.iter().all(|byte| *byte == 0) {
            key[0] = 1;
        }
        Ok(zeroize::Zeroizing::new(key))
    }

    fn store_blob(
        &mut self,
        id: &OpaqueId,
        key: &[u8; 32],
    ) -> Result<VaultWriteEvidence, SecretCompositionError> {
        if key.len() != PAIRING_KEY_BYTES || key.iter().all(|byte| *byte == 0) {
            return Err(SecretCompositionError::InvalidKeyMaterial);
        }
        if let Some(previous) = self.blobs.get_mut(id.as_str()) {
            previous.fill(0);
        }
        self.blobs.insert(id.as_str().to_owned(), key.to_vec());
        let blob_digest = Blake3Digest32::from_bytes(*blake3::hash(key).as_bytes());
        let receipt = VaultWriteEvidence::receipt_for(blob_digest)?;
        if self.fail_next_store_ambiguously {
            self.fail_next_store_ambiguously = false;
            return Err(SecretCompositionError::VaultWriteFailed);
        }
        Ok(VaultWriteEvidence {
            blob_digest,
            receipt,
        })
    }

    fn load_blob(&mut self, id: &OpaqueId) -> Result<Option<Vec<u8>>, SecretCompositionError> {
        Ok(self.blobs.get(id.as_str()).cloned())
    }

    fn remove_blob(&mut self, id: &OpaqueId) -> Result<(), SecretCompositionError> {
        if self.fail_next_remove_ambiguously {
            self.fail_next_remove_ambiguously = false;
            return Ok(());
        }
        if let Some(mut blob) = self.blobs.remove(id.as_str()) {
            blob.fill(0);
        }
        Ok(())
    }

    fn is_os_backed(&self) -> bool {
        false
    }
}

impl VaultWriteEvidence {
    /// Builds the content-free receipt naming one observed blob digest.
    ///
    /// Platform adapters call this after verifying their own write by exact
    /// readback; the receipt derives from executed evidence, never invented.
    pub fn receipt_for(blob_digest: Blake3Digest32) -> Result<ReceiptRef, SecretCompositionError> {
        ReceiptRef::new(format!("receipt:loopback-pairing:vault:{blob_digest}"))
            .map_err(|_| SecretCompositionError::ContractExhausted)
    }
}

/// Content-free provisioning receipt: what was created, never its key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvisionReceipt {
    /// Opaque bound reference now active.
    pub reference: SecretReference,
    /// Durable record revision (1 on first provisioning).
    pub record_revision: NonZeroRevision,
    /// Vault-readback receipt naming the observed blob digest.
    pub receipt: ReceiptRef,
}

/// Content-free rotation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RotationReceipt {
    /// Opaque bound reference after rotation.
    pub reference: SecretReference,
    /// Durable record revision advanced exactly once.
    pub record_revision: NonZeroRevision,
    /// Vault-readback receipt naming the observed replacement digest.
    pub receipt: ReceiptRef,
}

/// Content-free revocation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevocationReceipt {
    /// Opaque bound reference now durably deleted.
    pub reference: SecretReference,
    /// Final durable record revision.
    pub record_revision: NonZeroRevision,
    /// Absence-readback receipt.
    pub receipt: ReceiptRef,
}

/// How a mutation reached durability: cleanly or through exact recovery.
///
/// Partial/degraded outcomes stay typed here and are never relabeled as
/// plain success.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MutationOutcome<R> {
    /// Platform write confirmed on the first attempt.
    Committed(R),
    /// Platform write was ambiguous; exact readback proved the outcome.
    Recovered(R),
}

/// Purpose-bound pairing-secret composer: catalog lifecycle plus vault
/// effects plus lease-bound keyed proofs.
///
/// Exactly one active reference is managed: provisioning while one is active
/// is refused (rotate instead), and revocation clears the slot. The composer
/// holds no key material itself; every proof goes through a short-lived
/// [`SecretLease`] inside one callback.
#[derive(Debug)]
pub struct PairingSecretComposer {
    catalog: SecretCatalog,
    binding: SecretBinding,
    active_id: Option<OpaqueId>,
    limits: SecretLimits,
}

/// Verified replacement inputs for one rotation commit.
///
/// Bundles the exact readback evidence so the commit path stays within its
/// finite argument budget.
struct PreparedRotation<'a> {
    observed: Vec<u8>,
    evidence: &'a VaultWriteEvidence,
    replacement_version: NonZeroRevision,
    replacement_revision: NonZeroRevision,
    operation: &'a SecretOperation,
    recovered: bool,
}

impl PairingSecretComposer {
    /// Creates a composer for one exact authority binding.
    pub fn new(binding: SecretBinding) -> Result<Self, SecretCompositionError> {
        Self::with_limits(binding, DEFAULT_SECRET_LIMITS)
    }

    /// Creates a composer with explicit finite limits.
    pub fn with_limits(
        binding: SecretBinding,
        limits: SecretLimits,
    ) -> Result<Self, SecretCompositionError> {
        if limits.validate().is_err() {
            return Err(SecretCompositionError::CapacityExceeded);
        }
        Ok(Self {
            catalog: SecretCatalog::with_limits(limits)?,
            binding,
            active_id: None,
            limits,
        })
    }

    /// Exact authority binding this composer serves.
    #[must_use]
    pub const fn binding(&self) -> &SecretBinding {
        &self.binding
    }

    /// Currently active reference identity, if any.
    #[must_use]
    pub const fn active_id(&self) -> Option<&OpaqueId> {
        self.active_id.as_ref()
    }

    /// Whether the active reference (if any) is currently leaseable.
    #[must_use]
    pub fn has_active_leaseable(&self) -> bool {
        self.active_id.as_ref().is_some_and(|id| {
            self.catalog
                .get(id)
                .is_ok_and(|record| matches!(record.state(), SecretRecordState::Active))
        })
    }

    /// Provisions the first active pairing secret through the vault.
    ///
    /// Fails closed when a reference is already active (rotate instead),
    /// when the vault write cannot be verified by exact readback, or when
    /// the operation identity is reused with other bytes.
    pub fn provision(
        &mut self,
        vault: &mut impl PairingVault,
        operation: SecretOperation,
    ) -> Result<ProvisionReceipt, SecretCompositionError> {
        if self.has_active_leaseable() {
            return Err(SecretCompositionError::InvalidTransition);
        }
        let id = pairing_reference_id(&operation)?;
        if self.catalog.get(&id).is_ok() {
            return Err(SecretCompositionError::InvalidTransition);
        }
        let key = vault.generate_key()?;
        check_key_bytes(&key[..])?;
        let evidence = vault.store_blob(&id, &key)?;
        let observed = vault
            .load_blob(&id)?
            .ok_or(SecretCompositionError::EvidenceMissing)?;
        verify_blob_readback(&observed, &evidence)?;
        let payload = EncryptedPayload::new(observed, self.limits.max_ciphertext_bytes)
            .map_err(SecretCompositionError::from)?;
        let one = NonZeroRevision::new(1).map_err(|_| SecretCompositionError::Quarantined)?;
        let reference = SecretReference::new(id.clone(), self.binding.clone(), one);
        let record = SecretRecord::new_active(
            reference.clone(),
            payload,
            evidence.blob_digest,
            one,
            operation,
        );
        self.catalog.create(record)?;
        self.active_id = Some(id);
        Ok(ProvisionReceipt {
            reference,
            record_revision: one,
            receipt: evidence.receipt,
        })
    }

    /// Rotates the active secret exactly one version forward.
    ///
    /// An ambiguous platform write is not reported as success: the mutation
    /// is resolved by exact readback and reported as a typed
    /// [`MutationOutcome::Recovered`] receipt. A readback that matches
    /// neither the replacement nor the predecessor quarantines instead of
    /// diverging.
    pub fn rotate(
        &mut self,
        vault: &mut impl PairingVault,
        operation: &SecretOperation,
    ) -> Result<MutationOutcome<RotationReceipt>, SecretCompositionError> {
        let id = self
            .active_id
            .clone()
            .ok_or(SecretCompositionError::NotFound)?;
        let current = self.catalog.get(&id)?.clone();
        if !matches!(current.state(), SecretRecordState::Active) {
            return Err(SecretCompositionError::NotLeaseable);
        }
        if current.reference().binding() != &self.binding {
            return Err(SecretCompositionError::BindingMismatch);
        }
        let replacement_version = current
            .reference()
            .version()
            .checked_next()
            .map_err(|_| SecretCompositionError::ContractExhausted)?;
        let replacement_revision = current
            .record_revision()
            .checked_next()
            .map_err(|_| SecretCompositionError::ContractExhausted)?;
        let key = vault.generate_key()?;
        check_key_bytes(&key[..])?;
        let replacement_digest = Blake3Digest32::from_bytes(*blake3::hash(&key[..]).as_bytes());
        let Ok(evidence) = vault.store_blob(&id, &key) else {
            return self.resolve_ambiguous_store(
                vault,
                &id,
                replacement_digest,
                replacement_version,
                replacement_revision,
                operation,
            );
        };
        if evidence.blob_digest != replacement_digest {
            let _ = self.catalog.mark_outcome_unknown(&id, operation);
            return Err(SecretCompositionError::ReadbackMismatch);
        }
        let observed = vault
            .load_blob(&id)?
            .ok_or(SecretCompositionError::EvidenceMissing)?;
        verify_blob_readback(&observed, &evidence)?;
        self.commit_prepared_rotation(
            vault,
            &id,
            PreparedRotation {
                observed,
                evidence: &evidence,
                replacement_version,
                replacement_revision,
                operation,
                recovered: false,
            },
        )
    }

    /// Resolves an ambiguous rotation write by exact vault readback.
    ///
    /// The catalog is still Active here, which makes the decision exact:
    /// replacement bytes mean the write landed (prepare and confirm now,
    /// reported as recovered), anything else means it did not (retryable
    /// failure, no pending state left behind).
    fn resolve_ambiguous_store(
        &mut self,
        vault: &mut impl PairingVault,
        id: &OpaqueId,
        replacement_digest: Blake3Digest32,
        replacement_version: NonZeroRevision,
        replacement_revision: NonZeroRevision,
        operation: &SecretOperation,
    ) -> Result<MutationOutcome<RotationReceipt>, SecretCompositionError> {
        let observed = vault
            .load_blob(id)
            .map_err(|_| SecretCompositionError::OutcomeUnknown)?;
        let Some(bytes) = observed else {
            return Err(SecretCompositionError::VaultWriteFailed);
        };
        if Blake3Digest32::from_bytes(*blake3::hash(&bytes).as_bytes()) != replacement_digest {
            return Err(SecretCompositionError::VaultWriteFailed);
        }
        let evidence = VaultWriteEvidence {
            blob_digest: replacement_digest,
            receipt: VaultWriteEvidence::receipt_for(replacement_digest)?,
        };
        self.commit_prepared_rotation(
            vault,
            id,
            PreparedRotation {
                observed: bytes,
                evidence: &evidence,
                replacement_version,
                replacement_revision,
                operation,
                recovered: true,
            },
        )
    }

    /// Prepares and confirms a rotation whose replacement bytes were verified
    /// in the vault. `recovered` marks whether the durable write needed exact
    /// readback recovery after an ambiguous platform outcome.
    fn commit_prepared_rotation(
        &mut self,
        vault: &mut impl PairingVault,
        id: &OpaqueId,
        prepared: PreparedRotation<'_>,
    ) -> Result<MutationOutcome<RotationReceipt>, SecretCompositionError> {
        let PreparedRotation {
            observed,
            evidence,
            replacement_version,
            replacement_revision,
            operation,
            recovered,
        } = prepared;
        let payload = EncryptedPayload::new(observed, self.limits.max_ciphertext_bytes)
            .map_err(SecretCompositionError::from)?;
        let effect = self.catalog.prepare_rotation(
            id,
            &self.binding,
            replacement_version,
            payload,
            evidence.blob_digest,
            replacement_revision,
            operation.clone(),
        );
        let (replacement_reference, replacement_operation) = match effect {
            Ok(search_os_secrets::SecretMutationEffect::WriteEncrypted {
                reference,
                operation,
                ..
            }) => (reference, operation),
            // The rotation effect can never be a delete; report the fence
            // verdict directly. On error no pending mutation was installed,
            // so a healthy Active record is never quarantined here.
            Ok(search_os_secrets::SecretMutationEffect::Delete { .. }) => {
                return Err(SecretCompositionError::Quarantined);
            }
            Err(error) => return Err(error.into()),
        };
        let readback = SecretWriteReadback {
            reference: replacement_reference,
            ciphertext_digest: evidence.blob_digest,
            record_revision: replacement_revision,
            operation: replacement_operation,
            durable_receipt: Some(evidence.receipt.clone()),
        };
        if let Ok(confirmed) = self.catalog.confirm_rotation(id, &readback) {
            let receipt = RotationReceipt {
                reference: confirmed.reference,
                record_revision: confirmed.record_revision,
                receipt: confirmed.durable_receipt,
            };
            if recovered {
                Ok(MutationOutcome::Recovered(receipt))
            } else {
                Ok(MutationOutcome::Committed(receipt))
            }
        } else {
            let _ = self.catalog.mark_outcome_unknown(id, operation);
            self.recover_rotation(vault, id, operation)
        }
    }

    fn recover_rotation(
        &mut self,
        vault: &mut impl PairingVault,
        id: &OpaqueId,
        operation: &SecretOperation,
    ) -> Result<MutationOutcome<RotationReceipt>, SecretCompositionError> {
        let observed = vault
            .load_blob(id)?
            .ok_or(SecretCompositionError::EvidenceMissing)?;
        if observed.len() != PAIRING_KEY_BYTES {
            self.quarantine_active_record(id);
            return Err(SecretCompositionError::ReadbackMismatch);
        }
        let observed_digest = Blake3Digest32::from_bytes(*blake3::hash(&observed).as_bytes());
        let current = self.catalog.get(id)?.clone();
        let (expected_version, expected_revision) = match current.state() {
            SecretRecordState::RotationPending(_) | SecretRecordState::OutcomeUnknown(_) => {
                let version = current
                    .reference()
                    .version()
                    .checked_next()
                    .map_err(|_| SecretCompositionError::ContractExhausted)?;
                let revision = current
                    .record_revision()
                    .checked_next()
                    .map_err(|_| SecretCompositionError::ContractExhausted)?;
                (version, revision)
            }
            _ => return Err(SecretCompositionError::OutcomeUnknown),
        };
        let expected_reference =
            SecretReference::new(id.clone(), self.binding.clone(), expected_version);
        let receipt = VaultWriteEvidence::receipt_for(observed_digest)?;
        let readback = SecretWriteReadback {
            reference: expected_reference,
            ciphertext_digest: observed_digest,
            record_revision: expected_revision,
            operation: operation.clone(),
            durable_receipt: Some(receipt),
        };
        if let Ok(confirmed) = self.catalog.recover_rotation(id, &readback) {
            Ok(MutationOutcome::Recovered(RotationReceipt {
                reference: confirmed.reference,
                record_revision: confirmed.record_revision,
                receipt: confirmed.durable_receipt,
            }))
        } else {
            self.quarantine_active_record(id);
            Err(SecretCompositionError::OutcomeUnknown)
        }
    }

    /// Revokes the active secret, proving absence by exact readback.
    ///
    /// Deletion is confirmed only when the vault reports the blob verifiably
    /// absent. An ambiguous delete is recovered by bounded re-read/retry and
    /// reported as [`MutationOutcome::Recovered`]; a blob that survives
    /// bounded deletion quarantines instead of being relabeled deleted.
    pub fn revoke(
        &mut self,
        vault: &mut impl PairingVault,
        operation: &SecretOperation,
    ) -> Result<MutationOutcome<RevocationReceipt>, SecretCompositionError> {
        let id = self
            .active_id
            .clone()
            .ok_or(SecretCompositionError::NotFound)?;
        let current = self.catalog.get(&id)?.clone();
        if !matches!(current.state(), SecretRecordState::Active) {
            return Err(SecretCompositionError::NotLeaseable);
        }
        if current.reference().binding() != &self.binding {
            return Err(SecretCompositionError::BindingMismatch);
        }
        let effect = self
            .catalog
            .prepare_delete(&id, &self.binding, operation.clone());
        let pending_operation = match effect {
            Ok(search_os_secrets::SecretMutationEffect::Delete { operation, .. }) => operation,
            Ok(search_os_secrets::SecretMutationEffect::WriteEncrypted { .. }) | Err(_) => {
                return Err(SecretCompositionError::InvalidTransition);
            }
        };
        if vault.remove_blob(&id).is_err() {
            let _ = self.catalog.mark_outcome_unknown(&id, &pending_operation);
            return self.recover_delete(vault, &id, &pending_operation);
        }
        self.confirm_absent_delete(vault, &id, &pending_operation, false)
    }

    fn recover_delete(
        &mut self,
        vault: &mut impl PairingVault,
        id: &OpaqueId,
        operation: &SecretOperation,
    ) -> Result<MutationOutcome<RevocationReceipt>, SecretCompositionError> {
        for _ in 0..MAX_RECOVERY_ATTEMPTS {
            match vault.load_blob(id) {
                Ok(None) => return self.confirm_absent_delete(vault, id, operation, true),
                Ok(Some(_)) => {
                    let _ = vault.remove_blob(id);
                }
                Err(_) => {}
            }
        }
        if vault.load_blob(id).is_ok_and(|blob| blob.is_none()) {
            self.confirm_absent_delete(vault, id, operation, true)
        } else {
            self.quarantine_active_record(id);
            Err(SecretCompositionError::OutcomeUnknown)
        }
    }

    fn confirm_absent_delete(
        &mut self,
        vault: &mut impl PairingVault,
        id: &OpaqueId,
        operation: &SecretOperation,
        recovered: bool,
    ) -> Result<MutationOutcome<RevocationReceipt>, SecretCompositionError> {
        // Absence readback: the blob must be verifiably gone.
        if vault.load_blob(id)?.is_some() {
            let _ = self.catalog.mark_outcome_unknown(id, operation);
            return self.recover_delete(vault, id, operation);
        }
        let receipt = VaultWriteEvidence::receipt_for(Blake3Digest32::from_bytes([0xA5; 32]))?;
        let readback = SecretDeleteReadback {
            reference_absent: true,
            operation: operation.clone(),
            durable_receipt: Some(receipt),
        };
        let confirmed = match self.catalog.confirm_delete(id, &readback) {
            Ok(confirmed) => confirmed,
            Err(SecretError::InvalidTransition) => {
                // The mutation was marked outcome-unknown by an ambiguous
                // platform call above; recover through the unknown path.
                self.catalog
                    .recover_delete(id, &readback)
                    .map_err(SecretCompositionError::from)?
            }
            Err(error) => return Err(error.into()),
        };
        self.active_id = None;
        let receipt_out = RevocationReceipt {
            reference: confirmed.reference,
            record_revision: confirmed.record_revision,
            receipt: confirmed.durable_receipt,
        };
        if recovered {
            Ok(MutationOutcome::Recovered(receipt_out))
        } else {
            Ok(MutationOutcome::Committed(receipt_out))
        }
    }

    /// Verifies absence: no leaseable record and no vault blob.
    ///
    /// After revocation both must hold. A leaseable record without a vault
    /// blob (external loss) reports `false` rather than absence.
    pub fn absence_verified(
        &self,
        vault: &mut impl PairingVault,
    ) -> Result<bool, SecretCompositionError> {
        let leaseable = self.has_active_leaseable();
        let blob_present = match self.active_id.clone() {
            Some(id) => vault.load_blob(&id)?.is_some(),
            None => false,
        };
        Ok(!leaseable && !blob_present)
    }

    /// Issues one finite plaintext lease for the active reference.
    ///
    /// The vault blob is the plaintext: it must be present and exactly 32
    /// bytes, and the record must be active under the exact binding.
    pub fn issue_lease(
        &self,
        vault: &mut impl PairingVault,
        now: MonotonicInstant,
        ttl_ticks: u64,
    ) -> Result<SecretLease, SecretCompositionError> {
        let id = self
            .active_id
            .clone()
            .ok_or(SecretCompositionError::NotFound)?;
        let record = self.catalog.get(&id)?.clone();
        let (issued_at, expires_at) = lease_window(now, ttl_ticks)?;
        let plaintext = vault
            .load_blob(&id)?
            .ok_or(SecretCompositionError::EvidenceMissing)?;
        if plaintext.len() != PAIRING_KEY_BYTES {
            return Err(SecretCompositionError::InvalidKeyMaterial);
        }
        Ok(SecretLease::issue(
            &record,
            &self.binding,
            issued_at,
            expires_at,
            plaintext,
            self.limits,
        )?)
    }

    /// Exposes the active 32-byte key only for the callback duration.
    ///
    /// The lease window is enforced at `now`; the key never escapes by value.
    /// This is the sole path the endpoint authenticator uses to compute or
    /// verify keyed proofs.
    pub fn with_pairing_key<T>(
        &self,
        vault: &mut impl PairingVault,
        now: MonotonicInstant,
        use_key: impl FnOnce(&[u8; 32]) -> T,
    ) -> Result<T, SecretCompositionError> {
        let lease = self.issue_lease(vault, now, DEFAULT_PAIRING_LEASE_TTL_TICKS)?;
        lease
            .with_secret(now, |bytes| {
                let key: &[u8; 32] = bytes
                    .try_into()
                    .map_err(|_| SecretCompositionError::InvalidKeyMaterial)?;
                if key.iter().all(|byte| *byte == 0) {
                    return Err(SecretCompositionError::InvalidKeyMaterial);
                }
                Ok(use_key(key))
            })
            .map_err(SecretCompositionError::from)?
    }

    /// Explicitly quarantines the active reference; leases stop immediately.
    pub fn quarantine_active(&mut self, reason: SecretError) -> Result<(), SecretCompositionError> {
        let id = self
            .active_id
            .clone()
            .ok_or(SecretCompositionError::NotFound)?;
        self.catalog.quarantine(&id, reason)?;
        Ok(())
    }

    fn quarantine_active_record(&mut self, id: &OpaqueId) {
        let _ = self.catalog.quarantine(id, SecretError::Quarantined);
    }
}

/// Computes the finite lease window `[now, now + ttl)`.
pub fn lease_window(
    now: MonotonicInstant,
    ttl_ticks: u64,
) -> Result<(MonotonicInstant, MonotonicInstant), SecretCompositionError> {
    if ttl_ticks == 0 {
        return Err(SecretCompositionError::InvalidTtl);
    }
    let expires = now
        .ticks()
        .checked_add(ttl_ticks)
        .ok_or(SecretCompositionError::InvalidTtl)?;
    Ok((now, MonotonicInstant::from_ticks(expires)))
}

/// Derives the role-bound binding digest for one pairing key.
///
/// The digest binds the fixed loopback role to the key under a fixed domain.
/// Installation/incarnation/purpose binding is enforced one layer below, at
/// lease issuance through the exact [`SecretBinding`]; this digest is what
/// travels in the clear inside pairing transcripts. It must equal the
/// endpoint's `pairing_binding_digest` on the same key; the process test
/// proves that agreement on fixed vectors.
#[must_use]
pub fn derive_binding_digest(key: &[u8; 32]) -> ProofDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(BINDING_DOMAIN);
    hasher.update(PAIRING_ROLE.as_bytes());
    hasher.update(&[0]);
    hasher.update(key);
    ProofDigest::from_bytes(*hasher.finalize().as_bytes())
}

/// Computes the keyed proof over one exact pairing transcript.
///
/// The secret-owning side alone calls this: the key never leaves the lease
/// callback, and only the 32-byte digest crosses to the peer.
#[must_use]
pub fn pairing_keyed_proof(key: &[u8; 32], transcript: &PairingTranscript) -> ProofDigest {
    pairing_keyed_proof_raw(key, transcript.as_bytes())
}

/// Computes the keyed proof over exact envelope bytes.
///
/// Same key, same primitive as [`pairing_keyed_proof`]: per-request
/// command/body transcripts from `request.rs` compose with pairing keys
/// without a second authenticator.
#[must_use]
pub fn pairing_keyed_proof_raw(key: &[u8; 32], bytes: &[u8]) -> ProofDigest {
    ProofDigest::from_bytes(*blake3::keyed_hash(key, bytes).as_bytes())
}

/// Verifies a pairing proof in fixed-work time.
#[must_use]
pub fn verify_pairing_proof(expected: &ProofDigest, observed: &ProofDigest) -> bool {
    verify_proof(expected, observed)
}

/// Process-local monotonic clock for lease windows.
///
/// Ticks are milliseconds since first call in this process; values are
/// meaningful only inside this process incarnation and are never serialized.
#[must_use]
pub fn monotonic_now() -> MonotonicInstant {
    static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let epoch = EPOCH.get_or_init(std::time::Instant::now);
    let millis = epoch.elapsed().as_millis();
    MonotonicInstant::from_ticks(u64::try_from(millis).unwrap_or(u64::MAX))
}

fn check_key_bytes(key: &[u8]) -> Result<(), SecretCompositionError> {
    if key.len() != PAIRING_KEY_BYTES || key.iter().all(|byte| *byte == 0) {
        return Err(SecretCompositionError::InvalidKeyMaterial);
    }
    Ok(())
}

fn verify_blob_readback(
    observed: &[u8],
    evidence: &VaultWriteEvidence,
) -> Result<(), SecretCompositionError> {
    if observed.len() > MAX_VAULT_BLOB_BYTES {
        return Err(SecretCompositionError::ReadbackMismatch);
    }
    let digest = Blake3Digest32::from_bytes(*blake3::hash(observed).as_bytes());
    if digest != evidence.blob_digest {
        return Err(SecretCompositionError::ReadbackMismatch);
    }
    Ok(())
}

/// Test-only deterministic nonce; production callers pass OS randomness.
#[must_use]
pub fn test_nonce(seed: u8) -> [u8; 32] {
    let mut nonce = [0_u8; 32];
    for (index, slot) in nonce.iter_mut().enumerate() {
        *slot = seed.wrapping_add(u8::try_from(index).unwrap_or(0)).max(1);
    }
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::RequestId;
    use search_provider_protocol::pairing::{
        ClientNonce as ProtoClientNonce, PairingChallenge, PairingMachine,
        SessionId as ProtoSessionId, client_proof_transcript, server_proof_transcript,
    };
    use search_provider_protocol::request::{
        ControlCommand, envelope_transcript, seal_envelope, verify_envelope_proof,
    };

    fn test_binding() -> SecretBinding {
        pairing_binding(
            InstallationId::from_bytes([0x11; 16]),
            InstallationIncarnationId::from_bytes([0x22; 16]),
            Blake3Digest32::from_bytes([0x33; 32]),
        )
        .expect("binding")
    }

    fn composer() -> PairingSecretComposer {
        PairingSecretComposer::new(test_binding()).expect("composer")
    }

    fn provisioned() -> (PairingSecretComposer, MemoryPairingVault) {
        let mut composer = composer();
        let mut vault = MemoryPairingVault::new();
        let operation = fresh_operation("provision", &test_nonce(1)).expect("operation");
        composer
            .provision(&mut vault, operation)
            .expect("provision");
        (composer, vault)
    }

    #[test]
    fn memory_vault_is_explicitly_not_an_os_store() {
        assert!(!MemoryPairingVault::new().is_os_backed());
    }

    #[test]
    fn provision_issues_a_bound_lease_and_a_stable_binding_digest() {
        let (composer, mut vault) = provisioned();
        let now = MonotonicInstant::from_ticks(1_000);
        let first = composer
            .with_pairing_key(&mut vault, now, derive_binding_digest)
            .expect("key");
        let second = composer
            .with_pairing_key(&mut vault, now, derive_binding_digest)
            .expect("key");
        assert_eq!(first, second);
        // Role substitution changes the digest: the digest binds the role.
        let other = composer
            .with_pairing_key(&mut vault, now, |key| {
                let mut hasher = blake3::Hasher::new();
                hasher.update(BINDING_DOMAIN);
                hasher.update(b"other-role");
                hasher.update(&[0]);
                hasher.update(key);
                ProofDigest::from_bytes(*hasher.finalize().as_bytes())
            })
            .expect("key");
        assert_ne!(first, other);
    }

    #[test]
    fn second_provision_without_rotation_is_refused() {
        let (mut composer, mut vault) = provisioned();
        let operation = fresh_operation("provision", &test_nonce(9)).expect("operation");
        assert_eq!(
            composer.provision(&mut vault, operation),
            Err(SecretCompositionError::InvalidTransition)
        );
    }

    #[test]
    fn cross_binding_lease_is_denied() {
        let (mut composer, mut vault) = provisioned();
        // Tamper the composer's binding: the catalog record still carries the
        // original, so issuance must fail with a binding mismatch.
        composer.binding = pairing_binding(
            InstallationId::from_bytes([0x99; 16]),
            InstallationIncarnationId::from_bytes([0x22; 16]),
            Blake3Digest32::from_bytes([0x33; 32]),
        )
        .expect("binding");
        assert!(matches!(
            composer.issue_lease(&mut vault, MonotonicInstant::from_ticks(1_000), 60_000),
            Err(SecretCompositionError::BindingMismatch)
        ));
    }

    #[test]
    fn expired_lease_never_exposes_the_key() {
        let (composer, mut vault) = provisioned();
        let lease = composer
            .issue_lease(&mut vault, MonotonicInstant::from_ticks(1_000), 100)
            .expect("lease");
        assert!(lease.is_valid_at(MonotonicInstant::from_ticks(1_000)));
        assert!(!lease.is_valid_at(MonotonicInstant::from_ticks(1_100)));
        assert_eq!(
            lease
                .with_secret(MonotonicInstant::from_ticks(1_100), |_| ())
                .map_err(SecretCompositionError::from),
            Err(SecretCompositionError::LeaseExpired)
        );
        // The composer path enforces the same window.
        let fresh = composer.with_pairing_key(
            &mut vault,
            MonotonicInstant::from_ticks(999_999_999),
            |_| (),
        );
        assert!(
            fresh.is_ok(),
            "fresh lease at a later instant is still valid"
        );
        let stale = composer
            .issue_lease(&mut vault, MonotonicInstant::from_ticks(1_000), 100)
            .expect("lease");
        assert!(
            stale
                .with_secret(MonotonicInstant::from_ticks(9_999), |_| ())
                .is_err()
        );
    }

    #[test]
    fn zero_ttl_and_overflowing_ttl_fail_closed() {
        assert_eq!(
            lease_window(MonotonicInstant::from_ticks(10), 0),
            Err(SecretCompositionError::InvalidTtl)
        );
        assert_eq!(
            lease_window(MonotonicInstant::from_ticks(u64::MAX), 1),
            Err(SecretCompositionError::InvalidTtl)
        );
    }

    #[test]
    fn rotation_advances_exactly_once_and_replaces_the_key() {
        let (mut composer, mut vault) = provisioned();
        let now = MonotonicInstant::from_ticks(5_000);
        let before = composer
            .with_pairing_key(&mut vault, now, |key| *key)
            .expect("key");
        let operation = fresh_operation("rotate", &test_nonce(2)).expect("operation");
        let outcome = composer.rotate(&mut vault, &operation).expect("rotate");
        let receipt = match outcome {
            MutationOutcome::Committed(receipt) => receipt,
            MutationOutcome::Recovered(_) => panic!("clean rotation must commit, not recover"),
        };
        assert_eq!(receipt.reference.version().get(), 2);
        assert_eq!(receipt.record_revision.get(), 2);
        let after = composer
            .with_pairing_key(&mut vault, now, |key| *key)
            .expect("key");
        assert_ne!(before, after);
        // Old key no longer proves against the new binding digest.
        let old_binding = derive_binding_digest(&before);
        let new_binding = composer
            .with_pairing_key(&mut vault, now, derive_binding_digest)
            .expect("key");
        assert_ne!(old_binding, new_binding);
    }

    #[test]
    fn rotation_skipping_a_version_is_rejected() {
        let (mut composer, _vault) = provisioned();
        // Drive the catalog directly past the composer guard: preparing with
        // a version that jumps by two must fail version advancement.
        let id = composer.active_id().expect("active").clone();
        let record = composer.catalog.get(&id).expect("record").clone();
        let operation = fresh_operation("rotate", &test_nonce(3)).expect("operation");
        let jumped = record
            .reference()
            .version()
            .get()
            .checked_add(2)
            .expect("version");
        assert_eq!(
            composer.catalog.prepare_rotation(
                &id,
                &test_binding(),
                NonZeroRevision::new(jumped).expect("revision"),
                EncryptedPayload::new(vec![1; 32], 256).expect("payload"),
                Blake3Digest32::from_bytes([7; 32]),
                NonZeroRevision::new(2).expect("revision"),
                operation,
            ),
            Err(SecretError::VersionMismatch)
        );
    }

    #[test]
    fn ambiguous_rotation_write_recovers_by_exact_readback() {
        let (mut composer, mut vault) = provisioned();
        vault.fail_next_store_ambiguously = true;
        let operation = fresh_operation("rotate", &test_nonce(4)).expect("operation");
        let outcome = composer.rotate(&mut vault, &operation).expect("recover");
        match outcome {
            MutationOutcome::Recovered(receipt) => {
                assert_eq!(receipt.reference.version().get(), 2);
            }
            MutationOutcome::Committed(_) => {
                panic!("ambiguous write must recover, never report a clean commit")
            }
        }
        assert!(composer.has_active_leaseable());
    }

    #[test]
    fn revoke_proves_absence_and_denies_further_leases() {
        let (mut composer, mut vault) = provisioned();
        let operation = fresh_operation("revoke", &test_nonce(5)).expect("operation");
        match composer.revoke(&mut vault, &operation).expect("revoke") {
            MutationOutcome::Committed(_) => {}
            MutationOutcome::Recovered(_) => panic!("clean revoke must commit"),
        }
        assert!(composer.absence_verified(&mut vault).expect("absence"));
        assert!(matches!(
            composer.issue_lease(&mut vault, MonotonicInstant::from_ticks(1_000), 60_000),
            Err(SecretCompositionError::NotFound)
        ));
        // Re-provisioning after revocation starts a fresh lifecycle.
        let operation = fresh_operation("provision", &test_nonce(6)).expect("operation");
        assert!(composer.provision(&mut vault, operation).is_ok());
    }

    #[test]
    fn ambiguous_delete_recovers_and_reports_recovered_not_committed() {
        let (mut composer, mut vault) = provisioned();
        vault.fail_next_remove_ambiguously = true;
        let operation = fresh_operation("revoke", &test_nonce(7)).expect("operation");
        match composer.revoke(&mut vault, &operation).expect("revoke") {
            MutationOutcome::Recovered(_) => {}
            MutationOutcome::Committed(_) => {
                panic!("ambiguous delete must recover, never report a clean commit")
            }
        }
        assert!(composer.absence_verified(&mut vault).expect("absence"));
    }

    #[test]
    fn vault_loss_is_detected_not_relabelled_deleted() {
        let (composer, mut vault) = provisioned();
        let id = composer.active_id().expect("active").clone();
        vault.drop_blob_for_test(&id);
        assert!(matches!(
            composer.issue_lease(&mut vault, MonotonicInstant::from_ticks(1_000), 60_000),
            Err(SecretCompositionError::EvidenceMissing)
        ));
        assert!(!composer.absence_verified(&mut vault).expect("check"));
    }

    #[test]
    fn operation_reuse_with_other_bytes_is_rejected() {
        let mut composer = composer();
        let mut vault = MemoryPairingVault::new();
        let operation = fresh_operation("provision", &test_nonce(8)).expect("operation");
        composer
            .provision(&mut vault, operation.clone())
            .expect("first");
        // Same operation identity replayed for a rotation must conflict at
        // the catalog fence, not silently succeed.
        let id = composer.active_id().expect("active").clone();
        assert_eq!(
            composer.catalog.prepare_rotation(
                &id,
                &test_binding(),
                NonZeroRevision::new(2).expect("revision"),
                EncryptedPayload::new(vec![2; 32], 256).expect("payload"),
                Blake3Digest32::from_bytes([8; 32]),
                NonZeroRevision::new(2).expect("revision"),
                SecretOperation::new(
                    operation.mutation().clone(),
                    Blake3Digest32::from_bytes([0xFF; 32]),
                ),
            ),
            Err(SecretError::OperationConflict)
        );
    }

    #[test]
    fn quarantine_stops_leases_immediately() {
        let (mut composer, mut vault) = provisioned();
        composer
            .quarantine_active(SecretError::Quarantined)
            .expect("quarantine");
        assert!(matches!(
            composer.issue_lease(&mut vault, MonotonicInstant::from_ticks(1_000), 60_000),
            Err(SecretCompositionError::NotLeaseable)
        ));
    }

    #[test]
    fn keyed_proofs_bind_version_session_nonce_and_challenge() {
        let (composer, mut vault) = provisioned();
        let now = MonotonicInstant::from_ticks(7_000);
        let binding = composer
            .with_pairing_key(&mut vault, now, derive_binding_digest)
            .expect("binding");
        let session = ProtoSessionId::from_bytes([0x11; 16]).expect("session");
        let nonce = ProtoClientNonce::from_bytes([0x22; 16]).expect("nonce");
        let challenge = PairingChallenge::from_bytes([0x33; 32]).expect("challenge");
        let transcript = client_proof_transcript(
            PAIRING_PROTOCOL_VERSION,
            &binding,
            session,
            &nonce,
            &challenge,
        );
        let proof = composer
            .with_pairing_key(&mut vault, now, |key| pairing_keyed_proof(key, &transcript))
            .expect("proof");
        // Full ceremony through the pairing machine: exact proof verifies.
        let mut machine = PairingMachine::new(PAIRING_PROTOCOL_VERSION, binding);
        machine
            .issue_challenge(session, nonce, challenge)
            .expect("challenge");
        machine.verify_client_proof(&proof, &proof).expect("verify");
        // Wrong version binds a different transcript: verification fails.
        let other_version = ProtocolVersion { major: 1, minor: 1 };
        let other_transcript =
            client_proof_transcript(other_version, &binding, session, &nonce, &challenge);
        let other_proof = composer
            .with_pairing_key(&mut vault, now, |key| {
                pairing_keyed_proof(key, &other_transcript)
            })
            .expect("proof");
        assert_ne!(proof, other_proof);
        let mut machine = PairingMachine::new(PAIRING_PROTOCOL_VERSION, binding);
        machine
            .issue_challenge(session, nonce, challenge)
            .expect("challenge");
        // The server binds version 1.0: a 1.1 proof mismatches and fails the
        // ceremony terminally, even though both digests are well-formed.
        assert!(machine.verify_client_proof(&proof, &other_proof).is_err());
        assert!(!machine.is_mutually_verified());
        // Provider proof binds the server domain, never the client one.
        let server_transcript = server_proof_transcript(
            PAIRING_PROTOCOL_VERSION,
            &binding,
            session,
            &nonce,
            &challenge,
        );
        let server_proof = composer
            .with_pairing_key(&mut vault, now, |key| {
                pairing_keyed_proof(key, &server_transcript)
            })
            .expect("proof");
        assert_ne!(proof, server_proof);
        assert!(!verify_pairing_proof(&proof, &server_proof));
    }

    #[test]
    fn same_key_composes_with_request_envelopes_and_tampering_fails() {
        let (composer, mut vault) = provisioned();
        let now = MonotonicInstant::from_ticks(8_000);
        let envelope = seal_envelope(
            PAIRING_PROTOCOL_VERSION,
            search_provider_protocol::pairing::ServerNonce::from_bytes([0x44; 16]).expect("nonce"),
            RequestId::from_bytes([0x55; 16]),
            ControlCommand::Health,
            ProofDigest::from_bytes([0x66; 32]),
            ProofDigest::from_bytes([0; 32]),
        );
        let transcript = envelope_transcript(&envelope);
        let proof = composer
            .with_pairing_key(&mut vault, now, |key| {
                pairing_keyed_proof_raw(key, &transcript)
            })
            .expect("proof");
        let sealed = seal_envelope(
            envelope.version(),
            *envelope.server_nonce(),
            *envelope.request_id(),
            envelope.command(),
            *envelope.body_digest(),
            proof,
        );
        verify_envelope_proof(&sealed, &proof).expect("envelope proof verifies");
        // Altered command changes the transcript, so the same proof fails.
        let altered = seal_envelope(
            envelope.version(),
            *envelope.server_nonce(),
            *envelope.request_id(),
            ControlCommand::Shutdown,
            *envelope.body_digest(),
            proof,
        );
        assert_ne!(transcript, envelope_transcript(&altered));
        let mut tampered = *proof.as_bytes();
        tampered[0] ^= 1;
        assert!(!verify_pairing_proof(
            &proof,
            &ProofDigest::from_bytes(tampered)
        ));
    }

    #[test]
    fn lease_and_composer_debug_never_dump_key_material() {
        let (composer, mut vault) = provisioned();
        let lease = composer
            .issue_lease(&mut vault, MonotonicInstant::from_ticks(1_000), 60_000)
            .expect("lease");
        let debug = format!("{lease:?}");
        assert!(debug.contains("<redacted>"));
        let key_hex = composer
            .with_pairing_key(&mut vault, MonotonicInstant::from_ticks(1_000), |key| {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let mut out = String::with_capacity(64);
                for byte in key {
                    out.push(char::from(HEX[usize::from(byte >> 4)]));
                    out.push(char::from(HEX[usize::from(byte & 0x0F)]));
                }
                out
            })
            .expect("key");
        assert!(!debug.contains(&key_hex));
        assert!(!format!("{composer:?}").contains(&key_hex));
    }

    #[test]
    fn reference_and_operation_identities_are_stable_and_bounded() {
        let operation = fresh_operation("provision", &test_nonce(11)).expect("operation");
        let id = pairing_reference_id(&operation).expect("reference");
        assert!(id.as_str().starts_with("secret:loopback-pairing:"));
        assert!(id.as_str().len() <= 256);
        assert_eq!(id, pairing_reference_id(&operation).expect("reference"));
        assert!(fresh_operation("", &test_nonce(1)).is_err());
        assert!(fresh_operation("has space", &test_nonce(1)).is_err());
    }
}
