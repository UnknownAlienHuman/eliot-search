use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

use search_contracts::protocol::{HelloBody, PeerRole};
use search_contracts::evidence::RecipeIdV2;
use search_contracts::evidence::{
    EvidenceHints, EvidenceIntent, OrientationDimension, OrientationHints,
    OrientScopeRequestV2, PublicScopeSelector, RetrieveEvidenceRequestV2,
};
use search_contracts::query::{PriorityClass, QueryExecutionBudget};
use search_contracts::{
    AccessPartitionId, BindingId, BoundedSet, CorpusId, CorpusOrPortfolioId,
    DisclosureCeiling, GrantId, InstallationId, InstallationIncarnationId,
    Modality, OpaqueId, OpaqueRef, PortfolioRevision, ProfileId, ProtocolRange,
    ProtocolVersion, RecipeIdV1, ReferencePortfolioId, ScopeDomainId,
    SensitivityClass, SourceMembershipId, UtcTimestamp,
};
use search_provider_protocol::{
    authenticate_binding, BindingContext, BoundSession, ClientEnvelopeCodec, ClientNonce,
    EvidenceRecipeRequestV2, EvidenceRequestEnvelopeV2, MonotonicMillis, PairingChallenge,
    PairingMachine, ProofDigest, SearchReadGrantClaimsV2, ServerNonce, SessionId,
    StandaloneGrantRequestV2, TerminalKind, TransportPeer, CancelOutcome, DEFAULT_PROTOCOL_LIMITS,
};

