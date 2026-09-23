//! Synthetic effect-owner faults exercise the real coordinator, not redb,
//! native publication, transport or daemon integration qualification.

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use search_access::{
    AccessCheckpoint, AccessError, DurableSecurityRestriction, LiveSecurityState,
    MAX_SECURITY_DEPENDENTS, RequestSecurityFence, SecurityDependentReceipt,
    SecurityMutationBarrier, SecurityMutationEffects, SecurityRestriction,
};
use search_contracts::{
    AccessPolicyRevision, Blake3Digest32, BoundedList, BoundedSet, LiveDenySnapshotRef,
    MAX_SET_ITEMS, OpaqueId, OpaqueRef, ReceiptRef, SecurityMutationPhase, SourceMembershipId,
};

fn id(value: &str) -> OpaqueId {
    OpaqueId::new(value).unwrap()
}

fn reference(value: &str) -> ReceiptRef {
    ReceiptRef::new(value).unwrap()
}

fn member(value: u128) -> SourceMembershipId {
    SourceMembershipId::from_bytes(value.to_be_bytes())
}

fn command() -> SecurityRestriction {
    let expected_live = LiveSecurityState {
        generation: 7,
        denied_memberships: BTreeSet::new(),
        purged_memberships: BTreeSet::new(),
        fail_closed: false,
        snapshot_digest: Blake3Digest32::from_bytes([7; 32]),
    };
    let next_live = LiveSecurityState {
        generation: 8,
        denied_memberships: BTreeSet::from([member(1)]),
        purged_memberships: BTreeSet::from([member(2)]),
        snapshot_digest: Blake3Digest32::from_bytes([8; 32]),
        ..expected_live.clone()
    };
    SecurityRestriction {
        operation_id: id("fixture-operation"),
        security_domain_ref: OpaqueRef::new("fixture-domain").unwrap(),
        expected_policy_revision: AccessPolicyRevision::new(11),
        policy_revision: AccessPolicyRevision::new(12),
        expected_live,
        next_live,
        required_dependents: BoundedSet::from_items([
            id("queries"), id("handles"), id("continuations"),
        ]).unwrap(),
    }
}

fn owner(command: &SecurityRestriction) -> SecurityMutationBarrier {
    SecurityMutationBarrier::from_recovered_snapshot(
        command.security_domain_ref.clone(),
        command.expected_policy_revision,
        command.expected_live.clone(),
        command.required_dependents.clone(),
    ).unwrap()
}

fn checkpoint(owner: &SecurityMutationBarrier, value: u128) -> Result<u64, AccessError> {
    owner.with_live_checkpoint(
        &RequestSecurityFence {
            planned_generation: 7,
            memberships: BTreeSet::from([member(value)]),
        },
        AccessCheckpoint::BeforeResultEmission,
        |permit| permit.live_generation,
    )
}

