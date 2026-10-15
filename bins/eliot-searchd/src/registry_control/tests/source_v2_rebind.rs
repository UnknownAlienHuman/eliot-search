//! Metadata/real-redb proof for re-admitting one physical Source after a rename.
use super::*;

#[test]
fn source_v2_rebinding_retains_both_originals_and_restarts_without_new_identity() {
    let scratch = Scratch::new();
    let context = context();
    let (platform, capabilities) = platform();
    let profile = profile();
    let policy = policy(RuleEffect::Allow, 7);
    let first_original = original(&policy, "src/first.rs", 71);
    let next_original = original(&policy, "src/renamed.rs", 81);
    let work = |_unit: MigrationBootstrapWork| Ok::<_, OwnerError>(());
    let mut journal = super::super::journal(&scratch, &context);
    let mut publisher = search_control_redb::ControlSnapshotPublisher::new();
    let mut registry =
        SourceRegistry::new(search_source_registry::api::DEFAULT_REGISTRY_LIMITS).unwrap();
    let root = RootBindingId::from_bytes([13; 16]);
    let first_request = request(
        first_original.clone(),
        root,
        profile,
        1,
        SourceNamespaceSelectionV2::CreateNew,
        "src/first.rs",
        71,
        "rebind-first",
    );
    let identity;
    let first_bytes;
    {
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            &work,
            platform.clone(),
            capabilities.clone(),
        );
        register_source_root(&mut registry, &mut port, &context, &first_original);
        let mut ids = CounterIds::new(11);
        let first = prepared(&mut registry, &first_request, &mut ids, &mut port, &context);
        identity =
            commit_source_admission_v2(&mut registry, &first, &mut port, &context, &mut || Ok(()))
                .unwrap()
                .source_identity;
        first_bytes = first.batch().encoded().to_vec();
        let namespace = registry.namespaces().values().next().unwrap();
        let selection = SourceNamespaceSelectionV2::Existing {
            id: namespace.ownership.source_namespace_id,
            revision: namespace.ownership.ownership_record_revision,
            generation: namespace.ownership.source_owner_generation,
        };
        let mut next = request(
            next_original.clone(),
            root,
            profile,
            2,
            selection,
            "src/renamed.rs",
            81,
            "rebind-next",
        );
        let mut observed = next.identity_observation.as_inner().clone();
        observed.stable_evidence = first_request
            .identity_observation
            .as_inner()
            .stable_evidence
            .clone();
        observed.claimed_source_id = Some(identity.source_id);
        next.identity_observation = validate_identity_observation(observed, profile).unwrap();
        next.expected_source_revision = Some(NonZeroRevision::new(1).unwrap());
        let allocations = ids.allocations;
        // An unresolved original-input effect must fence both allocation and dispatch.
        port.original_uncertain = true;
        assert!(prepare(&mut registry, &next, &mut ids, &mut port, &context).is_err());
        assert_eq!(ids.allocations, allocations);
        port.original_uncertain = false;
        let update = prepared(&mut registry, &next, &mut ids, &mut port, &context);
        port.original_uncertain = true;
        let blocked = port.commit_batch(
            update.batch(),
            &context,
            &search_source_registry::error::registry_mutation(&next.operation_id),
        );
        assert!(matches!(blocked, Err(error) if error.kind() == PortErrorKind::OutcomeUnknown));
        port.original_uncertain = false;
        assert!(matches!(
            port.load_batch(&next.operation_id, &context).unwrap(),
            search_source_registry::recovery::RegistryBatchLookup::Absent { .. }
        ));
        let result =
            commit_source_admission_v2(&mut registry, &update, &mut port, &context, &mut || Ok(()))
                .unwrap();
        assert_eq!(result.source_identity, identity);
        assert_eq!(ids.allocations, allocations);
        assert_eq!(registry.sources().len(), 1);
        assert_eq!(
            registry
                .sources()
                .get(&identity)
                .unwrap()
                .source_revision()
                .get(),
            2
        );
    }
    drop(journal);
    let mut journal = search_control_redb::PersistentControlJournal::open(
        scratch.file(false),
        super::super::identity(),
        search_control_redb::JournalLimits::BASELINE,
    )
    .unwrap();
    let port = bind_v4(&mut journal, &mut publisher, &work, platform, capabilities);
    let restored = port
        .restore(&context)
        .expect("updated Source must restart")
        .into_registry();
    let current = restored.sources().get(&identity).unwrap();
    assert_eq!(current.binding().relative_path().as_str(), "src/renamed.rs");
    assert_eq!(
        current
            .permanent_admission()
            .unwrap()
            .original()
            .encoded_source_evidence(),
        next_original.encoded_source_evidence()
    );
    assert_eq!(restored.revision(), 3);
    let search_source_registry::recovery::RegistryBatchLookup::Committed(old) = port
        .load_batch(&first_request.operation_id, &context)
        .unwrap()
    else {
        panic!("original operation missing");
    };
    assert_eq!(old.record().encode().unwrap(), first_bytes);
    old.verify_operation().unwrap();
}
