//! Preconditions for immutable epoch publication. Synthetic manifest fixtures
//! exercise the actual coordinator API, not live Qdrant or digest derivation.
use super::*;

fn coordinator(old: &ProjectionManifest) -> PublicationCoordinator {
    PublicationCoordinator::new(epoch(4), epoch(9), Some(old.clone()), Some(digest(1)), 32).unwrap()
}

fn assert_rejected(old: &ProjectionManifest, new: ProjectionManifest) {
    let mut machine = coordinator(old);
    assert_eq!(
        machine.submit(prepared(Some(old.clone()), new, "conflict")),
        Err(PublicationError::InvalidPreparedPublication),
    );
    assert!(machine.active().is_none());
    assert_eq!(machine.visible_epoch(), epoch(4));
    assert_eq!(machine.last_reserved_epoch(), epoch(9));
    assert_eq!(machine.current_manifest(), Some(old));
    assert_eq!(
        machine.persist_intent(id("must-not-persist"), reference("unused")),
        Err(PublicationError::InvalidTransition),
    );
}

#[test]
fn every_immutable_entry_field_participates_in_collision_rejection() {
    let old = manifest(&[2]);
    for field in 0..16 {
        let mut new = old.clone();
        let entry = &mut new.entries[0];
        match field {
            0 => entry.identity_key.namespace_id = id("other-namespace"),
            1 => entry.identity_key.source_id = id("other-source"),
            2 => entry.identity_key.source_revision = NonZeroRevision::new(99).unwrap(),
            3 => entry.identity_key.unit_ordinal += 1,
            4 => {
                entry.identity_key.source_byte_start = 1;
                entry.identity_key.source_byte_end = 2;
            }
            5 => entry.identity_key.source_byte_end = 2,
            6 => entry.identity_key.projection_kind = ProjectionKind::ExactMetadata,
            7 => entry.identity_key.projection_fingerprint = digest(99),
            8 => entry.identity_key.projection_schema_revision = NonZeroRevision::new(2).unwrap(),
            9 => entry.source_membership_id = id("other-source-membership"),
            10 => entry.projection_membership_id = id("other-projection-membership"),
            11 => entry.unit_digest = digest(99),
            12 => entry.reference_digest = digest(99),
            13 => entry.payload_digest = digest(99),
            14 => {
                entry
                    .vector_digests
                    .insert("lexical".to_owned(), digest(99));
            }
            _ => {
                entry.vector_digests.insert("code".to_owned(), digest(98));
            }
        }
        assert_rejected(&old, new);
    }
}

#[test]
fn vector_names_and_values_cannot_change_under_a_retained_point_id() {
    let mut old = manifest(&[2]);
    old.entries[0]
        .vector_digests
        .insert("lexical".to_owned(), digest(7));
    for variant in 0..3 {
        let mut new = old.clone();
        match variant {
            0 => {
                new.entries[0]
                    .vector_digests
                    .insert("lexical".to_owned(), digest(8));
            }
            1 => new.entries[0].vector_digests.clear(),
            _ => {
                new.entries[0].vector_digests.clear();
                new.entries[0]
                    .vector_digests
                    .insert("renamed".to_owned(), digest(7));
            }
        }
        assert_rejected(&old, new);
    }
}

#[test]
fn collision_at_any_position_is_rejected_with_no_partial_reservation() {
    let old = manifest(&[1, 3, 5]);
    for index in [0, 2, 4] {
        let mut new = manifest(&[1, 2, 3, 4, 5, 6]);
        new.entries[index].payload_digest = digest(99);
        assert_rejected(&old, new);
    }
}

#[test]
fn exact_retained_points_are_neither_staged_nor_closed() {
    let old = manifest(&[1, 3, 5]);
    let mut machine = durable(Some(old), manifest(&[2, 3, 4]));
    let plan = machine.begin_compensation_plan().unwrap();
    assert_eq!(plan.staged_ids, vec![point(2), point(4)]);
    assert_eq!(plan.closed_ids, vec![point(1), point(5)]);
    assert!(!plan.staged_ids.contains(&point(3)));
    assert!(!plan.closed_ids.contains(&point(3)));
    machine
        .compensate_and_restore(compensation(&plan), restoration(&plan))
        .unwrap();
}

#[test]
fn fresh_and_delete_only_plans_remain_usable() {
    for (old, new, staged, closed) in [
        (None, manifest(&[2]), vec![point(2)], vec![]),
        (Some(manifest(&[2])), manifest(&[]), vec![], vec![point(2)]),
    ] {
        let mut machine = durable(old, new);
        let (commit, observed) = visible(&mut machine);
        assert_eq!(observed.staged_ids, staged);
        assert_eq!(observed.closed_ids, closed);
        machine.publish_control_snapshot(snapshot(&commit)).unwrap();
        machine.complete().unwrap();
    }
}

