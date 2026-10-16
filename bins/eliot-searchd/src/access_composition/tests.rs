use super::*;
use search_access::{
    AccessModality, AuthoritativeAccessSnapshot, AuthoritativePolicyState,
    AccessError, GrantClaims, GrantValidationContext, IndexedRouteFence,
    LiveSecurityState, MembershipAccessBinding, NamespacePolicyFence,
    PreRetrievalPlan, PreRetrievalRequest, RequestedMembershipScope,
    compile_pre_retrieval,
};
use search_contracts::{
    AccessPolicyRevision, BindingId, Blake3Digest32, CollectionGenerationId,
    Epoch, GrantId, InstallationId, InstallationIncarnationId, OpaqueId,
    OwnerEpoch, PurgeFenceRevision, RecipeIdV1, ShadowFenceRevision,
    SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration,
};
use std::collections::{BTreeMap, BTreeSet};

struct Bundle {
    claims: GrantClaims,
    context: GrantValidationContext,
    requested: RequestedMembershipScope,
    grant_scope: BTreeSet<SourceMembershipId>,
    authoritative: AuthoritativeAccessSnapshot,
    policy_fence: NamespacePolicyFence,
    authoritative_policy: AuthoritativePolicyState,
    route: IndexedRouteFence,
    live: LiveSecurityState,
}

fn oid(value: &str) -> OpaqueId {
    OpaqueId::new(value).expect("fixture id")
}

fn member(n: u128) -> SourceMembershipId {
    SourceMembershipId::from_bytes(n.to_be_bytes())
}

