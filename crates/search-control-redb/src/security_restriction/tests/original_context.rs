//! Real-redb proofs for the additive original-context security-restriction
//! read/commit/recovery entry points. Two causal cases only: an expired caller
//! deadline refuses before any write while one borrowed allowance is never
//! renewed, and a post-commit callback refusal stays outcome-unknown until the
//! exact stored request is recovered and confirmed current.
//!
//! These fixtures do not qualify native admission, credentials, policy
//! authority, power loss or a live restriction publication.

use super::*;
use crate::MigrationBootstrapWork as Work;
use crate::policy_codec::ACCESS_POLICY_RECORD_LEN;
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

/// Bytes charged by one reservation, mirroring the checked engine's own units.
fn charged(unit: Work) -> usize {
    match unit {
        Work::Read(bytes) | Work::Write(bytes) | Work::Allocate(bytes) | Work::Process(bytes) => bytes,
        Work::Checkpoint | Work::Open => 0,
    }
}

/// Replacement state used by both cases; only the deny generation moves.
fn successor() -> SecurityPolicyState {
    let mut next = state();
    next.policy.live_deny_generation += 1;
    next.policy.policy_revision = AccessPolicyRevision::new(12);
    next.snapshot_digest = Blake3Digest32::from_bytes([14; 32]);
    next
}

#[test]
fn current_observation_survives_owner_succession_without_rebinding_or_writes() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let original = initial(&mut journal);
    let namespace = state().policy.namespace_id;
    let original_bytes = codec::encode(original.mutation()).unwrap();
    let next = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    let mut journal = journal.advance_owner_with_context(next, MutationId([71; 32]),
        &context(&Cancel::default())).unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    let charged_bytes = Cell::new(0_usize);
    let work = |unit| { charged_bytes.set(charged_bytes.get() + charged(unit)); Ok::<_, &str>(()) };
    let snapshot = journal.read_snapshot().unwrap();
    let writes = journal.committed_writes();
    assert_eq!(journal.read_security_restriction_with_original_context(namespace, end, &work)
        .unwrap_err().control, ControlError::IdentityMismatch);
    assert_eq!(journal.recover_security_restriction_with_original_context(original.mutation(), end, &work)
        .unwrap_err().control, ControlError::IdentityMismatch);
    let (current, observed) = journal.observe_security_restriction_with_original_context(namespace, end, &work).unwrap();
    let observed = observed.unwrap();
    current.confirm_current(&observed).unwrap();
    assert_eq!(observed.observed_identity(), next);
    assert_eq!(observed.mutation(), original.mutation());
    assert_eq!(codec::encode(observed.mutation()).unwrap(), original_bytes);
    assert_eq!(observed.mutation().replacement(), &state());
    assert!(!observed.mutation().replacement().denied_memberships.is_empty());
    assert!(!observed.mutation().replacement().purged_memberships.is_empty());
    assert_eq!(journal.read_snapshot().unwrap(), snapshot);
    assert_eq!(journal.committed_writes(), writes);
    assert!(charged_bytes.get() > 0);
    assert_eq!(current.confirm_current(&original), Err(ControlError::IdentityMismatch));
    // A new restriction is a new current-owner command, not a rebound old one.
    let update = current.prepare_restriction(&observed, op("operation:new-owner"), digest(), successor(), owners()).unwrap();
    assert_eq!(update.identity(), next);
    let committed = journal.commit_security_restriction_with_original_context(&update, end, &work).unwrap();
    let (latest, _) = journal.observe_security_restriction_with_original_context(namespace, end, &work).unwrap();
    latest.confirm_current(&committed).unwrap();
    assert_eq!(latest.confirm_current(&observed), Err(ControlError::TransactionConflict));
}

