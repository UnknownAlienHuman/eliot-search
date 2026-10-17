use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use search_contracts::{
    Blake3Digest32, ObjectResidencyKeyDigest, OpaqueId, OpaqueRef, RequestId, SourceId,
    SourceNamespaceId, SourceRevisionId, SourceRevisionRef,
};
use search_ports::{
    CancellationProbe, ContextReason, DisclosureClass, OperationContext, PackageOpaque, Port,
    PortError, PortErrorKind, PortRetryability,
};

use super::*;
use crate::sweep::{
    CasAdmin, CasAdminError, CasDeleteAck, CasReadback, DurableRoot, RetentionRootKind,
    SweepLimits, begin_sweep, collect_protection, execute_sweep_batch, mark_reachable, plan_sweep,
};
use crate::{RetentionError, RetentionOperation};

#[derive(Debug)]
struct TestCancellation;

impl PackageOpaque for TestCancellation {
    fn owner_package(&self) -> &'static str {
        "search-retention"
    }
}

impl CancellationProbe for TestCancellation {
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct TestFencePort {
    observations: VecDeque<RootFenceObservation>,
}

impl Port for TestFencePort {
    type Error = PortError<ContextReason>;
    type Cancellation = TestCancellation;
}

impl RootFenceObservationPort for TestFencePort {
    fn observe_root_fence(
        &mut self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<RootFenceObservation, Self::Error> {
        context.preflight()?;
        self.observations.pop_front().ok_or_else(|| {
            PortError::new(
                PortErrorKind::DependencyUnavailable,
                PortRetryability::Never,
                DisclosureClass::Private,
                ContextReason::Cancelled,
                None,
            )
        })
    }
}

fn context() -> OperationContext<TestCancellation> {
    OperationContext::new(
        RequestId::from_bytes([0x31; 16]),
        60_000,
        TestCancellation,
        OpaqueRef::new("source-pin-canary-budget").expect("test budget ref"),
    )
    .expect("test context")
}

const fn limits() -> SweepLimits {
    SweepLimits {
        max_roots: 8,
        max_objects: 16,
        max_edges_per_object: 8,
        max_batches: 8,
        max_batch_objects: 4,
    }
}

fn target(object: &str, source: u8, revision: u8) -> SourcePinTarget {
    SourcePinTarget::new(
        OpaqueId::new(object).expect("object id"),
        SourceRevisionRef {
            source_namespace_id: SourceNamespaceId::from_bytes([source; 16]),
            source_id: SourceId::from_bytes([source; 16]),
            revision_id: SourceRevisionId::from_bytes([revision; 16]),
            content_digest: Blake3Digest32::from_bytes([revision; 32]),
            byte_length: 17,
        },
        ObjectResidencyKeyDigest::from_bytes([source; 32]),
    )
}

fn fence(
    control_generation: u64,
    publication_generation: u64,
    capture_time_ms: u64,
    ids: &[&str],
) -> RootFenceObservation {
    RootFenceObservation::new(
        control_generation,
        publication_generation,
        capture_time_ms,
        ids.iter()
            .map(|id| OpaqueId::new(*id).expect("pin id"))
            .collect(),
    )
}

fn fence_port(before: RootFenceObservation, after: RootFenceObservation) -> TestFencePort {
    TestFencePort {
        observations: VecDeque::from([before, after]),
    }
}

fn captured_sweep(
    registry: &SourcePinRegistry,
) -> (crate::sweep::ProtectionSet, crate::sweep::SweepPlan) {
    let evidence = registry
        .capture_pin_evidence(
            &mut fence_port(fence(7, 19, 1_000, &[]), fence(7, 19, 1_001, &[])),
            &context(),
        )
        .expect("capture exact pins");
    let root_id = OpaqueId::new("cas:a-root").expect("root id");
    let candidate_id = OpaqueId::new("cas:z-candidate").expect("candidate id");
    let protection = collect_protection(
        vec![DurableRoot {
            object_id: root_id.clone(),
            kind: RetentionRootKind::RetentionPolicy,
            residency_digest: ObjectResidencyKeyDigest::from_bytes([0x41; 32]),
            control_generation: 7,
        }],
        &evidence,
        &BTreeSet::new(),
        &BTreeSet::new(),
        limits(),
    )
    .expect("collect protection");
    let intent = begin_sweep(
        RetentionOperation {
            operation_id: OpaqueId::new("sweep:source-pin-fence").expect("operation id"),
            request_digest: Blake3Digest32::from_bytes([0x42; 32]),
        },
        &protection,
    )
    .expect("begin sweep");
    let inventory = vec![root_id.clone(), candidate_id];
    let graph = BTreeMap::from([(root_id, Vec::new())]);
    let mark = mark_reachable(
        &intent,
        &protection,
        &graph,
        &inventory.iter().cloned().collect(),
        limits(),
    )
    .expect("mark root");
    let plan = plan_sweep(&intent, &mark, &inventory, 1, &protection, limits())
        .expect("plan exact candidate");
    (protection, plan)
}

struct PinDuringDelete<'a> {
    registry: &'a SourcePinRegistry,
    target: SourcePinTarget,
    deadline: Instant,
    live: BTreeSet<OpaqueId>,
    attempted_pin: Option<Result<(), RetentionError>>,
    delete_called: bool,
    delete_error: Option<CasAdminError>,
}

impl CasAdmin for PinDuringDelete<'_> {
    fn delete_exact(
        &mut self,
        batch: &crate::sweep::SweepBatch,
        mutation: &crate::sweep::CasMutation,
    ) -> Result<CasDeleteAck, CasAdminError> {
        self.delete_called = true;
        self.attempted_pin = Some(
            match self.registry.pin(self.target.clone(), self.deadline) {
                Ok(guard) => {
                    drop(guard);
                    Ok(())
                }
                Err(error) => Err(error),
            },
        );
        if mutation.operation_id != batch.operation_id {
            return Err(CasAdminError::Conflict);
        }
        if let Some(error) = self.delete_error {
            return Err(error);
        }
        for id in &batch.object_ids {
            self.live.remove(id);
        }
        Ok(CasDeleteAck {
            operation_id: batch.operation_id.clone(),
            deleted_ids: batch.object_ids.clone(),
            replayed: false,
        })
    }

