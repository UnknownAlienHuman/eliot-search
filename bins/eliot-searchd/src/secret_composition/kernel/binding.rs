//! Purpose binding, operation identities, finite lease windows and keyed proofs.

use search_contracts::{
    Blake3Digest32, InstallationId, InstallationIncarnationId, OpaqueId,
};
use search_os_secrets::{SecretBinding, SecretOperation};
use search_ports::{IdempotencyClass, MonotonicInstant, MutationIdentity};
use search_provider_protocol::pairing::{PairingTranscript, ProofDigest, verify_proof};

use super::spec::{
    BINDING_DOMAIN, MAX_VAULT_BLOB_BYTES, OPERATION_DIGEST_DOMAIN,
    PAIRING_KEY_BYTES, PAIRING_ROLE, PAIRING_SECRET_PURPOSE,
    SecretCompositionError,
};
use super::vault::VaultWriteEvidence;

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
/// under a fixed domain.
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
#[must_use]
pub fn pairing_keyed_proof(key: &[u8; 32], transcript: &PairingTranscript) -> ProofDigest {
    pairing_keyed_proof_raw(key, transcript.as_bytes())
}

/// Computes the keyed proof over exact envelope bytes.
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
#[must_use]
pub fn monotonic_now() -> MonotonicInstant {
    static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let epoch = EPOCH.get_or_init(std::time::Instant::now);
    let millis = epoch.elapsed().as_millis();
    MonotonicInstant::from_ticks(u64::try_from(millis).unwrap_or(u64::MAX))
}

pub(super) fn check_key_bytes(key: &[u8]) -> Result<(), SecretCompositionError> {
    if key.len() != PAIRING_KEY_BYTES || key.iter().all(|byte| *byte == 0) {
        return Err(SecretCompositionError::InvalidKeyMaterial);
    }
    Ok(())
}

pub(super) fn verify_blob_readback(
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
