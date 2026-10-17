//! Real C02/C03 replay proof. Native, C07 link, domains and UUID inputs are
//! synthetic. No OS entropy, credential, Source locator or Qdrant is touched.
use super::*;
use crate::registry_control::{
    native_member_setup::{NativeMemberProfiles, NativeMemberSetupInput, SelectedNativeMember},
    source_scope::RootSourceScope,
};
use search_contracts as c;
use search_control_redb::{
    self as db, ControlRecordClass as Class, ControlRecordCondition as Condition,
};

fn uuid(seed: u8) -> [u8; 16] {
    let mut id = [seed; 16];
    id[6] = 0x40;
    id[8] = 0x80;
    id
}
fn profiles() -> (
    search_materializer::api::ValidatedMaterializerProfile,
    search_unitizer::ValidatedV3UnitizerProfile,
    search_lexical::AcceptedCodeIdentifiersProfile,
) {
    use search_materializer::api::{baseline_profile_descriptor, validate_materializer_profile};
    use search_unitizer::*;
    let materializer =
        validate_materializer_profile(&baseline_profile_descriptor("member-proof-code", 1))
            .unwrap();
    let unitizer = validate_v3_unitizer_profile(&V3UnitizerProfileDescriptor {
        profile_name: c::ProfileId::new("member-proof-code").unwrap(),
        profile_revision: 1,
        representation_kind: V3RepresentationKind::Code,
        unit_kind: c::UnitKind::File,
        boundary_revision: 1,
        overlap_policy: V3OverlapPolicy::None,
        anchor_policy: V3AnchorPolicy::ExactTextBytes,
        attachment_policy: V3AttachmentPolicy::Absent,
        omission_policy: V3OmissionPolicy::Forbidden,
        empty_policy: V3EmptyPolicy::Reject,
        limits: UnitizationLimits {
            max_input_bytes: 4096,
            preferred_unit_bytes: 64,
            max_unit_bytes: 128,
            max_lines: 512,
            max_units: 128,
        },
        min_unit_bytes: 1,
        max_unit_scalars: 128,
        max_unit_lines: 128,
        max_anchor_depth: 1,
        max_manifest_bytes: 65536,
        max_steps: 65536,
        fixture_digest: c::Blake3Digest32::from_bytes([7; 32]),
    })
    .unwrap();
    let lexical = search_lexical::CodeIdentifiersProfile::new().unwrap();
    let qualification = search_lexical::SparseQualification {
        profile_id: lexical.id().clone(),
        profile_revision: lexical.revision(),
        profile_fingerprint: lexical.fingerprint(),
        provider_artifact_digest: lexical.artifact_set_identity(),
        compatibility_fixture_digest: lexical.compatibility_fixture_identity(),
        collision_fixture_digest: lexical.collision_fixture_identity(),
        accepted: true,
        qualification_receipt: c::ReceiptRef::new("fixture-qualification-not-native-evidence")
            .unwrap(),
    };
    let lexical = search_lexical::accept_code_identifiers_profile(lexical, qualification).unwrap();
    (materializer, unitizer, lexical)
}