#[test]
fn current_observation_refuses_foreign_future_and_uncommitted_state() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let original = initial(&mut journal);
    let namespace = state().policy.namespace_id;
    let end = Instant::now() + Duration::from_secs(60);
    let work = |_| Ok::<_, &str>(());
    let command_key = key(RESTRICTION_PREFIX, namespace).unwrap();
    let mut foreign = original.mutation().clone();
    foreign.identity.data_root_id = DataRootId::from_bytes([99; 16]);
    raw_write(&mut journal, vec![ControlWrite { key: command_key.clone(),
        value: ControlValue::new(ControlRecordClass::Operation, codec::encode(&foreign).unwrap(), JournalLimits::BASELINE).unwrap() }], 72);
    let snapshot = journal.read_snapshot().unwrap();
    assert_eq!(journal.observe_security_restriction_with_original_context(namespace, end, &work).unwrap_err().control,
        ControlError::IdentityMismatch);
    assert_eq!(journal.read_snapshot().unwrap(), snapshot);
    let mut future = original.mutation().clone();
    future.identity.owner_epoch = OwnerEpoch::new(2).unwrap();
    raw_write(&mut journal, vec![ControlWrite { key: command_key.clone(),
        value: ControlValue::new(ControlRecordClass::Operation, codec::encode(&future).unwrap(), JournalLimits::BASELINE).unwrap() }], 73);
    assert_eq!(journal.observe_security_restriction_with_original_context(namespace, end, &work).unwrap_err().control,
        ControlError::IdentityMismatch);
    // Equal policy bytes cannot stand in for a native commit ledger row.
    let mut missing = original.mutation().clone();
    missing.operation_id = op("operation:record-without-commit");
    raw_write(&mut journal, vec![ControlWrite { key: command_key,
        value: ControlValue::new(ControlRecordClass::Operation, codec::encode(&missing).unwrap(), JournalLimits::BASELINE).unwrap() }], 74);
    let snapshot = journal.read_snapshot().unwrap();
    assert_eq!(journal.observe_security_restriction_with_original_context(namespace, end, &work).unwrap_err().control,
        ControlError::StoreCorrupt);
    assert_eq!(journal.read_snapshot().unwrap(), snapshot);
}

#[test]
fn original_context_expired_deadline_refuses_before_writes_and_never_renews_the_allowance() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let first = initial(&mut journal);
    let namespace = state().policy.namespace_id;
    let baseline = journal.read_snapshot().unwrap();
    assert_eq!(baseline.generation, 1);
    assert_eq!(journal.committed_writes(), 2);

    let read = journal.read_security_restriction(namespace, &context(&Cancel::default())).unwrap();
    let command = read
        .prepare_restriction(&first, op("operation:metered"), digest(), successor(), owners())
        .unwrap();

    // An expired original absolute deadline stops every entry point at its first
    // checkpoint: no callback unit, no reservation, no write and no fence.
    let refused = Cell::new(0);
    let expired_work = |_| {
        refused.set(refused.get() + 1);
        Ok::<_, &str>(())
    };
    let expired = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    let read_error = journal
        .read_security_restriction_with_original_context(namespace, expired, &expired_work)
        .unwrap_err();
    let commit_error = journal
        .commit_security_restriction_with_original_context(&command, expired, &expired_work)
        .unwrap_err();
    let recover_error = journal
        .recover_security_restriction_with_original_context(&command, expired, &expired_work)
        .unwrap_err();
    assert_eq!(read_error.control, ControlError::BudgetExceeded);
    assert_eq!(commit_error.control, ControlError::BudgetExceeded);
    // An interrupted inspection is unresolved, never resolved absence.
    assert_eq!(recover_error.control, ControlError::CommitOutcomeUnknown);
    for error in [read_error, commit_error, recover_error] {
        assert_eq!(error.first_context, None);
    }
    assert_eq!(refused.get(), 0, "an expired deadline must not consult the original context");
    assert_eq!(journal.read_snapshot().unwrap(), baseline);
    assert_eq!(journal.committed_writes(), 2);
    assert!(!journal.requires_recovery());

    // One borrowed allowance spans read, commit and recovery: the same closure
    // and the same absolute deadline are reused and never renewed per call.
    let observed = RefCell::new(Vec::new());
    let remaining = Cell::new(4 * 1024 * 1024_usize);
    let work = |unit: Work| {
        let next = remaining.get().checked_sub(charged(unit)).ok_or("original-allowance")?;
        remaining.set(next);
        observed.borrow_mut().push(unit);
        Ok::<_, &str>(())
    };
    let end = Instant::now() + Duration::from_secs(60);
    let stored = journal.read_security_restriction_with_original_context(namespace, end, &work).unwrap();
    let after_read = remaining.get();
    assert!(after_read < 4 * 1024 * 1024, "the read must charge the original allowance");
    let policy_key = key(POLICY_PREFIX, namespace).unwrap();
    let command_key = key(RESTRICTION_PREFIX, namespace).unwrap();
    let previous = journal.read_snapshot().unwrap();
    let previous_row = previous.get(&command_key).unwrap().len();
    assert!(
        observed.borrow().contains(&Work::Read(previous_row)),
        "the stored command row is charged at its actual size"
    );
    assert!(observed.borrow().contains(&Work::Read(ACCESS_POLICY_RECORD_LEN)));
    assert!(
        observed.borrow().iter().any(|unit| matches!(unit, Work::Allocate(bytes) if *bytes >= previous_row)),
        "command preparation reserves input-derived capacity before allocating"
    );
    let command = stored
        .prepare_restriction(&first, op("operation:metered"), digest(), successor(), owners())
        .unwrap();

    let committed = journal.commit_security_restriction_with_original_context(&command, end, &work).unwrap();
    let after_commit = remaining.get();
    assert!(after_commit < after_read, "the commit must share the borrowed allowance");
    assert_eq!(journal.committed_writes(), 3);
    let current = journal.read_snapshot().unwrap();
    let command_row = current.get(&command_key).unwrap().len();
    assert!(
        observed.borrow().iter().any(|unit| matches!(unit, Work::Allocate(bytes) if *bytes >= command_row))
    );
    // Both staged rows are charged at their exact key+value size before insertion.
    assert!(observed.borrow().contains(&Work::Write(command_key.as_bytes().len() + command_row + 1)));
    assert!(
        observed
            .borrow()
            .contains(&Work::Write(policy_key.as_bytes().len() + ACCESS_POLICY_RECORD_LEN + 1))
    );

    let writes = journal.committed_writes();
    let recovered = journal
        .recover_security_restriction_with_original_context(&command, end, &work)
        .unwrap()
        .unwrap();
    let after_recover = remaining.get();
    assert!(after_recover < after_commit, "recovery must not renew the borrowed allowance");
    assert_eq!(journal.committed_writes(), writes);
    assert_eq!(recovered.receipt().operation_id, committed.receipt().operation_id);
    assert_eq!(recovered.mutation(), &command);
    let fresh = journal.read_security_restriction_with_original_context(namespace, end, &work).unwrap();
    fresh.confirm_current(&recovered).unwrap();
    assert_eq!(fresh.mutation(), Some(&command));
    assert!(
        observed.borrow().contains(&Work::Read(command_row)),
        "the fresh read meters the committed command row at its exact size"
    );
    // One un-renewed allowance strictly declined across read, commit, recovery
    // and the fresh confirmation read; each call saw the earlier call's charge.
    assert!(remaining.get() < after_recover, "the fresh read shares the same un-renewed allowance");
}

