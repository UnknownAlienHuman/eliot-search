use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

use search_contracts::protocol::{HelloBody, PeerRole};
use search_contracts::{
    AccessPartitionId, BindingId, BoundedSet, CorpusId, CorpusOrPortfolioId,
    DisclosureCeiling, GrantId, InstallationId, InstallationIncarnationId,
    Modality, OpaqueId, OpaqueRef, PortfolioRevision, ProfileId, ProtocolRange,
    ProtocolVersion, RecipeIdV1, ReferencePortfolioId, ScopeDomainId,
    SensitivityClass, SourceMembershipId, UtcTimestamp, MAX_SET_ITEMS,
};
use search_provider_protocol::{
    authenticate_binding, BindingContext, BoundSession, ClientNonce, PairingChallenge,
    PairingMachine, ProofDigest, ServerNonce, SessionId, TransportPeer,
    DEFAULT_PROTOCOL_LIMITS,
};

use super::*;
use crate::access_composition::{
    AuthoritativeGrantPolicy, GrantIssuerError, GrantMintError,
    StandaloneGrantIssuer, StandaloneGrantMaterial, StandaloneGrantRequest,
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

fn request() -> StandaloneGrantRequest {
    StandaloneGrantRequest {
        operation_id: OpaqueId::new("operation-1").expect("operation"),
        binding_id: BindingId::from_bytes([1; 16]),
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

fn bound_session(role: PeerRole) -> BoundSession {
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
        peer_role: role,
        pairing_proof_ref: OpaqueRef::new("proof-ref").expect("proof ref"),
        supported_protocol_range: ProtocolRange::new(version, version).expect("range"),
        requested_capability_digest: None,
    };
    let peer = TransportPeer {
        role,
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

struct SequencedPolicySource {
    snapshots: VecDeque<AuthoritativeGrantPolicy>,
    fail_on_call: Option<usize>,
    calls: Rc<Cell<usize>>,
}

impl SequencedPolicySource {
    fn new(
        snapshots: impl IntoIterator<Item = AuthoritativeGrantPolicy>,
        calls: Rc<Cell<usize>>,
    ) -> Self {
        Self {
            snapshots: snapshots.into_iter().collect(),
            fail_on_call: None,
            calls,
        }
    }
}

impl StandaloneGrantPolicySource for SequencedPolicySource {
    type Error = ();

    fn snapshot(
        &mut self,
        _binding: &BindingContext,
    ) -> Result<AuthoritativeGrantPolicy, Self::Error> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if self.fail_on_call == Some(call) {
            return Err(());
        }
        self.snapshots.pop_front().ok_or(())
    }
}

struct CountingIssuer {
    calls: Rc<Cell<usize>>,
}

impl StandaloneGrantIssuer for CountingIssuer {
    fn issue(
        &mut self,
        template: &StandaloneGrantTemplate,
    ) -> Result<StandaloneGrantMaterial, GrantIssuerError> {
        self.calls.set(self.calls.get() + 1);
        Ok(StandaloneGrantMaterial {
            operation_id: template.operation_id.clone(),
            binding_id: template.binding_id,
            binding_generation: template.binding_generation,
            policy_generation: template.policy_generation,
            grant_id: GrantId::from_bytes([40; 16]),
            nonce: OpaqueId::new("nonce").expect("nonce"),
            issued_at: timestamp("2026-09-16T01:00:00.000000Z"),
            expires_at: timestamp("2026-09-16T01:00:30.000000Z"),
            effective_ttl_ms: template.requested_ttl_ms,
        })
    }
}

fn authority(
    source: SequencedPolicySource,
    issuer_calls: Rc<Cell<usize>>,
) -> SessionBoundGrantAuthority<SequencedPolicySource, CountingIssuer> {
    SessionBoundGrantAuthority::new(source, CountingIssuer { calls: issuer_calls })
}

#[test]
fn active_authenticated_standalone_session_mints_after_equal_policy_reads() {
    let session = bound_session(PeerRole::StandaloneCli);
    let policy = policy();
    let source_calls = Rc::new(Cell::new(0));
    let issuer_calls = Rc::new(Cell::new(0));
    let source = SequencedPolicySource::new(
        [policy.clone(), policy.clone()],
        Rc::clone(&source_calls),
    );
    let mut authority = authority(source, Rc::clone(&issuer_calls));

    let grant = authority
        .mint(&session, &request())
        .expect("session-bound grant");

    assert_eq!(grant.binding_id, policy.binding_id);
    assert_eq!(grant.installation_incarnation_id, policy.installation_incarnation_id);
    assert_eq!(source_calls.get(), 2);
    assert_eq!(issuer_calls.get(), 1);
}

#[test]
fn inactive_or_non_standalone_session_never_reads_policy_or_calls_issuer() {
    let source_calls = Rc::new(Cell::new(0));
    let issuer_calls = Rc::new(Cell::new(0));
    let source = SequencedPolicySource::new(
        [policy(), policy()],
        Rc::clone(&source_calls),
    );
    let mut authority = authority(source, Rc::clone(&issuer_calls));
    let mut inactive = bound_session(PeerRole::StandaloneCli);
    let _ = inactive.disconnect();
    assert_eq!(
        authority.mint(&inactive, &request()),
        Err(GrantAuthorityError::SessionInactive)
    );
    assert_eq!(source_calls.get(), 0);
    assert_eq!(issuer_calls.get(), 0);

    let source_calls = Rc::new(Cell::new(0));
    let issuer_calls = Rc::new(Cell::new(0));
    let source = SequencedPolicySource::new(
        [policy(), policy()],
        Rc::clone(&source_calls),
    );
    let mut authority = authority(source, Rc::clone(&issuer_calls));
    let managed = bound_session(PeerRole::ClientAdapter);
    assert_eq!(
        authority.mint(&managed, &request()),
        Err(GrantAuthorityError::PeerRoleDenied)
    );
    assert_eq!(source_calls.get(), 0);
    assert_eq!(issuer_calls.get(), 0);
}

#[test]
fn foreign_binding_or_incarnation_policy_fails_before_issuer() {
    for foreign in [
        {
            let mut policy = policy();
            policy.binding_id = BindingId::from_bytes([99; 16]);
            policy
        },
        {
            let mut policy = policy();
            policy.installation_incarnation_id =
                InstallationIncarnationId::from_bytes([99; 16]);
            policy
        },
    ] {
        let source_calls = Rc::new(Cell::new(0));
        let issuer_calls = Rc::new(Cell::new(0));
        let source = SequencedPolicySource::new(
            [foreign.clone(), foreign],
            Rc::clone(&source_calls),
        );
        let mut authority = authority(source, Rc::clone(&issuer_calls));
        assert_eq!(
            authority.mint(&bound_session(PeerRole::StandaloneCli), &request()),
            Err(GrantAuthorityError::PolicyBindingMismatch)
        );
        assert_eq!(source_calls.get(), 1);
        assert_eq!(issuer_calls.get(), 0);
    }
}

#[test]
fn policy_change_or_second_read_failure_discards_issued_claims() {
    let session = bound_session(PeerRole::StandaloneCli);
    let before = policy();
    let mut after = before.clone();
    after.policy_generation += 1;
    let source_calls = Rc::new(Cell::new(0));
    let issuer_calls = Rc::new(Cell::new(0));
    let source = SequencedPolicySource::new(
        [before, after],
        Rc::clone(&source_calls),
    );
    let mut authority = authority(source, Rc::clone(&issuer_calls));
    assert_eq!(
        authority.mint(&session, &request()),
        Err(GrantAuthorityError::PolicyChangedDuringIssuance)
    );
    assert_eq!(source_calls.get(), 2);
    assert_eq!(issuer_calls.get(), 1);

    let source_calls = Rc::new(Cell::new(0));
    let issuer_calls = Rc::new(Cell::new(0));
    let mut source = SequencedPolicySource::new(
        [policy()],
        Rc::clone(&source_calls),
    );
    source.fail_on_call = Some(2);
    let mut authority = authority(source, Rc::clone(&issuer_calls));
    assert_eq!(
        authority.mint(&session, &request()),
        Err(GrantAuthorityError::PolicyUnavailable)
    );
    assert_eq!(source_calls.get(), 2);
    assert_eq!(issuer_calls.get(), 1);
}

#[test]
fn exact_mint_failure_is_preserved_and_skips_second_policy_read() {
    let session = bound_session(PeerRole::StandaloneCli);
    let source_calls = Rc::new(Cell::new(0));
    let issuer_calls = Rc::new(Cell::new(0));
    let source = SequencedPolicySource::new(
        [policy(), policy()],
        Rc::clone(&source_calls),
    );
    let mut authority = authority(source, Rc::clone(&issuer_calls));
    let mut request = request();
    request.binding_id = BindingId::from_bytes([88; 16]);

    assert_eq!(
        authority.mint(&session, &request),
        Err(GrantAuthorityError::Mint(GrantMintError::BindingMismatch))
    );
    assert_eq!(source_calls.get(), 1);
    assert_eq!(issuer_calls.get(), 0);
}
