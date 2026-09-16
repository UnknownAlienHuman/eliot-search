use std::collections::VecDeque;

use search_contracts::protocol::{HelloBody, PeerRole};
use search_contracts::{
    AccessPartitionId, BindingId, BoundedSet, CorpusId, CorpusOrPortfolioId,
    DisclosureCeiling, GrantId, InstallationId, InstallationIncarnationId,
    Modality, OpaqueId, OpaqueRef, PortfolioRevision, ProfileId, ProtocolRange,
    ProtocolVersion, RecipeIdV1, ReferencePortfolioId, RequestId, ScopeDomainId,
    SensitivityClass, SourceMembershipId, UtcTimestamp, MAX_SET_ITEMS,
};
use search_provider_protocol::{
    authenticate_binding, encode_standalone_grant_request, seal_standalone_grant_envelope,
    BindingContext, BoundSession, ClientNonce, MonotonicMillis, PairingChallenge,
    PairingMachine, ProofDigest, ProtocolError, RequestStatus, ServerNonce, SessionId,
    StandaloneGrantRequestV1, TerminalKind, TransportPeer, DEFAULT_PROTOCOL_LIMITS,
};

use super::*;
use crate::access_composition::{
    AuthoritativeGrantPolicy, GrantIssuerError, StandaloneGrantMaterial,
    StandaloneGrantTemplate,
};

fn timestamp(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("timestamp")
}

fn profile(value: &str) -> ProfileId {
    ProfileId::new(value).expect("profile")
}

fn membership(byte: u8) -> SourceMembershipId {
    SourceMembershipId::from_bytes([byte; 16])
}

fn set<T: Ord, const LIMIT: usize>(
    values: impl IntoIterator<Item = T>,
) -> BoundedSet<T, LIMIT> {
    BoundedSet::from_items(values).expect("bounded set")
}