#[test]
fn original_context_post_commit_refusal_stays_unknown_until_exact_recovery_confirms_current() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let first = initial(&mut journal);
    let namespace = state().policy.namespace_id;
    let read = journal.read_security_restriction(namespace, &context(&Cancel::default())).unwrap();
    let command = read
        .prepare_restriction(&first, op("operation:unknown"), digest(), successor(), owners())
        .unwrap();

    // The engine reaches its real redb commit; the first post-commit readback
    // reservation is refused, so the outcome must stay unknown, not failed.
    let boundaries = Cell::new(0);
    let work = |unit: Work| {
        if unit == Work::Write(0) {
            boundaries.set(boundaries.get() + 1);
        }
        if boundaries.get() >= 2 && matches!(unit, Work::Read(bytes) if bytes > 0) {
            return Err("original-post-commit-readback");
        }
        Ok::<_, &str>(())
    };
    let end = Instant::now() + Duration::from_secs(60);
    let error = journal
        .commit_security_restriction_with_original_context(&command, end, &work)
        .unwrap_err();
    assert_eq!(error.control, ControlError::CommitOutcomeUnknown);
    assert_eq!(error.first_context, Some("original-post-commit-readback"));
    assert_eq!(boundaries.get(), 2, "both pre-commit boundary reservations completed before the refusal");
    assert!(journal.requires_recovery());
    // A pending fence blocks ordinary reads; only the exact request may resolve it.
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
    assert_eq!(journal.verify(), Err(ControlError::StoreQuarantined));

    // Only the identical original request resolves the fence. A changed request
    // is a conflict, and recovery writes nothing.
    let recovery = |_| Ok::<_, &str>(());
    let mut changed = command.clone();
    changed.replacement.fail_closed = true;
    assert_eq!(
        journal
            .recover_security_restriction_with_original_context(&changed, end, &recovery)
            .unwrap_err()
            .control,
        ControlError::OperationConflict
    );
    assert!(journal.requires_recovery());
    let writes = journal.committed_writes();
    let recovered = journal
        .recover_security_restriction_with_original_context(&command, end, &recovery)
        .unwrap()
        .unwrap();
    assert_eq!(journal.committed_writes(), writes);
    assert!(!journal.requires_recovery());
    assert_eq!(recovered.mutation(), &command);
    assert_eq!(recovered.receipt().operation_id, command.native_id());
    assert_eq!(recovered.receipt().after_generation, 2);
    // A fresh original-context read now returns that same stored request.
    let fresh = journal.read_security_restriction_with_original_context(namespace, end, &recovery).unwrap();
    fresh.confirm_current(&recovered).unwrap();
    assert_eq!(fresh.mutation(), Some(&command));
    assert_eq!(fresh.generation(), 2);
}
