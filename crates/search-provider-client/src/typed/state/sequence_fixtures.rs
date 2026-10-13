//! Sequence/replay and progressive/terminal fixtures for the private client response engine.
//! Focused client-engine cases; execution evidence is recorded at the accepted head.
//!
//! Synthetic owner fixtures over one private `State`. Each case encodes the provider record with
//! the real `ServerEnvelopeCodec` and MACs it with the real keyed
//! `ProviderFrameTranscript::response`, so the exact committed authentication, sequence,
//! correlation and terminal behavior is exercised without a platform connector, native
//! authentication proof, daemon authorization policy or a second handle/continuation authority.
//! These cases assert only what this client's own response engine accepts or refuses.

#[cfg(test)]
mod fixtures {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::io::{self, Read, Write};
    use std::rc::Rc;
    use std::task::Poll;
    use std::time::Duration;

    use search_contracts::{
        AccessPartitionId, BindingId, BoundedList, BoundedNonContentMetadata,
        BoundedProgressCounts, BoundedSet, CancelledBody, ContinuationHandle, ContinuationId,
        CorpusProfileRecipe, CoverageDenominatorKind, DisclosureCeiling, ErrorBody,
        ExactConclusion, ExactExecutionReport, ExactScanPlanRef, ExpandHandleRecipe,
        ExpandHandleTarget, GrantId, HandleExpansionKind, HelloBody, InstallationId,
        InstallationIncarnationId, MAX_SET_ITEMS, MessageKind, Modality, OpaqueHandleToken,
        OpaqueId, OpaqueRef, PlanFingerprint, PlanId, ProfileId, ProgressBody, ProgressPhase,
        ProtocolFailureCode, ProtocolRange, ProtocolRetryability, ProtocolVersion, ProviderBodyV1,
        ProviderEnvelope, ReceiptRef, RecipeBodyV1, RecipeIdV1, RecipeResultV1, RequestBody,
        RequestId, RequestedScope, ResultBody, ScopeDomainId, SearchReadGrantClaims,
        SearchReasonCodeV1, SearchRecipeRequest, SensitivityClass, SourceMembershipId, SourceView,
        UtcTimestamp, WorkspaceId, WorkspaceViewRevisionId, WorkspaceViewSource,
        protocol::PeerRole,
    };
    use search_provider_protocol::{
        BindingContext, BindingKey, ClientEnvelopeCodec, ClientNonce, DEFAULT_PROTOCOL_LIMITS,
        PairingChallenge, PairingMachine, ProofDigest, ProtocolError, ProviderFrameTranscript,
        ServerEnvelopeCodec, ServerNonce, SessionId, SessionMachine, StandaloneGrantRequestV1,
        TransportPeer, authenticate_binding, verify_proof,
    };

    use super::super::State;
    use crate::typed::io::{LocalByteStream, SocketIo};
    use crate::typed::keyed_parts;
    use crate::typed::{TypedClientError, TypedProviderSession};

    /// The one synthetic ceremony, key and server nonce shared by the peer and
    /// the client under test, so a signature cannot silently disagree with the
    /// session it is meant to authenticate.
    const CEREMONY_SESSION_BYTES: [u8; 16] = [1; 16];
    const SERVER_NONCE_BYTES: [u8; 16] = [0x44; 16];
    const PAIRING_KEY_BYTES: [u8; 32] = [0xAB; 32];

    const QUANTUM: Duration = Duration::from_millis(250);

    fn pairing_key() -> BindingKey {
        BindingKey::from_bytes(PAIRING_KEY_BYTES).expect("synthetic key")
    }

    fn server_nonce() -> ServerNonce {
        ServerNonce::from_bytes(SERVER_NONCE_BYTES).expect("synthetic server nonce")
    }

    fn ceremony_session() -> SessionId {
        SessionId::from_bytes(CEREMONY_SESSION_BYTES).expect("synthetic ceremony")
    }