fn policy() -> AuthoritativeGrantPolicy {
    AuthoritativeGrantPolicy {
        binding_id: BindingId::from_bytes([1; 16]),
        binding_generation: 4,
        policy_generation: 7,
        installation_id: InstallationId::from_bytes([2; 16]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([3; 16]),
        principal_opaque_id: OpaqueId::new("principal").expect("principal"),
        client_scope_ref: OpaqueRef::new("client-scope").expect("scope"),
        scope_domain_id: ScopeDomainId::from_bytes([4; 16]),
        allowed_membership_ids: set([membership(10), membership(11)]),
        allowed_corpus_or_portfolio_ids: set([
            CorpusOrPortfolioId::Corpus(CorpusId::from_bytes([20; 16])),
            CorpusOrPortfolioId::Portfolio(ReferencePortfolioId::from_bytes([21; 16])),
        ]),
        reference_portfolio_revision: Some(PortfolioRevision::new(8)),
        allowed_access_partitions: set([
            AccessPartitionId::from_bytes([30; 16]),
            AccessPartitionId::from_bytes([31; 16]),
        ]),
        allowed_modalities: set([Modality::Code, Modality::Text]),
        permitted_recipe_families: set([RecipeIdV1::Locate, RecipeIdV1::FindText]),
        allowed_budget_classes: set([profile("interactive"), profile("verification")]),
        sensitivity_ceiling: SensitivityClass::Confidential,
        disclosure_ceiling: DisclosureCeiling::NamedClient,
        source_read_permission: true,
        exact_scan_permission: true,
        issued_boot_id: OpaqueId::new("boot").expect("boot"),
        revocation_generation: 9,
        maximum_ttl_ms: 60_000,
    }
}

fn request_body() -> StandaloneGrantRequestV1 {
    StandaloneGrantRequestV1 {
        expected_binding_generation: 4,
        expected_policy_generation: 7,
        requested_membership_ids: set([membership(10)]),
        requested_corpus_or_portfolio_ids: set([CorpusOrPortfolioId::Corpus(
            CorpusId::from_bytes([20; 16]),
        )]),
        requested_access_partitions: set([AccessPartitionId::from_bytes([30; 16])]),
        requested_modalities: set([Modality::Code]),
        requested_recipe_families: set([RecipeIdV1::Locate]),
        requested_budget_class: profile("interactive"),
        requested_sensitivity_ceiling: SensitivityClass::Project,
        requested_disclosure_ceiling: DisclosureCeiling::LocalOnly,
        requested_source_read_permission: true,
        requested_exact_scan_permission: false,
        requested_ttl_ms: 30_000,
    }
}

fn bound_session() -> BoundSession {
    let version = ProtocolVersion { major: 1, minor: 0 };
    let binding_proof = ProofDigest::from_bytes([9; 32]);
    let mut pairing = PairingMachine::new(version, binding_proof);
    pairing
        .issue_challenge(
            SessionId::from_bytes([1; 16]).expect("session"),
            ClientNonce::from_bytes([2; 16]).expect("client nonce"),
            PairingChallenge::from_bytes([3; 32]).expect("challenge"),
        )
        .expect("issue challenge");
    pairing
        .verify_client_proof(&binding_proof, &binding_proof)
        .expect("verify client proof");
    pairing
        .issue_provider_proof(ProofDigest::from_bytes([4; 32]))
        .expect("issue provider proof");
    let pairing = pairing.into_verified().expect("verified pairing");
    let incarnation = InstallationIncarnationId::from_bytes([3; 16]);
    let hello = HelloBody {
        peer_role: PeerRole::StandaloneCli,
        pairing_proof_ref: OpaqueRef::new("proof-ref").expect("proof ref"),
        supported_protocol_range: ProtocolRange::new(version, version).expect("range"),
        requested_capability_digest: None,
    };
    let peer = TransportPeer {
        role: PeerRole::StandaloneCli,
        incarnation,
        binding: BindingId::from_bytes([1; 16]),
    };
    let context = authenticate_binding(&hello, &pairing, &incarnation, &peer)
        .expect("authenticated binding");
    BoundSession::open(
        context,
        pairing,
        ServerNonce::from_bytes([0x44; 16]).expect("server nonce"),
        DEFAULT_PROTOCOL_LIMITS,
    )
    .expect("bound session")
}

fn envelope(
    request_id: RequestId,
    body_bytes: &[u8],
    proof: ProofDigest,
) -> search_provider_protocol::AuthenticatedStandaloneGrantEnvelope {
    seal_standalone_grant_envelope(
        ProtocolVersion { major: 1, minor: 0 },
        ServerNonce::from_bytes([0x44; 16]).expect("nonce"),
        request_id,
        ProofDigest::from_bytes(*blake3::hash(body_bytes).as_bytes()),
        proof,
    )
}

fn input<'a>(
    envelope: &'a search_provider_protocol::AuthenticatedStandaloneGrantEnvelope,
    proof: &'a ProofDigest,
    body_bytes: &'a [u8],
) -> StandaloneGrantCommandInput<'a> {
    StandaloneGrantCommandInput {
        envelope,
        expected_proof: proof,
        body_bytes,
        sequence: 1,
        now: MonotonicMillis::new(10),
        relative_deadline_ms: Some(5_000),
    }
}

struct PolicySource {
    snapshots: VecDeque<AuthoritativeGrantPolicy>,
}

impl PolicySource {
    fn new(snapshots: impl IntoIterator<Item = AuthoritativeGrantPolicy>) -> Self {
        Self {
            snapshots: snapshots.into_iter().collect(),
        }
    }
}

impl StandaloneGrantPolicySource for PolicySource {
    type Error = ();

    fn snapshot(
        &mut self,
        _binding: &BindingContext,
    ) -> Result<AuthoritativeGrantPolicy, Self::Error> {
        self.snapshots.pop_front().ok_or(())
    }
}

struct Issuer;

impl StandaloneGrantIssuer for Issuer {
    fn issue(
        &mut self,
        template: &StandaloneGrantTemplate,
    ) -> Result<StandaloneGrantMaterial, GrantIssuerError> {
        Ok(StandaloneGrantMaterial {
            operation_id: template.operation_id.clone(),
            binding_id: template.binding_id,
            binding_generation: template.binding_generation,
            policy_generation: template.policy_generation,
            grant_id: GrantId::from_bytes([40; 16]),
            nonce: OpaqueId::new("nonce").expect("nonce"),
            issued_at: timestamp("2026-09-16T04:00:00.000000Z"),
            expires_at: timestamp("2026-09-16T04:00:30.000000Z"),
            effective_ttl_ms: template.requested_ttl_ms,
        })
    }
}