use super::*;
use crate::access_composition::{
    AuthoritativeGrantPolicy, BoundedStandaloneGrantIssuer, EvidenceRequestAdmissionError,
    GrantEntropySource, GrantIssuerError, GrantMintError, GrantTimeSource, GrantTimeWindow,
    GrantUseError, GrantValidationClock, StandaloneGrantIssuer, StandaloneGrantMaterial,
    StandaloneGrantRequest, StandaloneGrantTemplate,
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
        permitted_evidence_recipes: BoundedSet::empty(),
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
        requested_evidence_recipes: BoundedSet::empty(),
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

#[derive(Debug, Default)]
struct EvidenceEntropy(u8);

impl GrantEntropySource for EvidenceEntropy {
    fn fill_random(&mut self, output: &mut [u8]) -> Result<(), GrantIssuerError> {
        self.0 = self.0.wrapping_add(1).max(1);
        output.fill(self.0);
        Ok(())
    }
}

#[derive(Debug, Default)]
struct EvidenceClock;

impl GrantTimeSource for EvidenceClock {
    fn issue_window(&mut self, requested_ttl_ms: u64) -> Result<GrantTimeWindow, GrantIssuerError> {
        GrantTimeWindow::new(
            timestamp("2026-10-17T00:00:00.000000Z"),
            timestamp("2026-10-17T00:00:30.000000Z"),
            requested_ttl_ms,
        )?
        .with_monotonic_origin(crate::provider_composition::monotonic_millis())
    }
}

impl GrantValidationClock for EvidenceClock {
    fn check_grant_window(
        &mut self,
        _issued_at: &UtcTimestamp,
        _expires_at: &UtcTimestamp,
        monotonic_expiry: MonotonicMillis,
    ) -> Result<MonotonicMillis, GrantUseError> {
        Ok(monotonic_expiry)
    }
}

fn evidence_recipe(recipe: RecipeIdV2) -> EvidenceRecipeRequestV2 {
    let scope = PublicScopeSelector::WorkspaceRevision(
        search_contracts::WorkspaceViewRevisionId::from_bytes([0x71; 16]),
    );
    let source_view = search_contracts::SourceView::RetainedRevision(
        search_contracts::SourceRevisionId::from_bytes([0x72; 16]),
    );
    let question = search_contracts::BoundedText::new("trace the entry point")
        .expect("question");
    let profile_id = profile("interactive");
    let budgets = QueryExecutionBudget {
        priority_class: PriorityClass::Interactive,
        deadline_ms: 30_000,
        max_scoring_legs: 1,
        max_prefetch_candidates_per_leg: 1,
        max_validated_candidates: 1,
        max_source_read_bytes: 1,
        max_exact_scan_items: 1,
        max_exact_scan_bytes: 1,
        max_materialized_result_bytes: 1,
        max_cpu_ms: 1,
        max_memory_bytes: 1,
    };
    match recipe {
        RecipeIdV2::RetrieveEvidence => {
            EvidenceRecipeRequestV2::RetrieveEvidence(RetrieveEvidenceRequestV2 {
                scope,
                source_view,
                question,
                intent: EvidenceIntent::LocateImplementation,
                requested_roles: BoundedSet::empty(),
                optional_hints: EvidenceHints::default(),
                profile_id,
                budgets,
                progressive: true,
            })
        }
        RecipeIdV2::OrientScope => {
            EvidenceRecipeRequestV2::OrientScope(OrientScopeRequestV2 {
                scope,
                source_view,
                question,
                dimensions: set([OrientationDimension::EntryPoints]),
                optional_hints: OrientationHints::default(),
                profile_id,
                budgets,
                progressive: true,
            })
        }
    }
}

fn evidence_envelope(
    request_id: RequestId,
    connection_sequence: u64,
    grant: SearchReadGrantClaimsV2,
    recipe: RecipeIdV2,
) -> EvidenceRequestEnvelopeV2 {
    EvidenceRequestEnvelopeV2 {
        protocol_major: 1,
        protocol_minor: 0,
        installation_incarnation_id: InstallationIncarnationId::from_bytes([3; 16]),
        binding_id: BindingId::from_bytes([1; 16]),
        connection_sequence,
        request_id,
        relative_deadline_ms: Some(30_000),
        grant,
        recipe_request: evidence_recipe(recipe),
    }
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

fn fixture_authority(
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
    let mut authority = fixture_authority(source, Rc::clone(&issuer_calls));

    let grant = authority
        .mint(&session, &request())
        .expect("session-bound grant");

    assert_eq!(grant.binding_id, policy.binding_id);
    assert_eq!(grant.installation_incarnation_id, policy.installation_incarnation_id);
    assert_eq!(source_calls.get(), 2);
    assert_eq!(issuer_calls.get(), 1);
}

#[test]
fn authenticated_session_issues_only_the_policy_approved_v2_recipe() {
    let session = bound_session(PeerRole::StandaloneCli);
    let mut policy = policy();
    policy.permitted_evidence_recipes = set([RecipeIdV2::RetrieveEvidence]);
    let source_calls = Rc::new(Cell::new(0));
    let issuer_calls = Rc::new(Cell::new(0));
    let source = SequencedPolicySource::new(
        [policy.clone(), policy.clone()],
        Rc::clone(&source_calls),
    );
    let mut authority = fixture_authority(source, Rc::clone(&issuer_calls));
    let body = StandaloneGrantRequestV2 {
        expected_binding_generation: policy.binding_generation,
        expected_policy_generation: policy.policy_generation,
        requested_membership_ids: set([membership(10)]),
        requested_corpus_or_portfolio_ids: set([CorpusOrPortfolioId::Corpus(
            CorpusId::from_bytes([20; 16]),
        )]),
        requested_access_partitions: set([AccessPartitionId::from_bytes([30; 16])]),
        requested_modalities: set([Modality::Code]),
        requested_evidence_recipes: set([RecipeIdV2::RetrieveEvidence]),
        requested_budget_class: profile("interactive"),
        requested_sensitivity_ceiling: SensitivityClass::Project,
        requested_disclosure_ceiling: DisclosureCeiling::LocalOnly,
        requested_source_read_permission: true,
        requested_exact_scan_permission: false,
        requested_ttl_ms: 30_000,
    };

    let claims = authority
        .mint_protocol_request_v2(&session, RequestId::from_bytes([0x66; 16]), body)
        .expect("V2 grant");
    assert_eq!(claims.claims.binding_id, policy.binding_id);
    assert!(claims.claims.permitted_recipe_families.is_empty());
    assert_eq!(
        claims.permitted_evidence_recipes,
        set([RecipeIdV2::RetrieveEvidence])
    );
    assert_eq!(source_calls.get(), 2);
    assert_eq!(issuer_calls.get(), 1);
}

#[test]
fn evidence_admission_requires_exact_issued_recipe_and_cancels_denied_slot() {
    let mut session = bound_session(PeerRole::StandaloneCli);
    let mut current_policy = policy();
    current_policy.permitted_evidence_recipes = set([
        RecipeIdV2::RetrieveEvidence,
        RecipeIdV2::OrientScope,
    ]);
    let policy_calls = Rc::new(Cell::new(0));
    let policy_source = SequencedPolicySource::new(
        [
            current_policy.clone(),
            current_policy.clone(),
            current_policy.clone(),
            current_policy.clone(),
            current_policy.clone(),
        ],
        Rc::clone(&policy_calls),
    );
    let issuer = BoundedStandaloneGrantIssuer::new(EvidenceEntropy::default(), EvidenceClock, 4, 4)
        .expect("bounded issuer");
    let mut authority = SessionBoundGrantAuthority::new(policy_source, issuer);
    let grant_request = StandaloneGrantRequestV2 {
        expected_binding_generation: current_policy.binding_generation,
        expected_policy_generation: current_policy.policy_generation,
        requested_membership_ids: set([membership(10)]),
        requested_corpus_or_portfolio_ids: set([CorpusOrPortfolioId::Corpus(
            CorpusId::from_bytes([20; 16]),
        )]),
        requested_access_partitions: set([AccessPartitionId::from_bytes([30; 16])]),
        requested_modalities: set([Modality::Code]),
        requested_evidence_recipes: set([RecipeIdV2::RetrieveEvidence, RecipeIdV2::OrientScope]),
        requested_budget_class: profile("interactive"),
        requested_sensitivity_ceiling: SensitivityClass::Project,
        requested_disclosure_ceiling: DisclosureCeiling::LocalOnly,
        requested_source_read_permission: true,
        requested_exact_scan_permission: false,
        requested_ttl_ms: 30_000,
    };
    let claims = authority
        .mint_protocol_request_v2(&session, RequestId::from_bytes([0x70; 16]), grant_request)
        .expect("server-issued V2 claims");

    // The recipe is also allowed by current policy, but this altered permission
    // was never issued for this grant ID and must release its admitted slot.
    let mut altered = claims.clone();
    altered.permitted_evidence_recipes = set([RecipeIdV2::OrientScope]);
    let denied_id = RequestId::from_bytes([0x71; 16]);
    let versions = ProtocolRange::new(
        ProtocolVersion { major: 1, minor: 0 },
        ProtocolVersion { major: 1, minor: 0 },
    )
    .expect("protocol range");
    let denied_frame = ClientEnvelopeCodec::encode_evidence_request_v2(
        &evidence_envelope(denied_id, 1, altered, RecipeIdV2::OrientScope),
        DEFAULT_PROTOCOL_LIMITS,
        versions,
    )
    .expect("authenticated V2 frame shape");
    let proof = ProofDigest::from_bytes([0x75; 32]);
    let mut clock = || Ok(crate::provider_composition::monotonic_millis());
    assert!(matches!(
        authority.admit_evidence_request_v2(
            &mut session,
            denied_frame.as_slice(),
            &proof,
            30_000,
            &mut clock,
            |transcript| {
                assert_eq!(transcript.frame(), denied_frame.as_slice());
                Ok(proof)
            },
        ),
        Err(EvidenceRequestAdmissionError::Grant(GrantUseError::InvalidGrant))
    ));
    assert_eq!(session.in_flight_len(), 0);
    assert_eq!(
        session.complete_request(&denied_id, TerminalKind::Cancelled),
        Ok(search_provider_protocol::RequestStatus::Cancelled)
    );

    let accepted_id = RequestId::from_bytes([0x72; 16]);
    let accepted_frame = ClientEnvelopeCodec::encode_evidence_request_v2(
        &evidence_envelope(accepted_id, 2, claims.clone(), RecipeIdV2::RetrieveEvidence),
        DEFAULT_PROTOCOL_LIMITS,
        versions,
    )
    .expect("exact issued V2 request");
    let mut accepted_clock = || Ok(crate::provider_composition::monotonic_millis());
    let admitted = authority
        .admit_evidence_request_v2(
            &mut session,
            accepted_frame.as_slice(),
            &proof,
            30_000,
            &mut accepted_clock,
            |_| Ok(proof),
        )
        .expect("exact issued recipe admitted");
    assert_eq!(admitted.guard().request_id(), &accepted_id);
    assert_eq!(policy_calls.get(), 4);
    assert_eq!(session.in_flight_len(), 1);
    assert_eq!(
        session.cancel(&accepted_id),
        CancelOutcome::Cancelled { terminal: false }
    );
    assert_eq!(
        session.complete_request(&accepted_id, TerminalKind::Cancelled),
        Ok(search_provider_protocol::RequestStatus::Cancelled)
    );

    let orientation_id = RequestId::from_bytes([0x73; 16]);
    let orientation_frame = ClientEnvelopeCodec::encode_evidence_request_v2(
        &evidence_envelope(orientation_id, 3, claims, RecipeIdV2::OrientScope),
        DEFAULT_PROTOCOL_LIMITS,
        versions,
    )
    .expect("exact issued orientation request");
    let mut orientation_clock = || Ok(crate::provider_composition::monotonic_millis());
    let orientation = authority
        .admit_evidence_request_v2(
            &mut session,
            orientation_frame.as_slice(),
            &proof,
            30_000,
            &mut orientation_clock,
            |_| Ok(proof),
        )
        .expect("exact issued orientation recipe admitted");
    assert_eq!(orientation.guard().request_id(), &orientation_id);
    assert_eq!(
        orientation.envelope().recipe_request.recipe_id(),
        RecipeIdV2::OrientScope
    );
    assert_eq!(policy_calls.get(), 5);
    assert_eq!(session.in_flight_len(), 1);
    assert_eq!(
        session.cancel(&orientation_id),
        CancelOutcome::Cancelled { terminal: false }
    );
    assert_eq!(
        session.complete_request(&orientation_id, TerminalKind::Cancelled),
        Ok(search_provider_protocol::RequestStatus::Cancelled)
    );
}

#[test]
fn inactive_or_non_standalone_session_never_reads_policy_or_calls_issuer() {
    let source_calls = Rc::new(Cell::new(0));
    let issuer_calls = Rc::new(Cell::new(0));
    let source = SequencedPolicySource::new(
        [policy(), policy()],
        Rc::clone(&source_calls),
    );
    let mut authority = fixture_authority(source, Rc::clone(&issuer_calls));
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
    let mut authority = fixture_authority(source, Rc::clone(&issuer_calls));
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
        let mut authority = fixture_authority(source, Rc::clone(&issuer_calls));
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
    let mut authority = fixture_authority(source, Rc::clone(&issuer_calls));
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
    let mut authority = fixture_authority(source, Rc::clone(&issuer_calls));
    assert_eq!(
        authority.mint(&session, &request()),
        Err(GrantAuthorityError::PolicyChangedDuringIssuance)
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
    let mut authority = fixture_authority(source, Rc::clone(&issuer_calls));
    let mut request = request();
    request.binding_id = BindingId::from_bytes([88; 16]);

    assert_eq!(
        authority.mint(&session, &request),
        Err(GrantAuthorityError::Mint(GrantMintError::BindingMismatch))
    );
    assert_eq!(source_calls.get(), 1);
    assert_eq!(issuer_calls.get(), 0);
}