    fn readback_exact(&self, ids: &[OpaqueId]) -> Result<CasReadback, CasAdminError> {
        Ok(CasReadback {
            missing_ids: ids
                .iter()
                .filter(|id| !self.live.contains(*id))
                .cloned()
                .collect(),
            unexpected_ids: Vec::new(),
        })
    }
}

#[test]
fn source_pin_guard_drop_expiry_revoke_and_snapshot_fences_fail_closed() {
    let registry = SourcePinRegistry::new(limits()).expect("registry");
    let source_a = target("cas:source-a", 0x11, 0x21);
    let source_b = target("cas:source-b", 0x12, 0x22);
    let deadline = Instant::now() + Duration::from_secs(60);
    let guard_a = registry
        .pin(source_a.clone(), deadline)
        .expect("pin source A");
    let guard_b = registry
        .pin(source_b.clone(), deadline)
        .expect("pin source B");
    assert_eq!(guard_a.target(), &source_a);
    assert_eq!(guard_a.monotonic_deadline(), deadline);
    guard_a.validate_active().expect("pin is live");
    assert_eq!(
        validate_guard_state(
            &registry.inner.lock().expect("registry lock"),
            1,
            &source_a,
            deadline,
            deadline,
        ),
        Err(RetentionError::SourcePinInactive),
        "the exact deadline is expired without a wall clock"
    );

    let before = fence(7, 19, 1_000, &["cas:other-owner"]);
    let after = fence(7, 19, 1_001, &["cas:other-owner"]);
    let evidence = registry
        .capture_pin_evidence(&mut fence_port(before, after), &context())
        .expect("coherent pin evidence");
    let protection = collect_protection(
        vec![DurableRoot {
            object_id: OpaqueId::new("cas:durable-root").expect("root id"),
            kind: RetentionRootKind::RetentionPolicy,
            residency_digest: ObjectResidencyKeyDigest::from_bytes([0x13; 32]),
            control_generation: 7,
        }],
        &evidence,
        &BTreeSet::new(),
        &BTreeSet::new(),
        limits(),
    )
    .expect("S28 collector accepts captured evidence");
    assert!(protection.pinned_ids.contains(guard_a.target().object_id()));
    assert!(protection.pinned_ids.contains(guard_b.target().object_id()));
    assert!(protection.pinned_ids.contains(&OpaqueId::new("cas:other-owner").unwrap()));
    assert_eq!(protection.control_generation, 7);
    assert_eq!(protection.publication_generation, 19);

    drop(guard_a);
    assert_eq!(
        collect_protection(
            vec![DurableRoot {
                object_id: OpaqueId::new("cas:durable-root").unwrap(),
                kind: RetentionRootKind::RetentionPolicy,
                residency_digest: ObjectResidencyKeyDigest::from_bytes([0x13; 32]),
                control_generation: 7,
            }],
            &evidence,
            &BTreeSet::new(),
            &BTreeSet::new(),
            limits(),
        ),
        Err(RetentionError::RootGenerationChanged),
        "dropping one guard invalidates stale evidence without releasing another pin"
    );

    let after_drop = registry
        .capture_pin_evidence(
            &mut fence_port(
                fence(7, 19, 1_002, &[]),
                fence(7, 19, 1_003, &[]),
            ),
            &context(),
        )
        .expect("fresh evidence after exact drop");
    assert!(!after_drop.pinned_ids().contains(source_a.object_id()));
    assert!(after_drop.pinned_ids().contains(source_b.object_id()));

    registry
        .revoke_source_revision(&source_b.source_revision_ref())
        .expect("revoke exact revision before collection");
    assert_eq!(guard_b.validate_active(), Err(RetentionError::SourcePinInactive));
    let after_revoke = registry
        .capture_pin_evidence(
            &mut fence_port(fence(7, 19, 1_004, &[]), fence(7, 19, 1_005, &[])),
            &context(),
        )
        .expect("fresh evidence after revoke");
    assert!(!after_revoke.pinned_ids().contains(source_b.object_id()));

    assert!(matches!(
        registry.capture_pin_evidence(
            &mut fence_port(fence(7, 19, 1_006, &[]), fence(8, 19, 1_007, &[])),
            &context(),
        ),
        Err(RetentionError::RootGenerationChanged)
    ), "control drift across the original-context Root observations fails closed");

    let overflow = SourcePinRegistry::new(limits()).expect("overflow registry");
    overflow
        .inner
        .lock()
        .expect("registry lock")
        .pin_generation = u64::MAX;
    assert!(matches!(
        overflow.pin(target("cas:overflow", 0x14, 0x24), deadline),
        Err(RetentionError::ContractExhausted)
    ));
    assert!(matches!(overflow.snapshot(), Err(RetentionError::PinProtectionUnknown)));

    let expiry_registry = SourcePinRegistry::new(limits()).expect("expiry registry");
    let expiry_deadline = Instant::now() + Duration::from_secs(60);
    let expired_guard = expiry_registry
        .pin(target("cas:expired", 0x15, 0x25), expiry_deadline)
        .expect("expiry pin");
    {
        let mut state = expiry_registry.inner.lock().expect("registry lock");
        prune_expired(&mut state, expiry_deadline).expect("monotonic expiration");
    }
    assert_eq!(
        expired_guard.validate_active(),
        Err(RetentionError::SourcePinInactive)
    );
    assert!(expiry_registry.snapshot().expect("post-expiry snapshot").pinned_ids.is_empty());
}

