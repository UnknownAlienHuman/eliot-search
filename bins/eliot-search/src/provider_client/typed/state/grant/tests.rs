//! Synthetic owner fixtures; no native transport or server authorization proof.

#[cfg(test)]
mod fixtures {
    use std::cell::Cell;
    use std::io::{self, Read, Write};
    use std::rc::Rc;
    use std::time::Duration;

    use search_contracts::{
        AccessPartitionId, BindingId, BoundedSet, DisclosureCeiling, GrantId, HelloBody,
        InstallationId, InstallationIncarnationId, MAX_SET_ITEMS, Modality, OpaqueId, OpaqueRef,
        ProfileId, ProtocolRange, ProtocolVersion, RecipeIdV1, RequestId, ScopeDomainId,
        SearchReadGrantClaims, SensitivityClass, SourceMembershipId, UtcTimestamp,
        protocol::PeerRole,
    };
    use search_provider_protocol::{
        BindingKey, ClientNonce, DEFAULT_PROTOCOL_LIMITS, PairingChallenge, PairingMachine,
        ProofDigest, ServerNonce, SessionId, SessionMachine, StandaloneGrantRequestV1,
        TransportPeer, authenticate_binding,
    };

    use super::super::{State, TypedClientError, TypedProviderSession, validate_claims};
    use crate::provider_client::typed::io::{LocalByteStream, SocketIo};

    #[derive(Default)]
    struct Counts {
        reads: Cell<usize>,
        writes: Cell<usize>,
        flushes: Cell<usize>,
        drops: Cell<usize>,
    }

    struct CountingStream(Rc<Counts>);

    impl Read for CountingStream {
        fn read(&mut self, _bytes: &mut [u8]) -> io::Result<usize> {
            self.0.reads.set(self.0.reads.get() + 1);
            Ok(0)
        }
    }

