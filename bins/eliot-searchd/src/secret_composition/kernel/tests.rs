use super::*;
use super::spec::BINDING_DOMAIN;

use search_contracts::{
    Blake3Digest32, InstallationId, InstallationIncarnationId,
    NonZeroRevision, ProtocolVersion, RequestId,
};
use search_os_secrets::{
    EncryptedPayload, SecretBinding, SecretError, SecretOperation,
};
use search_ports::MonotonicInstant;
use search_provider_protocol::pairing::{
    ClientNonce as ProtoClientNonce, PairingChallenge, PairingMachine,
    ProofDigest, SessionId as ProtoSessionId, client_proof_transcript,
    server_proof_transcript,
};
use search_provider_protocol::request::{
    ControlCommand, envelope_transcript, seal_envelope,
    verify_envelope_proof,
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
    let fresh = composer.with_pairing_key(
        &mut vault,
        MonotonicInstant::from_ticks(999_999_999),
        |_| (),
    );
    assert!(fresh.is_ok());
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
        MutationOutcome::Recovered(_) => panic!("clean rotation must commit"),
    };
    assert_eq!(receipt.reference.version().get(), 2);
    assert_eq!(receipt.record_revision.get(), 2);
    let after = composer
        .with_pairing_key(&mut vault, now, |key| *key)
        .expect("key");
    assert_ne!(before, after);
    let old_binding = derive_binding_digest(&before);
    let new_binding = composer
        .with_pairing_key(&mut vault, now, derive_binding_digest)
        .expect("key");
    assert_ne!(old_binding, new_binding);
}

#[test]
fn rotation_skipping_a_version_is_rejected() {
    let (mut composer, _vault) = provisioned();
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
        MutationOutcome::Committed(_) => panic!("ambiguous write must recover"),
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
        MutationOutcome::Committed(_) => panic!("ambiguous delete must recover"),
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
    let mut machine = PairingMachine::new(PAIRING_PROTOCOL_VERSION, binding);
    machine
        .issue_challenge(session, nonce, challenge)
        .expect("challenge");
    machine.verify_client_proof(&proof, &proof).expect("verify");
    let other_version = ProtocolVersion { major: 1, minor: 1 };
    let other_transcript =
        client_proof_transcript(other_version, &binding, session, &nonce, &challenge);
    let other_proof = composer
        .with_pairing_key(&mut vault, now, |key| pairing_keyed_proof(key, &other_transcript))
        .expect("proof");
    assert_ne!(proof, other_proof);
    let mut machine = PairingMachine::new(PAIRING_PROTOCOL_VERSION, binding);
    machine
        .issue_challenge(session, nonce, challenge)
        .expect("challenge");
    assert!(machine.verify_client_proof(&proof, &other_proof).is_err());
    assert!(!machine.is_mutually_verified());
    let server_transcript = server_proof_transcript(
        PAIRING_PROTOCOL_VERSION,
        &binding,
        session,
        &nonce,
        &challenge,
    );
    let server_proof = composer
        .with_pairing_key(&mut vault, now, |key| pairing_keyed_proof(key, &server_transcript))
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
        .with_pairing_key(&mut vault, now, |key| pairing_keyed_proof_raw(key, &transcript))
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
