//! Actual normalized originals through the normal borrowed adapter and real redb.
//! The fixture's journal/native IDs are synthetic: no Native/Source grant claim.

use super::*;
use search_control_redb::MigrationBootstrapWork as Work;
use search_runtime_owner::OwnerError;
use search_source_admission::policy_v2::{
    DEFAULT_COMPILE_LIMITS, POLICY_SCHEMA_VERSION, PolicyInputV2, PredicateV2, RuleInputV2,
    compile_policy_v2,
};
use search_source_admission::{
    AcceptedCapabilities, BASELINE_PROFILE, RuleEffect, SectionPlatform, SourceClass,
    compiled_defaults, validate_section,
};
use search_source_provenance::OriginalAdmissionV2;
use search_source_provenance::{OriginalInputsLayoutV2, RetainedPolicyV2};
use search_source_registry::recovery::OriginalInputResolver as OriginalInputResolverTrait;
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

fn policy() -> (RetainedPolicyV2, SectionPlatform, AcceptedCapabilities) {
    let platform = SectionPlatform {
        os: "windows".into(),
        arch: "x64".into(),
    };
    let capabilities = AcceptedCapabilities {
        profiles: vec![BASELINE_PROFILE.into()],
    };
    let compiled = compile_policy_v2(
        &PolicyInputV2 {
            schema_version: POLICY_SCHEMA_VERSION,
            revision: u64::MAX,
            section: validate_section(&compiled_defaults(), &platform, &capabilities).unwrap(),
            rules: (0..128)
                .map(|index| RuleInputV2 {
                    id: format!("r{index:03}{}", "x".repeat(252)),
                    precedence: u32::MAX - index,
                    source_class: SourceClass::Documentation,
                    predicate: PredicateV2::RelativePathGlobMatches("a".repeat(256)),
                    effect: RuleEffect::Unsupported,
                    unknown_fields: vec![],
                })
                .collect(),
            unknown_fields: vec![],
        },
        DEFAULT_COMPILE_LIMITS,
        &mut || Ok(()),
    )
    .unwrap();
    let retained =
        RetainedPolicyV2::capture(&compiled, &platform, &capabilities, &mut || Ok(())).unwrap();
    (retained, platform, capabilities)
}