#[test]
fn sweep_admission_excludes_pins_from_capture_through_delete_readback() {
    let before_registry = SourcePinRegistry::new(limits()).expect("pre-delete registry");
    let (before_protection, before_plan) = captured_sweep(&before_registry);
    let candidate = target("cas:z-candidate", 0x51, 0x61);
    let late_guard = before_registry
        .pin(candidate.clone(), Instant::now() + Duration::from_secs(60))
        .expect("late pin admitted before sweep barrier");
    let batch = &before_plan.batches[0];
    let mutation = crate::sweep::CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0; 32],
    };
    let mut before_delete = PinDuringDelete {
        registry: &before_registry,
        target: candidate.clone(),
        deadline: Instant::now() + Duration::from_secs(60),
        live: [
            OpaqueId::new("cas:a-root").expect("root id"),
            candidate.object_id().clone(),
        ]
        .into_iter()
        .collect(),
        attempted_pin: None,
        delete_called: false,
        delete_error: None,
    };
    assert_eq!(
        execute_sweep_batch(
            &before_plan,
            0,
            &mut before_delete,
            &mutation,
            &before_protection,
        )
        .expect_err("pin generation changed after capture"),
        RetentionError::RootGenerationChanged
    );
    assert!(!before_delete.delete_called, "stale batch never dispatches");
    drop(late_guard);

    let during_registry = SourcePinRegistry::new(limits()).expect("callback registry");
    let (during_protection, during_plan) = captured_sweep(&during_registry);
    let candidate = target("cas:z-candidate", 0x52, 0x62);
    let deadline = Instant::now() + Duration::from_secs(60);
    let batch = &during_plan.batches[0];
    let mutation = crate::sweep::CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0; 32],
    };
    let mut during_delete = PinDuringDelete {
        registry: &during_registry,
        target: candidate.clone(),
        deadline,
        live: [
            OpaqueId::new("cas:a-root").expect("root id"),
            candidate.object_id().clone(),
        ]
        .into_iter()
        .collect(),
        attempted_pin: None,
        delete_called: false,
        delete_error: None,
    };
    let receipt = execute_sweep_batch(
        &during_plan,
        0,
        &mut during_delete,
        &mutation,
        &during_protection,
    )
    .expect("batch keeps admission closed through readback");
    assert!(during_delete.delete_called);
    assert_eq!(
        during_delete.attempted_pin,
        Some(Err(RetentionError::SourcePinAdmissionClosed))
    );
    assert_eq!(receipt.missing_ids, vec![candidate.object_id().clone()]);
    let after_batch = during_registry
        .pin(candidate, deadline)
        .expect("RAII sweep admission releases on return");
    drop(after_batch);

    let error_registry = SourcePinRegistry::new(limits()).expect("error-path registry");
    let (error_protection, error_plan) = captured_sweep(&error_registry);
    let candidate = target("cas:z-candidate", 0x53, 0x63);
    let deadline = Instant::now() + Duration::from_secs(60);
    let batch = &error_plan.batches[0];
    let mutation = crate::sweep::CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0; 32],
    };
    let mut failed_delete = PinDuringDelete {
        registry: &error_registry,
        target: candidate.clone(),
        deadline,
        live: [
            OpaqueId::new("cas:a-root").expect("root id"),
            candidate.object_id().clone(),
        ]
        .into_iter()
        .collect(),
        attempted_pin: None,
        delete_called: false,
        delete_error: Some(CasAdminError::Transport),
    };
    assert_eq!(
        execute_sweep_batch(
            &error_plan,
            0,
            &mut failed_delete,
            &mutation,
            &error_protection,
        )
        .expect_err("unknown delete still releases the process-local barrier"),
        RetentionError::SweepDeleteOutcomeUnknown
    );
    assert_eq!(
        failed_delete.attempted_pin,
        Some(Err(RetentionError::SourcePinAdmissionClosed))
    );
    let after_error = error_registry
        .pin(candidate, deadline)
        .expect("RAII sweep admission releases on error");
    drop(after_error);
}