fn bundle() -> Bundle {
    let installation_id = InstallationId::from_bytes([1; 16]);
    let incarnation = InstallationIncarnationId::from_bytes([2; 16]);
    let binding_id = BindingId::from_bytes([3; 16]);
    let boot = oid("boot-gate-0001");
    let members = [member(1), member(2)];
    let bindings = members
        .into_iter()
        .map(|membership| {
            (
                membership,
                MembershipAccessBinding {
                    membership_id: membership,
                    access_partition_digest: Blake3Digest32::from_bytes([0xA1; 32]),
                    scoring_partition_digest: Blake3Digest32::from_bytes([0xB2; 32]),
                    projection_membership_id: oid("projection-gate"),
                    active: true,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let namespace_id = SourceNamespaceId::from_bytes([0x20; 16]);
    let owner_generation = SourceOwnerGeneration::from_bytes([0x21; 32]);
    Bundle {
        claims: GrantClaims {
            grant_id: GrantId::from_bytes([9; 16]),
            binding_id,
            installation_id,
            installation_incarnation_id: incarnation,
            issued_boot_id: boot.clone(),
            issued_at_ms: 1_000,
            expires_at_ms: 2_000,
            nonce: oid("nonce-gate-0001"),
            revocation_generation: 4,
            allowed_recipes: BTreeSet::from([RecipeIdV1::FindText]),
            allowed_modalities: BTreeSet::from([AccessModality::Direct]),
            allowed_budget_classes: BTreeSet::from([oid("budget-gate")]),
            max_source_read_bytes: 1_024,
            max_result_bytes: 1_024,
        },
        context: GrantValidationContext {
            binding_id,
            installation_id,
            installation_incarnation_id: incarnation,
            boot_id: boot,
            now_ms: 1_500,
            current_revocation_generation: 4,
            signature_verified: true,
            pairing_verified: true,
            nonce_accepted: true,
        },
        requested: RequestedMembershipScope {
            memberships: BTreeSet::from(members),
        },
        grant_scope: BTreeSet::from(members),
        authoritative: AuthoritativeAccessSnapshot {
            generation: 7,
            source_catalog_generation: 3,
            membership_generation: 5,
            bindings,
            snapshot_digest: Blake3Digest32::from_bytes([0xCC; 32]),
        },
        policy_fence: NamespacePolicyFence {
            namespace_id,
            owner_generation,
            policy_revision: AccessPolicyRevision::new(11),
        },
        authoritative_policy: AuthoritativePolicyState {
            namespace_id,
            owner_generation,
            policy_revision: AccessPolicyRevision::new(11),
            shadow_revision: ShadowFenceRevision::new(6),
            purge_revision: PurgeFenceRevision::new(6),
        },
        route: IndexedRouteFence {
            collection_generation_id: CollectionGenerationId::from_bytes([0x10; 16]),
            visible_epoch: Epoch::new(12).expect("epoch"),
            route_generation: 2,
            owner_epoch: OwnerEpoch::new(1).expect("owner"),
        },
        live: LiveSecurityState {
            generation: 7,
            denied_memberships: BTreeSet::new(),
            purged_memberships: BTreeSet::new(),
            fail_closed: false,
            snapshot_digest: Blake3Digest32::from_bytes([0xDD; 32]),
        },
    }
}

fn admit(bundle: &Bundle) -> Result<PreRetrievalAdmission, AccessGateDenial> {
    admit_pre_retrieval(PreRetrievalRequest {
        claims: bundle.claims.clone(),
        validation_context: &bundle.context,
        recipe: RecipeIdV1::FindText,
        modality: AccessModality::Direct,
        requested_scope: &bundle.requested,
        grant_scope: &bundle.grant_scope,
        authoritative: &bundle.authoritative,
        policy_fence: &bundle.policy_fence,
        authoritative_policy: &bundle.authoritative_policy,
        route: bundle.route,
        live: &bundle.live,
        overlap_proof: None,
        max_legs: 8,
    })
}

#[test]
fn clean_request_admits_with_predicate_digests() {
    let bundle = bundle();
    let admission = admit(&bundle).expect("clean request admits");
    assert_eq!(admission.leg_count, 2);
    assert_eq!(admission.live_generation, 7);
    assert_eq!(admission.predicate_digests.len(), 2);
}

#[test]
fn revoked_grant_denies_before_provider_dispatch() {
    let mut bundle = bundle();
    bundle.context.current_revocation_generation = 99;
    let denial = admit(&bundle).expect_err("revoked grant denies");
    assert_eq!(denial.reason, ACCESS_PRE_RETRIEVAL_DENIED);
    assert_eq!(denial.access_code, AccessError::GrantRevoked.code());
}

#[test]
fn live_purge_denies_admission() {
    let mut bundle = bundle();
    bundle.live.purged_memberships.insert(member(1));
    let denial = admit(&bundle).expect_err("purged scope denies");
    assert_eq!(denial.access_code, AccessError::LivePurge.code());
}

#[test]
fn emission_recheck_fires_on_new_revocation() {
    let live = bundle().live;
    let memberships = BTreeSet::from([member(1), member(2)]);
    recheck_before_emission(&memberships, 7, &live).expect("clean live passes");
    let mut revoked = live;
    revoked.generation = 8;
    revoked.denied_memberships.insert(member(2));
    let denial = recheck_before_emission(&memberships, 7, &revoked)
        .expect_err("new revocation fires at emission");
    assert_eq!(denial.reason, ACCESS_LIVE_BARRIER_DENIED);
    assert_eq!(denial.access_code, AccessError::LiveRevocation.code());
}

#[test]
fn expansion_reauthorizes_live_state() {
    let live = bundle().live;
    let memberships = BTreeSet::from([member(1)]);
    recheck_before_expansion(&memberships, 7, &live, false).expect("clean handle pass");
    recheck_before_expansion(&memberships, 7, &live, true)
        .expect("clean continuation pass");
    let mut purged = live;
    purged.generation = 8;
    purged.purged_memberships.insert(member(1));
    let denial = recheck_before_expansion(&memberships, 7, &purged, false)
        .expect_err("purge fires at expansion");
    assert_eq!(denial.access_code, AccessError::LivePurge.code());
}

#[test]
fn contaminated_legs_discarded_whole() {
    let bundle = bundle();
    let admission = admit(&bundle).expect("baseline admits");
    assert_eq!(admission.leg_count, 2);
    let plan = compile_pre_retrieval(PreRetrievalRequest {
        claims: bundle.claims.clone(),
        validation_context: &bundle.context,
        recipe: RecipeIdV1::FindText,
        modality: AccessModality::Direct,
        requested_scope: &bundle.requested,
        grant_scope: &bundle.grant_scope,
        authoritative: &bundle.authoritative,
        policy_fence: &bundle.policy_fence,
        authoritative_policy: &bundle.authoritative_policy,
        route: bundle.route,
        live: &bundle.live,
        overlap_proof: None,
        max_legs: 8,
    })
    .expect("baseline plan compiles");
    let execution = plan
        .legs
        .iter()
        .map(|leg| search_access::LegSecurityPopulation {
            leg_id: leg.leg_id,
            memberships: leg.memberships.clone(),
            security_generation: bundle.live.generation,
            idf_population_digest: None,
        })
        .collect::<Vec<_>>();
    let mut moved = bundle.live.clone();
    moved.generation = 8;
    moved.denied_memberships.insert(member(2));
    assert_eq!(
        discard_contaminated_legs(&execution, &bundle.live, &moved),
        BTreeSet::from([1])
    );
}

#[test]
fn local_token_never_admits() {
    let denial = deny_local_token_as_authority();
    assert_eq!(denial.reason, ACCESS_LOCAL_TOKEN_NOT_AUTHORITY);
    assert_eq!(
        denial.access_code,
        AccessError::LocalIdentityNotAuthority.code()
    );
}

#[test]
fn qdrant_parity_deferred() {
    assert_eq!(QDRANT_LIVE_PARITY_DEFERRED_TO, "T24");
    assert_eq!(
        QDRANT_LIVE_PARITY_DEFERRED_TO,
        search_access::QDRANT_LIVE_PARITY_DEFERRED_TO
    );
}

mod retained_source_read_conditions {
    use super::*;
    use search_access::{
        RequestSecurityFence, SecurityDependentReceipt, SecurityRestriction,
        MAX_SECURITY_DEPENDENTS,
    };
    use search_contracts::{
        BoundedList, BoundedSet, DataRootId, InstallationIncarnationId, OpaqueRef, ReceiptRef,
        RequestId, SecurityMutationPhase,
    };
    use search_control_redb::{
        ControlRecordCondition, ControlSnapshotPublisher, JournalIdentity, JournalLimits,
        JournalProfileInitialization, MutationId, PersistentControlJournal,
        policy_codec::AccessPolicyRecord,
        security_restriction::SecurityPolicyState,
    };
    use search_handles::{HandlePolicy, HandleStore};
    use search_ports::{FakeCancellation, OperationContext};
    use std::fs::{self, OpenOptions};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            for _ in 0..128 {
                let path = std::env::temp_dir().join(format!(
                    "eliot-native-security-read-fence-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed),
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("fixture directory: {error}"),
                }
            }
            panic!("fixture collision limit");
        }

        fn create_journal(&self) -> PersistentControlJournal {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(self.0.join("control.redb"))
                .expect("create actual redb fixture");
            let mut journal = PersistentControlJournal::create(
                file,
                JournalIdentity {
                    installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
                    data_root_id: DataRootId::from_bytes([3; 16]),
                    owner_epoch: OwnerEpoch::new(1).expect("owner epoch"),
                    path_identity_digest: Blake3Digest32::from_bytes([4; 32]),
                    schema_family_digest: Blake3Digest32::from_bytes([5; 32]),
                    schema_version: search_control_redb::CURRENT_JOURNAL_SCHEMA_VERSION,
                },
                JournalLimits::BASELINE,
            )
            .expect("actual redb initialization");
            journal
                .initialize_operating_profile(
                    JournalProfileInitialization::Direct,
                    MutationId([6; 32]),
                    Blake3Digest32::from_bytes([7; 32]),
                    &context(),
                )
                .expect("explicit DIRECT profile");
            journal
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct Fixture {
        domain: NativeSecurityDomain,
        binding: NativeSecurityBinding,
        state: SecurityPolicyState,
        expected: [ControlRecordCondition; 2],
        journal: PersistentControlJournal,
        publisher: ControlSnapshotPublisher,
        _scratch: Scratch,
    }

    fn context() -> OperationContext<FakeCancellation> {
        OperationContext::new(
            RequestId::from_bytes([8; 16]),
            60_000,
            FakeCancellation::new(false),
            OpaqueRef::new("budget:native-security-read-fence").expect("budget"),
        )
        .expect("operation context")
    }

    fn fixture() -> Fixture {
        let scratch = Scratch::new();
        let mut journal = scratch.create_journal();
        let namespace = SourceNamespaceId::from_bytes([9; 16]);
        let owner = SourceOwnerGeneration::from_bytes([10; 32]);
        let domain_ref = OpaqueRef::new("domain:native-security-read-fence").expect("domain");
        let binding = NativeSecurityBinding {
            namespace,
            source_owner: owner,
            domain: domain_ref.clone(),
            dependents: BoundedSet::from_items([
                OpaqueId::new(HANDLE_SECURITY_DEPENDENT).expect("handle dependent"),
            ])
            .expect("required dependent"),
        };
        let state = SecurityPolicyState {
            policy: AccessPolicyRecord {
                namespace_id: namespace,
                owner_generation: owner,
                policy_revision: AccessPolicyRevision::new(11),
                live_deny_generation: 7,
                shadow_fence_revision: ShadowFenceRevision::new(6),
                purge_fence_revision: PurgeFenceRevision::new(6),
                policy_digest: Blake3Digest32::from_bytes([11; 32]),
            },
            security_domain_ref: domain_ref,
            snapshot_digest: Blake3Digest32::from_bytes([12; 32]),
            denied_memberships: BoundedSet::empty(),
            purged_memberships: BoundedSet::empty(),
            fail_closed: false,
        };
        let read = journal
            .read_security_restriction(namespace, &context())
            .expect("actual C02 readback");
        let mutation = read
            .initialize(
                OpaqueId::new("native-security-read-fence-init").expect("operation"),
                Blake3Digest32::from_bytes([13; 32]),
                state.clone(),
                binding.dependents.clone(),
            )
            .expect("explicit initial state");
        let committed = journal
            .commit_security_restriction(&mutation, &context())
            .expect("actual C02 commit");
        let expected = committed.current_conditions().expect("C02 head conditions");

        let mut publisher = ControlSnapshotPublisher::new();
        let mut handles = HandleStore::new(HandlePolicy::BASELINE).expect("handle owner");
        let mut dependents =
            SecurityInvalidationRegistry::new(&binding).expect("invalidation registry");
        dependents.register_handles(&mut handles).expect("actual handle invalidator");
        let domain = NativeSecurityDomain::restore(
            binding.clone(),
            &mut journal,
            &mut publisher,
            &mut dependents,
            &context(),
        )
        .expect("restored native domain");

        Fixture {
            domain,
            binding,
            state,
            expected,
            journal,
            publisher,
            _scratch: scratch,
        }
    }

    fn fence(state: &SecurityPolicyState) -> RequestSecurityFence {
        RequestSecurityFence {
            planned_generation: state.policy.live_deny_generation,
            memberships: BTreeSet::new(),
        }
    }

    fn pending_command(fixture: &Fixture) -> (SecurityRestriction, AccessPolicyRecord) {
        let previous = &fixture.state;
        let mut policy = previous.policy;
        policy.policy_revision = AccessPolicyRevision::new(12);
        policy.live_deny_generation = 8;
        policy.policy_digest = Blake3Digest32::from_bytes([14; 32]);
        let expected_live = LiveSecurityState {
            generation: previous.policy.live_deny_generation,
            denied_memberships: BTreeSet::new(),
            purged_memberships: BTreeSet::new(),
            fail_closed: previous.fail_closed,
            snapshot_digest: previous.snapshot_digest,
        };
        let next_live = LiveSecurityState {
            generation: policy.live_deny_generation,
            denied_memberships: BTreeSet::from([SourceMembershipId::from_bytes([16; 16])]),
            purged_memberships: BTreeSet::new(),
            fail_closed: false,
            snapshot_digest: Blake3Digest32::from_bytes([15; 32]),
        };
        (
            SecurityRestriction {
                operation_id: OpaqueId::new("native-security-read-fence-pending")
                    .expect("operation"),
                security_domain_ref: fixture.binding.domain.clone(),
                expected_policy_revision: previous.policy.policy_revision,
                policy_revision: policy.policy_revision,
                expected_live,
                next_live,
                required_dependents: fixture.binding.dependents.clone(),
            },
            policy,
        )
    }

    #[test]
    fn exact_conditions_require_matching_binding_live_head_and_completed_barrier() {
        let mut fixture = fixture();
        let request = fence(&fixture.state);
        let writes = fixture.journal.committed_writes();
        let actual = fixture
            .domain
            .retained_source_read_conditions(
                fixture.binding.namespace,
                fixture.binding.source_owner,
                &fixture.journal,
                &request,
            )
            .expect("current bound source conditions");
        assert_eq!(actual, fixture.expected);
        assert_eq!(fixture.journal.committed_writes(), writes);

        assert!(fixture
            .domain
            .retained_source_read_conditions(
                SourceNamespaceId::from_bytes([99; 16]),
                fixture.binding.source_owner,
                &fixture.journal,
                &request,
            )
            .is_err());
        assert!(fixture
            .domain
            .retained_source_read_conditions(
                fixture.binding.namespace,
                SourceOwnerGeneration::from_bytes([99; 32]),
                &fixture.journal,
                &request,
            )
            .is_err());

        let (command, policy) = pending_command(&fixture);
        let mut fail_dependent = |
            _: &search_control_redb::security_restriction::SecurityRestrictionCommit,
            _: &ReceiptRef,
            _: &search_contracts::LiveDenySnapshotRef,
            _: &OperationContext<FakeCancellation>,
        | -> Result<BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>, AccessError> {
            Err(AccessError::SecurityFailClosed)
        };
        assert!(fixture
            .domain
            .apply(
                command,
                policy,
                &mut fixture.journal,
                &mut fixture.publisher,
                &mut fail_dependent,
                &context(),
            )
            .is_err());
        assert_eq!(fixture.domain.phase(), SecurityMutationPhase::FailClosed);
        let writes = fixture.journal.committed_writes();
        let refused = fixture.domain.retained_source_read_conditions(
            fixture.binding.namespace,
            fixture.binding.source_owner,
            &fixture.journal,
            &request,
        );
        assert_eq!(
            refused,
            Err(NativeSecurityError::Access(AccessError::SecurityFailClosed)),
        );
        assert_eq!(fixture.journal.committed_writes(), writes);
    }

}