    /// Queues complete authenticated records, then holds the stream open.
    ///
    /// Each queued record is one exact peer record: the canonical length-prefixed
    /// frame followed by its 32-byte keyed proof. Exhaustion reports a retryable
    /// condition instead of EOF, so an absent record is never confused with a
    /// peer that closed mid-record.
    struct QueuedPeer {
        records: VecDeque<Vec<u8>>,
        eof_when_empty: bool,
        fail_write_after: Option<usize>,
        written: Rc<Cell<usize>>,
        captured_write: Rc<RefCell<Vec<u8>>>,
    }

    impl QueuedPeer {
        fn new(
            records: Vec<Vec<u8>>,
            eof_when_empty: bool,
        ) -> (Self, Rc<Cell<usize>>, Rc<RefCell<Vec<u8>>>) {
            let written = Rc::new(Cell::new(0));
            let captured_write = Rc::new(RefCell::new(Vec::new()));
            (
                Self {
                    records: records.into_iter().collect(),
                    eof_when_empty,
                    fail_write_after: None,
                    written: Rc::clone(&written),
                    captured_write: Rc::clone(&captured_write),
                },
                written,
                captured_write,
            )
        }

        fn failing_after(
            records: Vec<Vec<u8>>,
            bytes: usize,
        ) -> (Self, Rc<Cell<usize>>, Rc<RefCell<Vec<u8>>>) {
            let (mut peer, written, captured_write) = Self::new(records, false);
            peer.fail_write_after = Some(bytes);
            (peer, written, captured_write)
        }
    }