#[test]
fn native_member_scope_redb_recovers_prebind_assignment_and_preserves_restriction() {
    let scratch = Scratch::new();
    let context = context();
    let (platform, capabilities) = platform();
    let filesystem_profile = profile();
    let policy = policy(RuleEffect::Allow, 7);
    let original = original(&policy, "src/member.rs", 71);
    let work = |_| Ok::<_, OwnerError>(());
    let mut journal = super::super::journal(&scratch, &context);
    let mut publisher = db::ControlSnapshotPublisher::new();
    let mut registry =
        SourceRegistry::new(search_source_registry::api::DEFAULT_REGISTRY_LIMITS).unwrap();
    let (materializer, unitizer, lexical) = profiles();
    let profiles = NativeMemberProfiles {
        materializer: &materializer,
        unitizer: &unitizer,
        lexical: &lexical,
    };
    let domains = search_revision_store::DomainKind::ALL.map(|kind| {
        let seed = match kind {
            search_revision_store::DomainKind::Scope => 30,
            search_revision_store::DomainKind::Access => 31,
            search_revision_store::DomainKind::Confidentiality => 32,
            search_revision_store::DomainKind::EncryptionKey => 33,
            search_revision_store::DomainKind::Retention => 34,
            search_revision_store::DomainKind::Erasure => 35,
        };
        let body = c::to_canonical_cbor(&c::CanonicalValue::U64(u64::from(seed))).unwrap();
        search_revision_store::DomainBinding::new(kind, uuid(seed), body.into_vec()).unwrap()
    });
    let operation = c::OpaqueId::new("fixture-original-retention-operation").unwrap();
    let scope_ids = RootSourceScope {
        workspace_id: c::WorkspaceId::from_bytes(uuid(25)),
        corpus_id: c::CorpusId::from_bytes(uuid(26)),
    };
    let assignment = || {
        (
            c::SourceMembershipId::from_bytes(uuid(41)),
            c::AccessPolicyBindingId::from_bytes(uuid(42)),
            c::ResidencyPolicyBindingId::from_bytes(uuid(43)),
        )
    };
    let limits = db::JournalLimits::BASELINE;
    let current_key = db::ControlKey::new(b"fixture/member-current-key".to_vec(), limits).unwrap();
    let current_value = db::ControlValue::new(
        Class::State,
        b"fixture-current-key-metadata".to_vec(),
        limits,
    )
    .unwrap();
    let current = Condition::exact(current_key.clone(), current_value.clone());
    let link_key = db::ControlKey::new(b"fixture/member-c07-link".to_vec(), limits).unwrap();
    let link_value = db::ControlValue::new(
        Class::State,
        b"fixture-verified-source-link".to_vec(),
        limits,
    )
    .unwrap();
    let mut source_keys = vec![link_key.clone()];
    {
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            &work,
            platform.clone(),
            capabilities.clone(),
        );
        register_source_root(&mut registry, &mut port, &context, &original);
        let request = request(
            original,
            RootBindingId::from_bytes([13; 16]),
            filesystem_profile,
            registry.revision(),
            SourceNamespaceSelectionV2::CreateNew,
            "src/member.rs",
            71,
            "fixture-member-source",
        );
        let source = prepared(
            &mut registry,
            &request,
            &mut CounterIds::new(1),
            &mut port,
            &context,
        );
        commit_source_admission_v2(&mut registry, &source, &mut port, &context, &mut || Ok(()))
            .unwrap();
        let root = registry.root(request.root_binding_id).unwrap().clone();
        let scope = port
            .scope_test_assignment(&registry, &root, &context, scope_ids)
            .unwrap();
        let namespace = registry.namespaces().values().next().unwrap().clone();
        let source = registry.sources().values().next().unwrap().clone();
        let selected = SelectedNativeMember::from_owners(
            &profiles,
            &source,
            &namespace,
            b"fixture-current-security-policy-semantics",
            &domains,
        )
        .unwrap();
        for (family, record) in [
            (
                db::RegistryStateFamily::Root,
                search_source_registry::recovery::RegistryStateRecord::Root(root.clone()),
            ),
            (
                db::RegistryStateFamily::Namespace,
                search_source_registry::recovery::RegistryStateRecord::Namespace(namespace.clone()),
            ),
            (
                db::RegistryStateFamily::Source,
                search_source_registry::recovery::RegistryStateRecord::Source(source.clone()),
            ),
        ] {
            let key =
                db::RegistryStateKey::new(family, record.key().encode().unwrap(), limits).unwrap();
            let encoded = if family == db::RegistryStateFamily::Root {
                record.encode().unwrap()
            } else {
                record
                    .encode_with_profile("eliot.registry-state.v4")
                    .unwrap()
            };
            source_keys.push(
                key.current_condition(&encoded, limits)
                    .unwrap()
                    .key()
                    .clone(),
            );
        }
        let read = port
            .journal
            .read_control_keys_with_original_context(
                &[link_key.clone(), current_key.clone()],
                Instant::now() + Duration::from_secs(60),
                &work,
            )
            .unwrap();
        let id = db::MutationId([92; 32]);
        let command = db::ConditionalControlMutation::new(
            db::ControlMutation::new(
                id,
                c::Blake3Digest32::from_bytes([93; 32]),
                read.generation(),
                vec![
                    db::ControlWrite {
                        key: link_key,
                        value: link_value,
                    },
                    db::ControlWrite {
                        key: current_key,
                        value: current_value,
                    },
                ],
                vec![],
            ),
            vec![],
        );
        port.journal
            .transact_records_with_original_context(
                &command,
                &db::control_mutation_identity(id).unwrap(),
                Instant::now() + Duration::from_secs(60),
                &work,
            )
            .unwrap();
        let source_read = port
            .journal
            .read_control_keys_with_original_context(
                &source_keys,
                Instant::now() + Duration::from_secs(60),
                &work,
            )
            .unwrap();
        let input = NativeMemberSetupInput {
            root: &root,
            namespace: &namespace,
            source: &source,
            scope,
            selected: &selected,
            operation: &operation,
            source_read: &source_read,
            selected_current: current.clone(),
        };
        let checkpoints = Cell::new(0);
        assert!(
            port.native_member_test_assignment(
                &mut registry,
                &input,
                &context,
                &mut || {
                    checkpoints.set(checkpoints.get() + 1);
                    if checkpoints.get() == 4 {
                        Err(OwnerError::OwnerOperationConflict)
                    } else {
                        Ok(())
                    }
                },
                assignment
            )
            .is_err()
        );
        assert_eq!(
            checkpoints.get(),
            4,
            "assignment committed before refused C03 dispatch"
        );
        assert!(registry.memberships().is_empty());
    }
    drop(journal);
    let mut journal =
        db::PersistentControlJournal::open(scratch.file(false), super::super::identity(), limits)
            .unwrap();
    let mut port = bind_v4(&mut journal, &mut publisher, &work, platform, capabilities);
    let mut registry = port.restore(&context).unwrap().into_registry();
    let root = registry.roots().values().next().unwrap().clone();
    let namespace = registry.namespaces().values().next().unwrap().clone();
    let source = registry.sources().values().next().unwrap().clone();
    let scope = port
        .scope_test_assignment(&registry, &root, &context, scope_ids)
        .unwrap();
    let selected = SelectedNativeMember::from_owners(
        &profiles,
        &source,
        &namespace,
        b"fixture-current-security-policy-semantics",
        &domains,
    )
    .unwrap();
    let read = port
        .journal
        .read_control_keys_with_original_context(
            &source_keys,
            Instant::now() + Duration::from_secs(60),
            &work,
        )
        .unwrap();
    let input = NativeMemberSetupInput {
        root: &root,
        namespace: &namespace,
        source: &source,
        scope,
        selected: &selected,
        operation: &operation,
        source_read: &read,
        selected_current: current.clone(),
    };
    let receipt = port
        .native_member_test_assignment(&mut registry, &input, &context, &mut || Ok(()), || {
            panic!("reopen must recover assigned UUIDs before RNG")
        })
        .unwrap();
    assert_eq!(receipt.member.source_membership_id, assignment().0);
    assert_eq!(receipt.member.access_policy_binding_id, assignment().1);
    assert_eq!(receipt.member.residency_policy_binding_id, assignment().2);
    assert_eq!(receipt.member.workspace_id, scope.workspace_id);
    assert_eq!(
        &receipt.member.preparation_profile_id,
        selected.preparation_profile_id()
    );
    assert_eq!(
        &receipt.member.retention_policy_id,
        selected.retention_policy_id()
    );
    search_source_registry::api::transition_source_membership_v2(
        &mut registry,
        receipt.member.source_membership_id,
        receipt.member.membership_revision,
        search_source_registry::membership::MembershipCommand::Restrict,
        &c::OpaqueId::new("fixture-restrict-member").unwrap(),
        c::Blake3Digest32::from_bytes([95; 32]),
        &c::ReceiptRef::new("fixture-member-restricted").unwrap(),
        &mut port,
        &context,
    )
    .unwrap();
    let read = port
        .journal
        .read_control_keys_with_original_context(
            &source_keys,
            Instant::now() + Duration::from_secs(60),
            &work,
        )
        .unwrap();
    let input = NativeMemberSetupInput {
        source_read: &read,
        ..input
    };
    assert!(
        port.native_member_test_assignment(
            &mut registry,
            &input,
            &context,
            &mut || Ok(()),
            || panic!("restricted original must never redraw")
        )
        .is_err()
    );
    assert!(
        search_source_registry::api::active_source_membership_v2(
            &registry,
            receipt.member.source_membership_id
        )
        .is_err()
    );
    let restored = port.restore(&context).unwrap().into_registry();
    assert!(
        search_source_registry::api::active_source_membership_v2(
            &restored,
            receipt.member.source_membership_id
        )
        .is_err()
    );
}