#[test]
fn original_inputs_cancelled_context_preserves_reason_without_work_or_write() {
    use search_ports::{ContextReason, PortErrorKind, PortFailure, PortRetryability};
    let scratch = Scratch::new();
    let live = context();
    let mut journal = journal(&scratch, &live);
    let mut publisher = ControlSnapshotPublisher::new();
    let (retained, _, _) = policy();
    let layout = OriginalInputsLayoutV2::policy(&retained, &mut || Ok(())).unwrap();
    let references = [layout.reference()];
    let calls = Cell::new(0_usize);
    let work = |_| {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let port = RegistryJournal::bind_original(
        &mut journal,
        &mut publisher,
        identity(),
        DEFAULT_REGISTRY_LIMITS,
        Instant::now() + Duration::from_secs(60),
        &work,
    )
    .unwrap();
    let generation = port
        .original_inputs_snapshot(&references, &live)
        .unwrap()
        .generation();
    let calls_before = calls.get();
    let cancelled = OperationContext::new(
        RequestId::from_bytes([11; 16]),
        60_000,
        FakeCancellation::new(true),
        OpaqueRef::new("cancelled-original-input-fixture").unwrap(),
    )
    .unwrap();
    let error = port
        .original_inputs_snapshot(&references, &cancelled)
        .err()
        .expect("cancelled read");
    match error {
        super::super::original_inputs::OriginalInputsError::Context(error) => {
            assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
            assert_eq!(error.retryability(), PortRetryability::SameRequest);
            assert_eq!(*error.reason(), ContextReason::Cancelled);
        }
        other => panic!("original cancellation was relabelled: {other:?}"),
    }
    assert_eq!(calls.get(), calls_before, "no callback after cancellation");
    assert!(!port.requires_recovery());
    assert_eq!(
        port
            .original_inputs_snapshot(&references, &live)
            .unwrap()
            .generation(),
        generation
    );
}

#[test]
fn original_inputs_normal_adapter_roundtrips_82255_byte_policy_after_real_restart() {
    let scratch = Scratch::new();
    let ctx = context();
    let mut journal = journal(&scratch, &ctx);
    let mut publisher = ControlSnapshotPublisher::new();
    let (policy, platform, capabilities) = policy();
    assert_eq!(policy.encoded_inputs().len(), 82_255);
    let layout = OriginalInputsLayoutV2::policy(&policy, &mut || Ok(())).unwrap();
    assert_eq!(layout.parts().len(), 6);
    let deadline = Instant::now() + Duration::from_secs(60);
    let remaining = Cell::new(16 * 1024 * 1024_usize);
    let work = |unit| {
        let bytes = match unit {
            Work::Read(n) | Work::Write(n) | Work::Allocate(n) | Work::Process(n) => n,
            _ => 0,
        };
        let next = remaining
            .get()
            .checked_sub(bytes)
            .ok_or(OwnerError::OwnerOperationCapacityExceeded)?;
        remaining.set(next);
        Ok(())
    };
    let mut probe = || {
        if Instant::now() >= deadline {
            return Err(search_source_admission::policy_v2::CheckpointFailureV2::DeadlineExceeded);
        }
        work(Work::Checkpoint)
            .map_err(|_| search_source_admission::policy_v2::CheckpointFailureV2::DeadlineExceeded)
    };
    let (saved_policy_reference, saved_admission_reference, saved_original);
    {
        let mut port = RegistryJournal::bind_original(
            &mut journal,
            &mut publisher,
            identity(),
            DEFAULT_REGISTRY_LIMITS,
            deadline,
            &work,
        )
        .unwrap();
        let prepared = port.prepare_original_inputs(&layout, &ctx).unwrap();
        port.commit_original_inputs(&prepared, &ctx).unwrap();
        assert!(!port.requires_recovery());
        let observed = port
            .original_inputs_snapshot(&[layout.reference()], &ctx)
            .unwrap();
        assert_eq!(observed.identity(), identity());
        let restored = observed
            .policy(layout.reference(), &platform, &capabilities, &mut probe)
            .unwrap();
        assert_eq!(restored.encoded_inputs(), policy.encoded_inputs());
        assert_eq!(restored.fingerprint(), policy.fingerprint());
        let generation = observed.generation();
        let replay = port.prepare_original_inputs(&layout, &ctx).unwrap();
        port.commit_original_inputs(&replay, &ctx).unwrap();
        assert_eq!(
            port
                .original_inputs_snapshot(&[layout.reference()], &ctx)
                .unwrap()
                .generation(),
            generation,
            "retained original is stored once"
        );
        assert_eq!(
            port.restore(&ctx).unwrap().registry().revision(),
            0,
            "metadata preparation is not a Source commit"
        );
        let compiled = compile_policy_v2(
            &PolicyInputV2 {
                schema_version: POLICY_SCHEMA_VERSION,
                revision: 7,
                section: validate_section(&compiled_defaults(), &platform, &capabilities).unwrap(),
                rules: vec![RuleInputV2 {
                    id: "source-allow".into(),
                    precedence: 0,
                    source_class: SourceClass::Regular,
                    predicate: PredicateV2::Always,
                    effect: RuleEffect::Allow,
                    unknown_fields: vec![],
                }],
                unknown_fields: vec![],
            },
            DEFAULT_COMPILE_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        let source_policy =
            RetainedPolicyV2::capture(&compiled, &platform, &capabilities, &mut || Ok(())).unwrap();
        let source_policy_layout =
            OriginalInputsLayoutV2::policy(&source_policy, &mut || Ok(())).unwrap();
        let prepared = port
            .prepare_original_inputs(&source_policy_layout, &ctx)
            .unwrap();
        port.commit_original_inputs(&prepared, &ctx).unwrap();
        let raw = search_source_admission::UnvalidatedObservationInput {
            locator_class: "normalized-file".into(),
            location_class: "local-fixed".into(),
            source_kind: "file".into(),
            source_class: "regular".into(),
            byte_size: Some(8),
            is_generated: Some(false),
            is_vendor: Some(false),
            is_binary: Some(false),
            is_system: Some(false),
            sensitivity: Some("internal".into()),
            detector_id: Some("detector:normal-originals".into()),
            profile_id: Some(BASELINE_PROFILE.into()),
            unavailable_fields: vec![],
            unknown_fields: vec![],
        };
        let observation = search_source_admission::original_inputs_v2::reconstitute_observation_v2(
            &raw,
            Some(b"src/normal.rs"),
            source_policy.compiled(),
            search_source_admission::observation_v2::MAX_OBSERVATION_PREIMAGE_BYTES,
            &mut || Ok(()),
        )
        .unwrap();
        let source_policy = std::sync::Arc::new(source_policy);
        let original = OriginalAdmissionV2::retain(
            std::sync::Arc::clone(&source_policy),
            &observation,
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(
            original.receipt().decision().outcome(),
            search_source_admission::AdmissionOutcome::Allow
        );
        let original_layout = OriginalInputsLayoutV2::admission(&original, &mut || Ok(())).unwrap();
        let prepared = port
            .prepare_original_inputs(&original_layout, &ctx)
            .unwrap();
        port.commit_original_inputs(&prepared, &ctx).unwrap();
        let readback = port
            .original_inputs_snapshot(&[original_layout.reference()], &ctx)
            .unwrap();
        let restored = readback
            .admission(original_layout.reference(), source_policy, &mut probe)
            .unwrap();
        assert_eq!(restored.receipt(), original.receipt());
        assert_eq!(restored.observation(), original.observation());
        assert_eq!(port.restore(&ctx).unwrap().registry().revision(), 0);
        let combined = port
            .original_inputs_snapshot(
                &[source_policy_layout.reference(), original_layout.reference()],
                &ctx,
            )
            .unwrap();
        let mut resolver = combined.v4_resolver(&platform, &capabilities);
        let cached = OriginalInputResolverTrait::resolve_policy(
            &mut resolver,
            source_policy_layout.reference(),
            &mut probe,
        )
        .unwrap();
        let cached_again = OriginalInputResolverTrait::resolve_policy(
            &mut resolver,
            source_policy_layout.reference(),
            &mut probe,
        )
        .unwrap();
        assert!(std::sync::Arc::ptr_eq(&cached, &cached_again));
        let restored_v4 = OriginalInputResolverTrait::resolve_admission(
            &mut resolver,
            original_layout.reference(),
            cached,
            &mut probe,
        )
        .unwrap();
        assert_eq!(restored_v4.receipt(), original.receipt());
        assert_eq!(restored_v4.observation(), original.observation());
        saved_policy_reference = source_policy_layout.reference().to_vec();
        saved_admission_reference = original_layout.reference().to_vec();
        saved_original = original;
    }
    let after_first = remaining.get();
    drop(journal);
    let mut journal =
        PersistentControlJournal::open(scratch.file(false), identity(), JournalLimits::BASELINE)
            .unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    {
        let port = RegistryJournal::bind_original(
            &mut journal,
            &mut publisher,
            identity(),
            DEFAULT_REGISTRY_LIMITS,
            deadline,
            &work,
        )
        .unwrap();
        let observed = port
            .original_inputs_snapshot(&[layout.reference()], &ctx)
            .unwrap();
        let restored = observed
            .policy(layout.reference(), &platform, &capabilities, &mut probe)
            .unwrap();
        assert_eq!(restored.encoded_inputs(), policy.encoded_inputs());
        assert_eq!(restored.fingerprint(), policy.fingerprint());
        let source_observed = port
            .original_inputs_snapshot(&[saved_policy_reference.as_slice()], &ctx)
            .unwrap();
        let source_policy = source_observed
            .policy(
                &saved_policy_reference,
                &platform,
                &capabilities,
                &mut probe,
            )
            .unwrap();
        let admission_observed = port
            .original_inputs_snapshot(&[saved_admission_reference.as_slice()], &ctx)
            .unwrap();
        let restored_original = admission_observed
            .admission(
                &saved_admission_reference,
                std::sync::Arc::new(source_policy),
                &mut probe,
            )
            .unwrap();
        assert_eq!(
            restored_original, saved_original,
            "the complete observation/decision/receipt reproduces after actual DB reopen"
        );
        assert!(
            remaining.get() < after_first,
            "the same original allowance reaches restart readback"
        );
    }
}

#[test]
fn original_inputs_normal_adapter_retains_exact_pending_command_and_owner_cause() {
    use super::super::original_inputs::OriginalInputsError;
    let scratch = Scratch::new();
    let ctx = context();
    let mut journal = journal(&scratch, &ctx);
    let mut publisher = ControlSnapshotPublisher::new();
    let (policy, _, _) = policy();
    let layout = OriginalInputsLayoutV2::policy(&policy, &mut || Ok(())).unwrap();
    let fail_write = Cell::new(true);
    let work = |unit| {
        if fail_write.get() && matches!(unit, Work::Write(n) if n>0) {
            Err(OwnerError::OwnerCancelledBeforeMutation)
        } else {
            Ok(())
        }
    };
    {
        let mut port = RegistryJournal::bind_original(
            &mut journal,
            &mut publisher,
            identity(),
            DEFAULT_REGISTRY_LIMITS,
            Instant::now() + Duration::from_secs(60),
            &work,
        )
        .unwrap();
        let prepared = port.prepare_original_inputs(&layout, &ctx).unwrap();
        let OriginalInputsError::OriginalControl(error) =
            port.commit_original_inputs(&prepared, &ctx).unwrap_err()
        else {
            panic!("original control cause")
        };
        assert_eq!(error.control, ControlError::CommitOutcomeUnknown);
        assert_eq!(
            error.first_context,
            Some(OwnerError::OwnerCancelledBeforeMutation)
        );
        assert!(port.requires_recovery());
        assert!(
            port.commit_original_inputs(&prepared, &ctx).is_err(),
            "unknown is never blindly replayed"
        );
        fail_write.set(false);
        assert_eq!(
            port.recover_original_inputs(&prepared, &ctx).unwrap(),
            search_control_redb::CommitRecoveryDecision::NotCommittedRetrySameOperation
        );
        assert!(!port.requires_recovery());
        port.commit_original_inputs(&prepared, &ctx).unwrap();
        assert!(
            port
                .original_inputs_snapshot(&[layout.reference()], &ctx)
                .is_ok()
        );
    }
}