fn authority(
    snapshots: impl IntoIterator<Item = AuthoritativeGrantPolicy>,
) -> SessionBoundGrantAuthority<PolicySource, Issuer> {
    SessionBoundGrantAuthority::new(PolicySource::new(snapshots), Issuer)
}

#[test]
fn exact_authenticated_body_mints_and_releases_one_terminal() {
    let policy = policy();
    let bytes = encode_standalone_grant_request(&request_body()).expect("body");
    let proof = ProofDigest::from_bytes([0x77; 32]);
    let request_id = RequestId::from_bytes([0x55; 16]);
    let envelope = envelope(request_id, &bytes, proof);
    let mut session = bound_session();
    let mut authority = authority([policy.clone(), policy.clone()]);

    let claims = execute_standalone_grant_command(
        &mut session,
        &mut authority,
        input(&envelope, &proof, &bytes),
    )
    .expect("grant command");

    assert_eq!(claims.binding_id, policy.binding_id);
    assert_eq!(session.in_flight_len(), 0);
    assert_eq!(
        session.complete_request(&request_id, TerminalKind::Success),
        Err(ProtocolError::DuplicateTerminal)
    );
}

#[test]
fn body_mismatch_does_not_consume_request_identity_or_sequence() {
    let policy = policy();
    let bytes = encode_standalone_grant_request(&request_body()).expect("body");
    let proof = ProofDigest::from_bytes([0x78; 32]);
    let request_id = RequestId::from_bytes([0x56; 16]);
    let mut current_envelope = envelope(request_id, b"different", proof);
    let mut session = bound_session();
    let mut first_authority = authority([policy.clone(), policy.clone()]);

    let failure = execute_standalone_grant_command(
        &mut session,
        &mut first_authority,
        input(&current_envelope, &proof, &bytes),
    )
    .expect_err("body mismatch");
    assert_eq!(
        failure.error(),
        GrantCommandError::Protocol(ProtocolError::InvalidBody)
    );
    assert_eq!(failure.status(), None);
    assert_eq!(session.in_flight_len(), 0);

    current_envelope = envelope(request_id, &bytes, proof);
    let mut second_authority = authority([policy.clone(), policy]);
    execute_standalone_grant_command(
        &mut session,
        &mut second_authority,
        input(&current_envelope, &proof, &bytes),
    )
    .expect("same identity and sequence remain valid");
}

#[test]
fn malformed_authenticated_body_records_failed_terminal_and_releases() {
    let body = b"{}";
    let proof = ProofDigest::from_bytes([0x79; 32]);
    let request_id = RequestId::from_bytes([0x57; 16]);
    let envelope = envelope(request_id, body, proof);
    let mut session = bound_session();
    let mut authority = authority([policy(), policy()]);

    let failure = execute_standalone_grant_command(
        &mut session,
        &mut authority,
        input(&envelope, &proof, body),
    )
    .expect_err("malformed body");
    assert_eq!(
        failure.error(),
        GrantCommandError::Protocol(ProtocolError::InvalidBody)
    );
    assert_eq!(failure.status(), Some(RequestStatus::Failed));
    assert_eq!(session.in_flight_len(), 0);
    assert_eq!(
        session.complete_request(&request_id, TerminalKind::Failed),
        Err(ProtocolError::DuplicateTerminal)
    );
}

#[test]
fn policy_change_after_issuer_is_outcome_unknown_and_releases() {
    let before = policy();
    let mut after = before.clone();
    after.policy_generation += 1;
    let bytes = encode_standalone_grant_request(&request_body()).expect("body");
    let proof = ProofDigest::from_bytes([0x7a; 32]);
    let request_id = RequestId::from_bytes([0x58; 16]);
    let envelope = envelope(request_id, &bytes, proof);
    let mut session = bound_session();
    let mut authority = authority([before, after]);

    let failure = execute_standalone_grant_command(
        &mut session,
        &mut authority,
        input(&envelope, &proof, &bytes),
    )
    .expect_err("policy race");
    assert_eq!(
        failure.error(),
        GrantCommandError::Authority(GrantAuthorityError::PolicyChangedDuringIssuance)
    );
    assert_eq!(failure.status(), Some(RequestStatus::OutcomeUnknown));
    assert_eq!(session.in_flight_len(), 0);
}