#[test]
fn valid_replacement_with_new_ids_reaches_commit_and_snapshot_acknowledgement() {
    let old = manifest(&[1, 3]);
    let new = manifest(&[2, 3]);
    let mut machine = durable(Some(old), new.clone());
    let (commit, observed) = visible(&mut machine);
    assert_eq!(observed.staged_ids, vec![point(2)]);
    assert_eq!(observed.closed_ids, vec![point(1)]);
    assert_eq!(
        recover(machine.active().unwrap(), &observed).unwrap(),
        PublicationRecoveryDecision::PublishSnapshot
    );
    machine.publish_control_snapshot(snapshot(&commit)).unwrap();
    machine.complete().unwrap();
    assert_eq!(machine.current_manifest(), Some(&new));
    assert_eq!(machine.visible_epoch(), epoch(1));
}

#[test]
fn rejecting_a_collision_does_not_prevent_a_corrected_submission() {
    let old = manifest(&[1]);
    let mut machine = coordinator(&old);
    let mut bad = old.clone();
    bad.entries[0].payload_digest = digest(9);
    assert!(
        machine
            .submit(prepared(Some(old.clone()), bad, "bad"))
            .is_err()
    );
    assert_eq!(
        machine
            .submit(prepared(Some(old), manifest(&[2]), "corrected"))
            .unwrap(),
        epoch(10)
    );
    assert_eq!(machine.active().unwrap().transaction_id(), &id("corrected"));
}

#[test]
fn conflicting_competitor_does_not_replace_the_active_transaction() {
    let old = manifest(&[1]);
    let mut machine = durable(Some(old.clone()), manifest(&[2]));
    let before = machine.active().unwrap().clone();
    let mut bad = old.clone();
    bad.entries[0].unit_digest = digest(99);
    assert_eq!(
        machine.submit(prepared(Some(old), bad, "competitor")),
        Err(PublicationError::PublicationBusy)
    );
    assert_eq!(machine.active(), Some(&before));
    assert_eq!(machine.last_reserved_epoch(), epoch(1));
}

#[test]
fn exhaustive_small_manifest_pairs_never_overlap_staged_and_retired_ids() {
    fn subset(mask: u8) -> ProjectionManifest {
        manifest(
            &(1_u8..=5)
                .filter(|n| mask & (1_u8 << (*n - 1)) != 0)
                .collect::<Vec<_>>(),
        )
    }
    for old_mask in 0_u8..32 {
        for new_mask in 0_u8..32 {
            for changed in [false, true] {
                let old = subset(old_mask);
                let mut new = subset(new_mask);
                if changed {
                    for entry in &mut new.entries {
                        entry.payload_digest = digest(99);
                    }
                }
                let overlap = changed && old_mask & new_mask != 0;
                let mut machine = coordinator(&old);
                let result = machine.submit(prepared(Some(old.clone()), new, "enumerated"));
                if overlap {
                    assert_eq!(result, Err(PublicationError::InvalidPreparedPublication));
                    assert!(machine.active().is_none());
                    assert_eq!(machine.last_reserved_epoch(), epoch(9));
                } else {
                    assert_eq!(result, Ok(epoch(10)));
                    machine
                        .persist_intent(id("persist"), reference("intent"))
                        .unwrap();
                    let plan = machine.begin_compensation_plan().unwrap();
                    assert!(
                        plan.staged_ids
                            .iter()
                            .all(|id| !plan.closed_ids.contains(id))
                    );
                }
            }
        }
    }
}

#[test]
fn legacy_same_id_effects_cannot_authorize_forward_or_snapshot_recovery() {
    let mut machine = durable(Some(manifest(&[1])), manifest(&[2]));
    let (commit, mut seen) = visible(&mut machine);
    machine.publish_control_snapshot(snapshot(&commit)).unwrap();
    let mut legacy = machine.active().unwrap().clone();
    // A synthetic already-recorded conflicting plan, not a new allowed submit.
    legacy.prepared.new_manifest.entries[0].point_id = point(1);
    legacy.stage_receipt.as_mut().unwrap().staged_ids = vec![point(1)];
    seen.staged_ids = vec![point(1)];
    let original = legacy.clone();
    for phase in [
        PublicationPhase::IntentDurable,
        PublicationPhase::NewPointsAcknowledged,
        PublicationPhase::OldPointsClosedAcknowledged,
        PublicationPhase::ReadbackVerified,
        PublicationPhase::ControlCommitted,
        PublicationPhase::SnapshotPublished,
        PublicationPhase::Compensating,
        PublicationPhase::Aborted,
        PublicationPhase::PublicationBlocked,
    ] {
        legacy.phase = phase;
        for observed_epoch in [legacy.previous_visible_epoch, legacy.target_epoch] {
            for snapshot_published in [false, true] {
                seen.control_visible_epoch = observed_epoch;
                seen.snapshot_published = snapshot_published;
                let before = legacy.clone();
                assert_eq!(
                    recover(&legacy, &seen).unwrap(),
                    PublicationRecoveryDecision::PublicationBlocked
                );
                assert_eq!(legacy, before);
            }
        }
    }
    legacy.phase = original.phase;
    assert_eq!(legacy, original);
}

#[test]
fn legacy_collision_is_blocked_even_without_observed_effects() {
    let machine = durable(Some(manifest(&[1])), manifest(&[2]));
    let mut transaction = machine.active().unwrap().clone();
    transaction.prepared.new_manifest.entries[0].point_id = point(1);
    assert_eq!(
        recover(&transaction, &observation(&machine)).unwrap(),
        PublicationRecoveryDecision::PublicationBlocked
    );
}