fn snapshot_ref(command: &SecurityRestriction) -> LiveDenySnapshotRef {
    LiveDenySnapshotRef {
        security_domain_ref: command.security_domain_ref.clone(),
        live_deny_generation: command.next_live.generation,
        snapshot_digest: command.next_live.snapshot_digest,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Fault {
    #[default]
    None,
    BeforeCommit,
    AfterCommit,
    CommitMismatch,
    Readback,
    Publish,
    Publication(u8),
    Invalidate,
    Acknowledgement(u8),
    Panic(u8),
}

#[derive(Default)]
struct Effects {
    committed: Option<DurableSecurityRestriction>,
    fault: Fault,
    calls: Vec<&'static str>,
}

impl SecurityMutationEffects for Effects {
    type Error = &'static str;

    fn commit_restriction(
        &mut self,
        command: &SecurityRestriction,
    ) -> Result<DurableSecurityRestriction, Self::Error> {
        self.calls.push("commit");
        if self.fault == Fault::BeforeCommit {
            return Err("fixture cancellation before write");
        }
        let committed = DurableSecurityRestriction {
            command: command.clone(),
            receipt_ref: reference(&format!("fixture-durable-{}", command.operation_id.as_str())),
        };
        self.committed = Some(committed.clone());
        assert_ne!(self.fault, Fault::Panic(0), "fixture unwind after commit");
        if self.fault == Fault::AfterCommit {
            return Err("fixture timeout after possible write");
        }
        let mut returned = committed;
        if self.fault == Fault::CommitMismatch {
            returned.command.operation_id = id("foreign-operation");
        }
        Ok(returned)
    }

    fn readback_restriction(
        &mut self,
        _command: &SecurityRestriction,
    ) -> Result<Option<DurableSecurityRestriction>, Self::Error> {
        self.calls.push("readback");
        if self.fault == Fault::Readback {
            return Err("fixture unreadable control head");
        }
        Ok(self.committed.clone())
    }

    fn publish_live_restriction(
        &mut self,
        committed: &DurableSecurityRestriction,
    ) -> Result<LiveDenySnapshotRef, Self::Error> {
        self.calls.push("publish");
        assert_ne!(self.fault, Fault::Panic(1), "fixture publication unwind");
        if self.fault == Fault::Publish {
            return Err("fixture publication failure");
        }
        let mut result = snapshot_ref(&committed.command);
        match self.fault {
            Fault::Publication(0) => result.security_domain_ref = OpaqueRef::new("foreign").unwrap(),
            Fault::Publication(1) => result.live_deny_generation -= 1,
            Fault::Publication(2) => result.snapshot_digest = Blake3Digest32::from_bytes([99; 32]),
            _ => {}
        }
        Ok(result)
    }

    fn invalidate_dependents(
        &mut self,
        committed: &DurableSecurityRestriction,
    ) -> Result<BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>, Self::Error> {
        self.calls.push("invalidate");
        assert_ne!(self.fault, Fault::Panic(2), "fixture invalidation unwind");
        if self.fault == Fault::Invalidate {
            return Err("fixture partial invalidation");
        }
        let mut receipts = committed.command.required_dependents.iter().map(|dependent| {
            SecurityDependentReceipt {
                dependent: dependent.clone(),
                mutation_receipt_ref: committed.receipt_ref.clone(),
                live_snapshot_ref: snapshot_ref(&committed.command),
                receipt_ref: reference(&format!("fixture-receipt-{}", dependent.as_str())),
            }
        }).collect::<Vec<_>>();
        match self.fault {
            Fault::Acknowledgement(0) => { receipts.pop(); }
            Fault::Acknowledgement(1) => receipts[1] = receipts[0].clone(),
            Fault::Acknowledgement(2) => receipts[0].dependent = id("foreign-owner"),
            Fault::Acknowledgement(3) => receipts[0].mutation_receipt_ref = reference("foreign-mutation"),
            Fault::Acknowledgement(4) => receipts[0].live_snapshot_ref.live_deny_generation -= 1,
            Fault::Acknowledgement(5) => {
                receipts[0].live_snapshot_ref.snapshot_digest = Blake3Digest32::from_bytes([99; 32]);
            }
            Fault::Acknowledgement(6) => receipts[1].receipt_ref = receipts[0].receipt_ref.clone(),
            _ => {}
        }
        receipts.reverse();
        Ok(BoundedList::new(receipts).unwrap())
    }
}

#[test]
fn ordered_effects_exact_acknowledgements_and_idempotent_completion() {
    let command = command();
    let mut owner = owner(&command);
    let mut effects = Effects::default();
    assert_eq!(checkpoint(&owner, 1), Ok(7));
    let receipt = owner.apply_security_mutation(command.clone(), &mut effects).unwrap();
    assert_eq!(effects.calls, ["commit", "publish", "invalidate"]);
    assert_eq!(owner.phase(), SecurityMutationPhase::Acknowledged);
    assert_eq!(owner.live_snapshot(), Ok(&command.next_live));
    assert_eq!(checkpoint(&owner, 1), Err(AccessError::LiveRevocation));
    assert_eq!(checkpoint(&owner, 2), Err(AccessError::LivePurge));
    assert_eq!(checkpoint(&owner, 3), Ok(8));
    assert_eq!(receipt.dependent_receipts.iter().map(|r| &r.dependent).collect::<Vec<_>>(),
        command.required_dependents.iter().collect::<Vec<_>>());
    assert_eq!(owner.apply_security_mutation(command.clone(), &mut effects), Ok(receipt));
    assert_eq!(effects.calls.len(), 3);
    let diagnostic = format!("{owner:?}");
    assert!(!diagnostic.contains("fixture-domain"));
    assert!(!diagnostic.contains("fixture-operation"));
    let mut changed = command;
    changed.next_live.denied_memberships.insert(member(9));
    assert_eq!(
        owner.apply_security_mutation(changed, &mut effects),
        Err(AccessError::SecurityOperationConflict),
    );
    assert_eq!(checkpoint(&owner, 3), Ok(8));
}

#[test]
fn every_effect_fault_closes_checkpoints_and_recovers_by_readback() {
    for fault in [Fault::BeforeCommit, Fault::AfterCommit, Fault::Publish, Fault::Invalidate] {
        let command = command();
        let mut owner = owner(&command);
        let mut effects = Effects { fault, ..Effects::default() };
        assert!(owner.apply_security_mutation(command.clone(), &mut effects).is_err());
        assert_eq!(owner.phase(), SecurityMutationPhase::FailClosed);
        assert_eq!(checkpoint(&owner, 3), Err(AccessError::SecurityFailClosed));
        assert!(owner.live_snapshot().is_err());
        effects.fault = Fault::None;
        effects.calls.clear();
        owner.apply_security_mutation(command.clone(), &mut effects).unwrap();
        let expected = if fault == Fault::BeforeCommit {
            vec!["readback", "commit", "publish", "invalidate"]
        } else {
            vec!["readback", "publish", "invalidate"]
        };
        assert_eq!(effects.calls, expected, "{fault:?}");
        assert_eq!(checkpoint(&owner, 1), Err(AccessError::LiveRevocation));
    }
}

#[test]
fn no_mismatched_or_incomplete_receipt_can_complete_the_transition() {
    let faults = std::iter::once(Fault::CommitMismatch)
        .chain((0..3).map(Fault::Publication))
        .chain((0..7).map(Fault::Acknowledgement));
    for fault in faults {
        let command = command();
        let mut owner = owner(&command);
        let mut effects = Effects { fault, ..Effects::default() };
        assert!(owner.apply_security_mutation(command.clone(), &mut effects).is_err(), "{fault:?}");
        assert_eq!(checkpoint(&owner, 3), Err(AccessError::SecurityFailClosed));
        assert_eq!(owner.phase(), SecurityMutationPhase::FailClosed);
        let calls = effects.calls.len();
        let mut foreign = command;
        foreign.operation_id = id("another-operation");
        assert_eq!(
            owner.apply_security_mutation(foreign, &mut effects),
            Err(AccessError::SecurityOperationConflict),
        );
        assert_eq!(effects.calls.len(), calls);
        effects.fault = Fault::None;
        owner.recover_security_mutation(&mut effects).unwrap();
        assert_eq!(checkpoint(&owner, 3), Ok(8));
    }
}

#[test]
fn known_commit_cannot_disappear_change_identity_or_skip_failed_readback() {
    for case in 0..3 {
        let command = command();
        let mut owner = owner(&command);
        let mut effects = Effects { fault: Fault::Publish, ..Effects::default() };
        assert!(owner.apply_security_mutation(command, &mut effects).is_err());
        let saved = effects.committed.clone();
        effects.fault = Fault::None;
        match case {
            0 => effects.committed = None,
            1 => effects.committed.as_mut().unwrap().receipt_ref = reference("replacement"),
            _ => effects.fault = Fault::Readback,
        }
        effects.calls.clear();
        assert!(owner.recover_security_mutation(&mut effects).is_err());
        assert_eq!(effects.calls, ["readback"]);
        assert_eq!(checkpoint(&owner, 3), Err(AccessError::SecurityFailClosed));
        effects.committed = saved;
        effects.fault = Fault::None;
        owner.recover_security_mutation(&mut effects).unwrap();
        assert_eq!(checkpoint(&owner, 3), Ok(8));
    }
}

#[test]
fn pending_restore_and_adapter_unwind_never_expose_the_old_snapshot() {
    for stage in 0..3 {
        let command = command();
        let mut owner = owner(&command);
        let mut effects = Effects { fault: Fault::Panic(stage), ..Effects::default() };
        assert!(catch_unwind(AssertUnwindSafe(|| {
            owner.apply_security_mutation(command.clone(), &mut effects)
        })).is_err());
        assert_eq!(checkpoint(&owner, 3), Err(AccessError::SecurityFailClosed));
        effects.fault = Fault::None;
        effects.calls.clear();
        let mut restored = SecurityMutationBarrier::from_pending_restriction(
            command, effects.committed.as_ref().map(|value| value.receipt_ref.clone()),
        ).unwrap();
        assert_eq!(checkpoint(&restored, 3), Err(AccessError::SecurityFailClosed));
        restored.recover_security_mutation(&mut effects).unwrap();
        assert_eq!(effects.calls, ["readback", "publish", "invalidate"]);
        // A different recovered object cannot silently reopen the interrupted one.
        assert_eq!(checkpoint(&owner, 3), Err(AccessError::SecurityFailClosed));
    }
}

#[test]
fn malformed_restrictions_are_refused_before_any_effect_or_domain_block() {
    for case in 0..10 {
        let baseline = command();
        let mut owner = owner(&baseline);
        let mut changed = baseline.clone();
        let mut effects = Effects::default();
        match case {
            0 => changed.security_domain_ref = OpaqueRef::new("foreign-domain").unwrap(),
            1 => changed.required_dependents = BoundedSet::from_items([id("handles")]).unwrap(),
            2 => changed.expected_policy_revision = AccessPolicyRevision::new(10),
            3 => changed.expected_live.snapshot_digest = Blake3Digest32::from_bytes([99; 32]),
            4 => changed.next_live.generation = 7,
            5 => changed.next_live.generation = 6,
            6 => changed.policy_revision = AccessPolicyRevision::new(10),
            7 => changed.next_live.fail_closed = true,
            8 => {
                changed.next_live.denied_memberships.clear();
                changed.next_live.purged_memberships.clear();
            }
            _ => {
                changed.next_live.denied_memberships = (0..=MAX_SET_ITEMS)
                    .map(|value| member(value as u128)).collect();
            }
        }
        assert!(owner.apply_security_mutation(changed, &mut effects).is_err(), "case={case}");
        assert!(effects.calls.is_empty());
        assert_eq!(owner.live_snapshot(), Ok(&baseline.expected_live));
        assert_eq!(checkpoint(&owner, 3), Ok(7));
    }
}

#[test]
fn old_replay_and_permissive_changes_cannot_replace_a_newer_restriction() {
    let first = command();
    let mut owner = owner(&first);
    let mut effects = Effects::default();
    owner.apply_security_mutation(first.clone(), &mut effects).unwrap();
    let second = SecurityRestriction {
        operation_id: id("second-operation"),
        expected_policy_revision: first.policy_revision,
        policy_revision: AccessPolicyRevision::new(13),
        expected_live: first.next_live.clone(),
        next_live: LiveSecurityState {
            generation: 9,
            denied_memberships: BTreeSet::from([member(1), member(4)]),
            snapshot_digest: Blake3Digest32::from_bytes([9; 32]),
            ..first.next_live.clone()
        },
        ..first.clone()
    };
    let mut weakened = second.clone();
    weakened.next_live.purged_memberships.clear();
    assert!(owner.apply_security_mutation(weakened, &mut effects).is_err());
    owner.apply_security_mutation(second, &mut effects).unwrap();
    effects.calls.clear();
    assert_eq!(owner.apply_security_mutation(first, &mut effects), Err(AccessError::SecurityFenceStale));
    assert!(effects.calls.is_empty());
    assert_eq!(checkpoint(&owner, 2), Err(AccessError::LivePurge));
    assert_eq!(checkpoint(&owner, 3), Ok(9));
}

#[test]
fn initialization_bounds_empty_owners_and_generation_exhaustion_fail_closed() {
    let mut command = command();
    assert!(SecurityMutationBarrier::from_recovered_snapshot(
        command.security_domain_ref.clone(), command.expected_policy_revision,
        command.expected_live.clone(), BoundedSet::empty(),
    ).is_err());
    let initial = owner(&command);
    assert!(initial.with_live_checkpoint(
        &RequestSecurityFence {
            planned_generation: 7,
            memberships: (0..=MAX_SET_ITEMS).map(|value| member(value as u128)).collect(),
        },
        AccessCheckpoint::BeforeResultEmission,
        |_| -> () { panic!("oversized checkpoint must not execute") },
    ).is_err());
    assert!(initial.live_snapshot().is_ok());
    command.expected_live.generation = u64::MAX;
    command.next_live.generation = u64::MAX;
    let mut owner = owner(&command);
    let mut effects = Effects::default();
    assert_eq!(
        owner.apply_security_mutation(command, &mut effects),
        Err(AccessError::SecurityGenerationRegression),
    );
    assert!(effects.calls.is_empty());
}