    impl Read for QueuedPeer {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let Some(front) = self.records.front_mut() else {
                return if self.eof_when_empty {
                    Ok(0)
                } else {
                    Err(io::ErrorKind::WouldBlock.into())
                };
            };
            let count = bytes.len().min(front.len());
            bytes[..count].copy_from_slice(&front[..count]);
            front.drain(..count);
            let drained = front.is_empty();
            if drained {
                self.records.pop_front();
            }
            Ok(count)
        }
    }

    impl Write for QueuedPeer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if let Some(limit) = self.fail_write_after {
                let written = self.written.get();
                if written >= limit {
                    return Err(io::ErrorKind::BrokenPipe.into());
                }
                let count = bytes.len().min(limit - written);
                self.written.set(written + count);
                self.captured_write
                    .borrow_mut()
                    .extend_from_slice(&bytes[..count]);
                return Ok(count);
            }
            self.written.set(self.written.get() + bytes.len());
            self.captured_write.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl LocalByteStream for QueuedPeer {
        fn configure_local(&self) -> Result<(), TypedClientError> {
            Ok(())
        }
    }

    /// One fully paired, activated client owner reading from the queued peer.
    fn owner(records: Vec<Vec<u8>>) -> State {
        owner_with_peer(QueuedPeer::new(records, false).0)
    }

    fn owner_eof(records: Vec<Vec<u8>>) -> State {
        owner_with_peer(QueuedPeer::new(records, true).0)
    }

    fn owner_with_peer(stream: QueuedPeer) -> State {
        let version = ProtocolVersion { major: 1, minor: 0 };
        let proof = ProofDigest::from_bytes([9; 32]);
        let mut ceremony = PairingMachine::new(version, proof);
        ceremony
            .issue_challenge(
                ceremony_session(),
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
        let pairing = ceremony.into_verified().expect("pairing");
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
        let binding: BindingContext =
            authenticate_binding(&hello, &pairing, &peer.incarnation, &peer).expect("binding");
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 1).expect("session");
        session.negotiate(version).expect("negotiated");
        session.activate(&proof, &proof).expect("active");
        State::new(
            SocketIo::from_local_stream(stream),
            binding,
            pairing,
            pairing_key(),
            server_nonce(),
            DEFAULT_PROTOCOL_LIMITS,
            session,
        )
    }

    fn one<T: Ord>(value: T) -> BoundedSet<T, MAX_SET_ITEMS> {
        BoundedSet::from_items([value]).expect("one bounded fixture value")
    }

    /// A private fixture grant lets the client-engine tests reach its real send
    /// path. Server grant issuance/authorization is covered by the grant owner.
    fn grant() -> SearchReadGrantClaims {
        grant_for(RecipeIdV1::CorpusProfile)
    }

    fn grant_for(recipe: RecipeIdV1) -> SearchReadGrantClaims {
        SearchReadGrantClaims {
            grant_id: GrantId::from_bytes([0x51; 16]),
            installation_id: InstallationId::from_bytes([0x52; 16]),
            installation_incarnation_id: InstallationIncarnationId::from_bytes([3; 16]),
            binding_id: BindingId::from_bytes([4; 16]),
            principal_opaque_id: OpaqueId::new("fixture-principal").expect("principal"),
            client_scope_ref: OpaqueRef::new("fixture-scope").expect("scope"),
            scope_domain_id: ScopeDomainId::from_bytes([0x53; 16]),
            allowed_membership_ids: one(SourceMembershipId::from_bytes([0x54; 16])),
            allowed_corpus_or_portfolio_ids: BoundedSet::empty(),
            reference_portfolio_revision: None,
            allowed_access_partitions: one(AccessPartitionId::from_bytes([0x55; 16])),
            allowed_modalities: one(Modality::Text),
            permitted_recipe_families: one(recipe),
            maximum_budget_class: ProfileId::new("interactive").expect("budget"),
            sensitivity_ceiling: SensitivityClass::Project,
            disclosure_ceiling: DisclosureCeiling::LocalOnly,
            source_read_permission: false,
            exact_scan_permission: false,
            issued_boot_id: OpaqueId::new("fixture-boot").expect("boot"),
            issued_at: UtcTimestamp::parse("2099-01-01T00:00:00.000000Z").expect("issued"),
            expires_at: UtcTimestamp::parse("2099-01-01T00:00:30.000000Z").expect("expires"),
            nonce: OpaqueId::new("fixture-grant-nonce").expect("nonce"),
            revocation_generation: 0,
        }
    }

    /// Reserves a real client request by encoding, MACing and writing it through
    /// `State::send_request`; the following signed provider records must then
    /// causally correlate to that pending request.
    fn profile_request(id: RequestId) -> RequestBody {
        let grant = grant();
        let workspace = WorkspaceId::from_bytes([0x56; 16]);
        let recipe_request = SearchRecipeRequest::new(
            id,
            RecipeIdV1::CorpusProfile,
            SourceView::WorkingTreeCurrent(WorkspaceViewSource {
                workspace_instance_id: workspace,
                workspace_view_revision_ref: WorkspaceViewRevisionId::from_bytes([0x57; 16]),
            }),
            RequestedScope::ActiveWorkspace(workspace),
            ProfileId::new("interactive").expect("budget"),
            RecipeBodyV1::CorpusProfile(CorpusProfileRecipe {
                facets: BoundedSet::empty(),
            }),
        )
        .expect("valid profile request");
        RequestBody {
            grant,
            recipe_request,
        }
    }

    fn send_profile_request(state: &mut State, id: RequestId) {
        send_profile_request_with_timeout(state, id, Duration::from_secs(30));
    }

    fn send_profile_request_with_timeout(state: &mut State, id: RequestId, timeout: Duration) {
        let body = profile_request(id);
        state.grant = Some(body.grant.clone());
        assert_eq!(
            state
                .send_request(body, timeout)
                .expect("request is sent through the client engine"),
            id
        );
    }

    fn grant_request() -> StandaloneGrantRequestV1 {
        StandaloneGrantRequestV1 {
            expected_binding_generation: 1,
            expected_policy_generation: 1,
            requested_membership_ids: one(SourceMembershipId::from_bytes([0x54; 16])),
            requested_corpus_or_portfolio_ids: BoundedSet::empty(),
            requested_access_partitions: one(AccessPartitionId::from_bytes([0x55; 16])),
            requested_modalities: one(Modality::Text),
            requested_recipe_families: one(RecipeIdV1::CorpusProfile),
            requested_budget_class: ProfileId::new("interactive").expect("budget"),
            requested_sensitivity_ceiling: SensitivityClass::Project,
            requested_disclosure_ceiling: DisclosureCeiling::LocalOnly,
            requested_source_read_permission: false,
            requested_exact_scan_permission: false,
            requested_ttl_ms: 30_000,
        }
    }

    fn continuation_request(id: RequestId, handle: ContinuationHandle) -> RequestBody {
        let grant = grant_for(RecipeIdV1::ExpandHandle);
        let workspace = WorkspaceId::from_bytes([0x56; 16]);
        let recipe_request = SearchRecipeRequest::new(
            id,
            RecipeIdV1::ExpandHandle,
            SourceView::WorkingTreeCurrent(WorkspaceViewSource {
                workspace_instance_id: workspace,
                workspace_view_revision_ref: WorkspaceViewRevisionId::from_bytes([0x57; 16]),
            }),
            RequestedScope::ActiveWorkspace(workspace),
            ProfileId::new("interactive").expect("budget"),
            RecipeBodyV1::ExpandHandle(ExpandHandleRecipe {
                handle: ExpandHandleTarget::Continuation(handle),
                expansion: HandleExpansionKind::Continuation,
                max_bytes: 512,
            }),
        )
        .expect("valid continuation recipe");
        RequestBody {
            grant,
            recipe_request,
        }
    }

    fn provider_envelope(
        request: RequestId,
        sequence: u64,
        body: ProviderBodyV1,
    ) -> ProviderEnvelope {
        ProviderEnvelope {
            protocol_major: 1,
            protocol_minor: 0,
            installation_incarnation_id: InstallationIncarnationId::from_bytes([3; 16]),
            binding_id: BindingId::from_bytes([4; 16]),
            connection_sequence: sequence,
            request_id: request,
            message_kind: body.message_kind(),
            relative_deadline_ms: None,
            body,
        }
    }

    fn progressed(
        request: RequestId,
        sequence: u64,
        event: u64,
        completed_legs: u32,
    ) -> ProviderEnvelope {
        provider_envelope(
            request,
            sequence,
            ProviderBodyV1::Progress(ProgressBody {
                event_sequence: event,
                phase: ProgressPhase::Retrieving,
                bounded_counts: BoundedProgressCounts {
                    completed_legs,
                    total_planned_legs: 2,
                    nominated_candidates: 1,
                    validated_candidates: 1,
                    omitted_or_failed_legs: 0,
                },
                degraded_reason_codes: BoundedSet::empty(),
            }),
        )
    }

    fn terminated(request: RequestId, sequence: u64) -> ProviderEnvelope {
        provider_envelope(
            request,
            sequence,
            ProviderBodyV1::Cancelled(CancelledBody {
                target_request_id: request,
                terminal: true,
            }),
        )
    }

    fn cancellation_ack(control: RequestId, target: RequestId, sequence: u64) -> ProviderEnvelope {
        provider_envelope(
            control,
            sequence,
            ProviderBodyV1::Cancelled(CancelledBody {
                target_request_id: target,
                terminal: false,
            }),
        )
    }

    /// One exact peer record: the real server frame plus its keyed response MAC.
    fn signed(envelope: &ProviderEnvelope) -> Vec<u8> {
        let versions = ProtocolRange::new(
            ProtocolVersion { major: 1, minor: 0 },
            ProtocolVersion { major: 1, minor: 0 },
        )
        .expect("supported versions");
        let frame = ServerEnvelopeCodec::encode(envelope, DEFAULT_PROTOCOL_LIMITS, versions)
            .expect("server frame");
        let proof = keyed_parts(
            &pairing_key(),
            ProviderFrameTranscript::response(ceremony_session(), server_nonce(), frame.as_slice())
                .parts(),
        );
        let mut record = frame.as_slice().to_vec();
        record.extend_from_slice(proof.as_bytes());
        record
    }

    /// The same record with a corrupted trailer, so the body is never released.
    fn corrupt(envelope: &ProviderEnvelope) -> Vec<u8> {
        let mut record = signed(envelope);
        let last = record.len() - 1;
        record[last] ^= 0xFF;
        record
    }

    fn ready<R>(polled: Poll<R>) -> R {
        match polled {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("queued record must deliver"),
        }
    }

    #[test]
    fn one_authenticated_pending_request_delivers_ordered_events_then_exactly_one_terminal() {
        let request = RequestId::from_bytes([0x11; 16]);
        let mut state = owner(vec![
            signed(&progressed(request, 1, 1, 1)),
            signed(&progressed(request, 2, 2, 2)),
            signed(&terminated(request, 3)),
            signed(&terminated(request, 4)),
        ]);
        send_profile_request(&mut state, request);

        let first = state.receive().expect("first progressive event");
        assert_eq!(first.message_kind, MessageKind::Progress);
        assert_eq!(first.request_id, request);
        assert_eq!(first.connection_sequence, 1);
        let held = state
            .requests
            .get(&request)
            .expect("progress is not terminal");
        assert_eq!(held.events.next_expected(), Some(2));
        assert_eq!(held.progress.expect("progress retained").completed(), 1);

        let second = ready(
            state
                .poll_receive(QUANTUM)
                .expect("second progressive event"),
        );
        assert_eq!(second.message_kind, MessageKind::Progress);
        assert_eq!(second.connection_sequence, 2);
        let held = state
            .requests
            .get(&request)
            .expect("progress is not terminal");
        assert_eq!(held.events.next_expected(), Some(3));
        assert_eq!(held.progress.expect("progress retained").completed(), 2);

        let terminal = ready(state.poll_receive(QUANTUM).expect("authenticated terminal"));
        assert_eq!(terminal.message_kind, MessageKind::Cancelled);
        assert_eq!(terminal.connection_sequence, 3);
        assert!(state.requests.is_empty(), "terminal released the sole slot");

        // The terminal already consumed the correlation identity, so a further
        // body for it cannot release a second completion.
        let repeated = state
            .poll_receive(QUANTUM)
            .expect_err("no read is permitted after the request terminal");
        assert!(matches!(repeated, TypedClientError::NothingPending));
        assert!(state.requests.is_empty());
    }

    #[test]
    fn a_stale_provider_sequence_is_refused_without_releasing_the_pending_request() {
        let request = RequestId::from_bytes([0x22; 16]);
        let mut state = owner(vec![
            signed(&progressed(request, 1, 1, 1)),
            signed(&progressed(request, 1, 1, 1)),
            signed(&progressed(request, 2, 2, 2)),
        ]);
        send_profile_request(&mut state, request);

        let first = ready(
            state
                .poll_receive(QUANTUM)
                .expect("first progressive event"),
        );
        assert_eq!(first.connection_sequence, 1);

        // The peer replays the exact accepted sequence. The record is authentic,
        // so only the directional ledger can refuse it.
        let stale = state
            .poll_receive(QUANTUM)
            .expect_err("a repeated provider sequence must be refused");
        assert!(matches!(
            stale,
            TypedClientError::Protocol(ProtocolError::DuplicateSequence)
        ));
        let held = state
            .requests
            .get(&request)
            .expect("refusal releases nothing");
        assert_eq!(held.events.next_expected(), Some(2));
        assert_eq!(held.progress.expect("progress retained").completed(), 1);
        assert_eq!(
            state.session.sequences().provider().next_expected(),
            Some(2),
            "a refused record spends no sequence"
        );

        // The refusal consumed nothing, so the exact next sequence still delivers.
        let resumed = ready(state.poll_receive(QUANTUM).expect("resumed delivery"));
        assert_eq!(resumed.connection_sequence, 2);
        let held = state.requests.get(&request).expect("still pending");
        assert_eq!(held.events.next_expected(), Some(3));
        assert_eq!(held.progress.expect("progress retained").completed(), 2);
    }

    #[test]
    fn authenticated_progress_regression_preserves_all_cursors_and_can_recover() {
        let request = RequestId::from_bytes([0x23; 16]);
        let mut state = owner(vec![
            signed(&progressed(request, 1, 1, 1)),
            signed(&progressed(request, 2, 2, 0)),
            signed(&progressed(request, 2, 2, 2)),
            signed(&terminated(request, 3)),
        ]);
        send_profile_request(&mut state, request);
        state.receive().expect("first progress");

        assert!(matches!(
            state.poll_receive(QUANTUM),
            Err(TypedClientError::Protocol(ProtocolError::ProgressRegression))
        ));
        let held = state.requests.get(&request).expect("no terminal on refusal");
        assert_eq!(held.events.next_expected(), Some(2));
        assert_eq!(held.progress.expect("original progress").completed(), 1);
        assert_eq!(state.session.sequences().provider().next_expected(), Some(2));

        assert_eq!(state.receive().expect("same sequence recovers").connection_sequence, 2);
        assert_eq!(state.requests.get(&request).unwrap().progress.unwrap().completed(), 2);
        assert_eq!(state.receive().expect("terminal").message_kind, MessageKind::Cancelled);
        assert!(state.requests.is_empty());
    }

    #[test]
    fn a_response_without_the_bound_keyed_proof_is_refused_before_its_body_is_released() {
        let request = RequestId::from_bytes([0x33; 16]);
        let mut state = owner(vec![corrupt(&progressed(request, 1, 1, 1))]);
        send_profile_request(&mut state, request);

        let refused = state
            .poll_receive(QUANTUM)
            .expect_err("an unbound proof cannot authenticate a response");
        assert!(matches!(
            refused,
            TypedClientError::Protocol(ProtocolError::AuthenticationFailed)
        ));
        let held = state
            .requests
            .get(&request)
            .expect("refusal releases nothing");
        assert_eq!(held.progress, None);
        assert_eq!(
            state.session.sequences().provider().next_expected(),
            Some(1),
            "an unauthenticated record never enters the sequence ledger"
        );
    }

    #[test]
    fn cancellation_ack_keeps_the_original_request_pending_until_its_terminal() {
        let request = RequestId::from_bytes([0x41; 16]);
        let cancel = RequestId::from_bytes([0x42; 16]);
        let mut state = owner(vec![
            signed(&cancellation_ack(cancel, request, 1)),
            signed(&terminated(request, 2)),
        ]);
        send_profile_request(&mut state, request);
        let original_deadline = state
            .requests
            .get(&request)
            .expect("pending request")
            .deadline;

        state
            .send_cancel(cancel, request, Duration::from_secs(30))
            .expect("cancel control uses the same client owner");
        let acknowledgement = ready(state.poll_receive(QUANTUM).expect("cancel acknowledgement"));
        assert_eq!(acknowledgement.request_id, cancel);
        assert!(
            state.cancel.is_none(),
            "control slot is released by its acknowledgement"
        );
        let pending = state
            .requests
            .get(&request)
            .expect("target remains pending");
        assert!(pending.cancellation_acknowledged);
        assert_eq!(
            pending.deadline, original_deadline,
            "ack cannot renew target deadline"
        );

        let terminal = ready(state.poll_receive(QUANTUM).expect("target terminal"));
        assert_eq!(terminal.request_id, request);
        assert!(
            state.requests.is_empty(),
            "only the target terminal releases its slot"
        );
    }

    #[test]
    fn request_deadline_is_not_renewed_by_a_longer_poll_quantum() {
        let request = RequestId::from_bytes([0x43; 16]);
        let mut state = owner(Vec::new());
        send_profile_request_with_timeout(&mut state, request, Duration::from_millis(10));

        let expired = state
            .poll_receive(Duration::from_secs(1))
            .expect_err("poll quantum cannot replace the request deadline");
        assert!(matches!(expired, TypedClientError::DeadlineExpired));
        assert!(
            state.requests.contains_key(&request),
            "expiry does not forge a terminal"
        );
    }

    #[test]
    fn eof_after_a_request_write_is_unknown_and_the_typed_owner_cannot_be_reused() {
        let request = RequestId::from_bytes([0x44; 16]);
        let state = owner_eof(Vec::new());
        let mut owner = TypedProviderSession { state: Some(state) };
        let body = profile_request(request);
        owner.state.as_mut().expect("open owner").grant = Some(body.grant.clone());
        owner
            .send_request(body, Duration::from_secs(30))
            .expect("request write completes");

        let error = owner
            .receive()
            .expect_err("peer EOF prevents an authenticated response");
        assert!(matches!(error, TypedClientError::PeerClosed));
        assert_eq!(error.outcome_class(), crate::OutcomeClass::Unknown);
        assert!(
            owner.state.is_none(),
            "failed operation drops the sole stream and key"
        );
        assert!(matches!(owner.receive(), Err(TypedClientError::Closed)));
    }

    #[test]
    fn a_partial_request_write_reports_unknown_outcome_and_drops_the_owner() {
        let request = RequestId::from_bytes([0x45; 16]);
        let (peer, written, _) = QueuedPeer::failing_after(Vec::new(), 1);
        let state = owner_with_peer(peer);
        let mut owner = TypedProviderSession { state: Some(state) };
        let body = profile_request(request);
        owner.state.as_mut().expect("open owner").grant = Some(body.grant.clone());

        let error = owner
            .send_request(body, Duration::from_secs(30))
            .expect_err("transport fails after a peer-visible request prefix");
        assert!(matches!(error, TypedClientError::Io));
        assert_eq!(
            written.get(),
            1,
            "at least one frame byte could have reached the peer"
        );
        assert_eq!(error.outcome_class(), crate::OutcomeClass::Unknown);
        assert!(
            owner.state.is_none(),
            "the uncertain request is never replayed on this owner"
        );
        assert!(matches!(owner.receive(), Err(TypedClientError::Closed)));
    }

    #[test]
    fn the_original_grant_deadline_covers_both_command_writes_and_response_wait() {
        let (peer, written, _) = QueuedPeer::new(Vec::new(), false);
        let state = owner_with_peer(peer);
        let mut owner = TypedProviderSession { state: Some(state) };
        owner
            .state
            .as_mut()
            .expect("open owner")
            .bind_trusted_installation_id(InstallationId::from_bytes([0x52; 16]))
            .expect("independent trust fixture");

        let error = owner
            .request_standalone_grant(
                RequestId::from_bytes([0x46; 16]),
                grant_request(),
                Duration::from_millis(250),
            )
            .expect_err("the peer withholds the first grant response record");
        assert!(matches!(error, TypedClientError::DeadlineExpired));
        assert!(
            written.get() > 0,
            "both command records were peer-visible before timeout"
        );
        assert_eq!(error.outcome_class(), crate::OutcomeClass::Unknown);
        assert!(owner.state.is_none(), "grant timeout drops the sole owner");
        assert!(matches!(owner.receive(), Err(TypedClientError::Closed)));
    }

    #[test]
    fn a_foreign_continuation_stays_opaque_and_a_wrong_recipe_result_cannot_complete_it() {
        let request = RequestId::from_bytes([0x47; 16]);
        let handle = ContinuationHandle {
            continuation_id: ContinuationId::from_bytes([0x61; 16]),
            expires_at: UtcTimestamp::parse("2099-01-01T00:01:00.000000Z").expect("expiry"),
            opaque_token: OpaqueHandleToken::new(&[0x62; 32]).expect("opaque token"),
        };
        let wrong_recipe = RecipeResultV1::ExecuteExactScan(ExactExecutionReport {
            plan_ref: ExactScanPlanRef {
                plan_id: PlanId::from_bytes([0x63; 16]),
                plan_fingerprint: PlanFingerprint::from_bytes([0x64; 32]),
            },
            matched_items: BoundedList::empty(),
            scanned_items: 0,
            scanned_bytes: 0,
            unreadable_items: BoundedList::empty(),
            changed_or_unavailable_items: BoundedList::empty(),
            timed_out: true,
            cancelled: false,
            scope_drifted: false,
            coverage: CoverageDenominatorKind::CandidateScope,
            conclusion: ExactConclusion::Incomplete,
            receipt_ref: ReceiptRef::new("fixture-exact-receipt").expect("receipt"),
        });
        let peer_response = provider_envelope(
            request,
            1,
            ProviderBodyV1::Result(ResultBody {
                event_sequence: 1,
                result: wrong_recipe,
            }),
        );
        let wrong_request_id = RequestId::from_bytes([0x48; 16]);
        let uncorrelated = progressed(wrong_request_id, 1, 1, 1);
        let denied = provider_envelope(
            request,
            1,
            ProviderBodyV1::Error(ErrorBody {
                code: ProtocolFailureCode::Search(SearchReasonCodeV1::SecurityFailClosed),
                retryability: ProtocolRetryability::Never,
                message_template_id: OpaqueId::new("fixture-denial").expect("template"),
                bounded_metadata: BoundedNonContentMetadata::empty(),
            }),
        );
        let (stream, _, captured) = QueuedPeer::new(
            vec![
                signed(&uncorrelated),
                signed(&peer_response),
                signed(&denied),
            ],
            false,
        );
        let mut state = owner_with_peer(stream);
        let body = continuation_request(request, handle.clone());
        state.grant = Some(body.grant.clone());
        state
            .send_request(body.clone(), Duration::from_secs(30))
            .expect("continuation recipe is sent through the real client engine");

        let bytes = captured.borrow();
        let prefix: [u8; 4] = bytes[..4].try_into().expect("client frame prefix");
        let frame_end = 4 + usize::try_from(u32::from_le_bytes(prefix)).expect("frame length");
        let frame = &bytes[..frame_end];
        let observed_proof = ProofDigest::from_bytes(
            bytes[frame_end..frame_end + 32]
                .try_into()
                .expect("complete keyed request proof"),
        );
        let expected_proof = keyed_parts(
            &pairing_key(),
            ProviderFrameTranscript::request(ceremony_session(), server_nonce(), frame).parts(),
        );
        assert!(verify_proof(&expected_proof, &observed_proof));
        let versions = ProtocolRange::new(
            ProtocolVersion { major: 1, minor: 0 },
            ProtocolVersion { major: 1, minor: 0 },
        )
        .expect("version range");
        let decoded = ClientEnvelopeCodec::decode(frame, DEFAULT_PROTOCOL_LIMITS, versions)
            .expect("canonical client envelope");
        let ProviderBodyV1::Request(sent) = decoded.body else {
            panic!("client wrote the continuation recipe")
        };
        assert_eq!(sent.recipe_request, body.recipe_request);
        assert_eq!(
            sent.recipe_request.body,
            RecipeBodyV1::ExpandHandle(ExpandHandleRecipe {
                handle: ExpandHandleTarget::Continuation(handle),
                expansion: HandleExpansionKind::Continuation,
                max_bytes: 512,
            }),
            "the opaque foreign-session handle remains caller data"
        );

        let uncorrelated = state
            .poll_receive(QUANTUM)
            .expect_err("an authenticated response for a different request is refused");
        assert!(matches!(uncorrelated, TypedClientError::ResponseMismatch));
        assert!(state.requests.contains_key(&request));
        assert_eq!(
            state.session.sequences().provider().next_expected(),
            Some(1)
        );

        let rejected = state
            .poll_receive(QUANTUM)
            .expect_err("an authenticated result for a different recipe is refused");
        assert!(matches!(rejected, TypedClientError::ResponseMismatch));
        assert!(
            state.requests.contains_key(&request),
            "wrong recipe cannot complete the request"
        );
        assert_eq!(
            state.session.sequences().provider().next_expected(),
            Some(1),
            "refused recipe result spends no provider sequence"
        );

        // This synthetic authenticated denial asserts only the client boundary:
        // an authority owner may reject a foreign continuation and that denial
        // terminates the exact correlated request without a client-side registry.
        let terminal = ready(
            state
                .poll_receive(QUANTUM)
                .expect("correlated server denial"),
        );
        assert_eq!(terminal.request_id, request);
        assert_eq!(terminal.message_kind, MessageKind::Error);
        assert!(state.requests.is_empty());
    }
}