    impl Write for CountingStream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.writes.set(self.0.writes.get() + 1);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.0.flushes.set(self.0.flushes.get() + 1);
            Ok(())
        }
    }

    impl LocalByteStream for CountingStream {
        fn configure_local(&self) -> Result<(), TypedClientError> {
            panic!("grant admission must not configure the already-owned stream")
        }
    }

    impl Drop for CountingStream {
        fn drop(&mut self) {
            self.0.drops.set(self.0.drops.get() + 1);
        }
    }

    fn state(counts: &Rc<Counts>, trusted: Option<InstallationId>) -> State {
        let version = ProtocolVersion { major: 1, minor: 0 };
        let proof = ProofDigest::from_bytes([9; 32]);
        let mut ceremony = PairingMachine::new(version, proof);
        ceremony
            .issue_challenge(
                SessionId::from_bytes([1; 16]).expect("session"),
                ClientNonce::from_bytes([2; 16]).expect("client nonce"),
                PairingChallenge::from_bytes([3; 32]).expect("challenge"),
            )
            .expect("challenge issued");
        ceremony
            .verify_client_proof(&proof, &proof)
            .expect("client verified");
        ceremony
            .issue_provider_proof(proof)
            .expect("provider proof");
        let pairing = ceremony.into_verified().expect("synthetic ceremony");
        let peer = TransportPeer {
            role: PeerRole::StandaloneCli,
            incarnation: InstallationIncarnationId::from_bytes([3; 16]),
            binding: BindingId::from_bytes([4; 16]),
        };
        let hello = HelloBody {
            peer_role: peer.role,
            pairing_proof_ref: OpaqueRef::new("pairing-ref").expect("ref"),
            supported_protocol_range: ProtocolRange::new(version, version).expect("range"),
            requested_capability_digest: None,
        };
        let binding = authenticate_binding(&hello, &pairing, &peer.incarnation, &peer)
            .expect("synthetic binding");
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 1).expect("session");
        session.negotiate(version).expect("negotiated");
        session.activate(&proof, &proof).expect("active");
        let mut state = State::new(
            SocketIo::from_local_stream(CountingStream(Rc::clone(counts))),
            binding,
            pairing,
            BindingKey::from_bytes([0xAB; 32]).expect("synthetic key"),
            ServerNonce::from_bytes([0x44; 16]).expect("server nonce"),
            DEFAULT_PROTOCOL_LIMITS,
            session,
        );
        if let Some(expected) = trusted {
            state
                .bind_trusted_installation_id(expected)
                .expect("trust expectation");
        }
        state
    }

    fn one<T: Ord>(value: T) -> BoundedSet<T, MAX_SET_ITEMS> {
        BoundedSet::from_items([value]).expect("bounded set")
    }

    fn request() -> StandaloneGrantRequestV1 {
        StandaloneGrantRequestV1 {
            expected_binding_generation: 1,
            expected_policy_generation: 1,
            requested_membership_ids: one(SourceMembershipId::from_bytes([7; 16])),
            requested_corpus_or_portfolio_ids: BoundedSet::empty(),
            requested_access_partitions: one(AccessPartitionId::from_bytes([8; 16])),
            requested_modalities: one(Modality::Text),
            requested_recipe_families: one(RecipeIdV1::FindText),
            requested_budget_class: ProfileId::new("interactive").expect("profile"),
            requested_sensitivity_ceiling: SensitivityClass::Project,
            requested_disclosure_ceiling: DisclosureCeiling::LocalOnly,
            requested_source_read_permission: false,
            requested_exact_scan_permission: false,
            requested_ttl_ms: 30_000,
        }
    }

    fn claims(
        request: &StandaloneGrantRequestV1,
        expected: InstallationId,
    ) -> SearchReadGrantClaims {
        SearchReadGrantClaims {
            grant_id: GrantId::from_bytes([1; 16]),
            installation_id: expected,
            installation_incarnation_id: InstallationIncarnationId::from_bytes([3; 16]),
            binding_id: BindingId::from_bytes([4; 16]),
            principal_opaque_id: OpaqueId::new("principal").expect("principal"),
            client_scope_ref: OpaqueRef::new("scope").expect("scope"),
            scope_domain_id: ScopeDomainId::from_bytes([5; 16]),
            allowed_membership_ids: request.requested_membership_ids.clone(),
            allowed_corpus_or_portfolio_ids: request.requested_corpus_or_portfolio_ids.clone(),
            reference_portfolio_revision: None,
            allowed_access_partitions: request.requested_access_partitions.clone(),
            allowed_modalities: request.requested_modalities.clone(),
            permitted_recipe_families: request.requested_recipe_families.clone(),
            maximum_budget_class: request.requested_budget_class.clone(),
            sensitivity_ceiling: request.requested_sensitivity_ceiling,
            disclosure_ceiling: request.requested_disclosure_ceiling,
            source_read_permission: request.requested_source_read_permission,
            exact_scan_permission: request.requested_exact_scan_permission,
            issued_boot_id: OpaqueId::new("boot").expect("boot"),
            issued_at: UtcTimestamp::parse("2026-09-02T10:00:00.000000Z").expect("issued"),
            expires_at: UtcTimestamp::parse("2026-09-02T10:00:01.000000Z").expect("expires"),
            nonce: OpaqueId::new("nonce").expect("nonce"),
            revocation_generation: 0,
        }
    }

    fn no_io(counts: &Counts) {
        assert_eq!(counts.reads.get(), 0);
        assert_eq!(counts.writes.get(), 0);
        assert_eq!(counts.flushes.get(), 0);
    }

    #[test]
    fn absent_trust_precedes_validation_sequence_and_io() {
        let counts = Rc::new(Counts::default());
        let mut state = state(&counts, None);
        let mut invalid = request();
        invalid.expected_binding_generation = 0;
        assert!(invalid.validate().is_err());
        let result = state.request_standalone_grant(
            RequestId::from_bytes([0x21; 16]),
            invalid,
            Duration::ZERO,
        );
        assert!(matches!(
            result,
            Err(TypedClientError::TrustedBindingRequired)
        ));
        assert!(state.grant.is_none());
        assert!(state.requests.is_empty());
        assert!(state.cancel.is_none());
        assert_eq!(state.next_client_sequence().expect("unspent sequence"), 1);
        no_io(&counts);
    }

    #[test]
    fn public_refusal_drops_owner_and_cannot_be_reused() {
        let counts = Rc::new(Counts::default());
        let mut owner = TypedProviderSession {
            state: Some(state(&counts, None)),
        };
        let request = request();
        request.validate().expect("valid request");
        let result = owner.request_standalone_grant(
            RequestId::from_bytes([0x21; 16]),
            request.clone(),
            Duration::from_secs(1),
        );
        assert_eq!(
            result.expect_err("missing trust").code(),
            "REMOTE_TYPED_TRUSTED_BINDING_REQUIRED"
        );
        assert!(owner.state.is_none());
        assert_eq!(counts.drops.get(), 1);
        assert!(matches!(
            owner.request_standalone_grant(
                RequestId::from_bytes([0x22; 16]),
                request,
                Duration::from_secs(1),
            ),
            Err(TypedClientError::Closed)
        ));
        no_io(&counts);
    }

    #[test]
    fn exact_expected_claims_pass_but_each_identity_substitution_fails() {
        let counts = Rc::new(Counts::default());
        let expected = InstallationId::from_bytes([2; 16]);
        let state = state(&counts, Some(expected));
        let request = request();
        let accepted = claims(&request, expected);
        assert!(validate_claims(&state, &request, &accepted, expected).is_ok());
        let mut installation = accepted.clone();
        installation.installation_id = InstallationId::from_bytes([9; 16]);
        let mut incarnation = accepted.clone();
        incarnation.installation_incarnation_id = InstallationIncarnationId::from_bytes([9; 16]);
        let mut binding = accepted;
        binding.binding_id = BindingId::from_bytes([9; 16]);
        for substituted in [installation, incarnation, binding] {
            assert!(matches!(
                validate_claims(&state, &request, &substituted, expected),
                Err(TypedClientError::GrantMismatch)
            ));
        }
        no_io(&counts);
    }

    #[test]
    fn trusted_identity_does_not_allow_widened_scope_or_permissions() {
        let counts = Rc::new(Counts::default());
        let expected = InstallationId::from_bytes([2; 16]);
        let state = state(&counts, Some(expected));
        let request = request();
        let accepted = claims(&request, expected);
        let mut membership = accepted.clone();
        membership.allowed_membership_ids = one(SourceMembershipId::from_bytes([9; 16]));
        let mut modality = accepted.clone();
        modality.allowed_modalities = one(Modality::Code);
        let mut permission = accepted.clone();
        permission.source_read_permission = true;
        let mut budget = accepted;
        budget.maximum_budget_class = ProfileId::new("unrequested").expect("profile");
        for widened in [membership, modality, permission, budget] {
            assert!(matches!(
                validate_claims(&state, &request, &widened, expected),
                Err(TypedClientError::GrantMismatch)
            ));
        }
        no_io(&counts);
    }
}
