//! Canonical admission rejects tampered producer products before coordinator state changes.
use super::*;

fn coordinator(_: &Fixture) -> PublicationCoordinator {
    PublicationCoordinator::new(32).unwrap()
}

fn seeded(mut case: PublicationCase) -> PublicationCase {
    case.seed = publication_support::OwnerSeed {
        previous_visible: epoch(4),
        current_visible: epoch(4),
        last_reserved: epoch(9),
    };
    case
}

fn assert_rejected(old: &Fixture, new: Fixture) {
    let mut machine = coordinator(old);
    assert_eq!(
        machine.submit_fixture(seeded(proposal(Some(old.clone()), new, "conflict"))),
        Err(PublicationError::InvalidPreparedPublication),
    );
    assert!(machine.active().is_none());
    assert_eq!(machine.visible_epoch(), None);
    assert_eq!(machine.last_reserved_epoch(), None);
    assert_eq!(machine.current_manifest(), None);
    assert_eq!(
        machine.persist_intent(id("must-not-persist"), reference("unused")),
        Err(PublicationError::InvalidTransition),
    );
}

#[test]
fn every_immutable_entry_field_participates_in_collision_rejection() {
    let old = manifest(&[2]);
    for field in 0..15 {
        let mut new = old.clone();
        let entry = &mut new.entries[0];
        match field {
            0 => entry.point_identity_digest_256 = digest(99),
            1 => entry.source_membership_id = SourceMembershipId::from_bytes([99; 16]),
            2 => entry.projection_membership_id = ProjectionMembershipId::from_bytes([99; 16]),
            3 => entry.unit_id = UnitId::from_bytes([99; 16]),
            4 => entry.collection_generation_id = CollectionGenerationId::from_bytes([99; 16]),
            5 => {
                entry.installation_incarnation_id = InstallationIncarnationId::from_bytes([99; 16]);
            }
            6 => entry.access_partition_id = AccessPartitionId::from_bytes([99; 16]),
            7 => entry.scoring_partition_id = ScoringPartitionId::from_bytes([99; 16]),
            8 => entry.scoring_document_id = ScoringDocumentId::from_bytes([99; 16]),
            9 => {
                entry.projection_profile_set_id =
                    ProjectionProfileSetId::new("other-fixture-v1").unwrap();
            }
            10 => entry.payload_identity = digest(99),
            11 => entry.unit_digest = digest(99),
            12 => entry.reference_digest = digest(99),
            13 => {
                entry
                    .vector_digests
                    .insert("changed".to_owned(), digest(99));
            }
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
    for index in 0..old.units.units().len() {
        let new = old.with_changed_vector_at(index);
        let changed_unit = old.units.units()[index].unit_id();
        let old_at_epoch = old.at_epoch(epoch(4));
        let new_at_epoch = new.at_epoch(epoch(4));
        for fixture in [&old_at_epoch, &new_at_epoch] {
            search_projection_planner::verify_manifest_reconstruction(
                &fixture.plan.manifest,
                &fixture.plan.points,
                &fixture.scope,
                search_projection_planner::ProjectionBudget::BASELINE,
            )
            .expect("each C13 product is internally complete and canonical");
        }
        let old_entry = old_at_epoch
            .plan
            .manifest
            .entries
            .iter()
            .find(|entry| entry.unit_id == changed_unit)
            .expect("old planned point");
        let new_entry = new_at_epoch
            .plan
            .manifest
            .entries
            .iter()
            .find(|entry| entry.unit_id == changed_unit)
            .expect("changed planned point");
        assert_eq!(old_entry.point_id, new_entry.point_id);
        assert_ne!(old_entry.vector_digests, new_entry.vector_digests);
        assert_eq!(
            old_at_epoch
                .plan
                .manifest
                .entries
                .iter()
                .map(|entry| entry.point_id)
                .collect::<Vec<_>>(),
            new_at_epoch
                .plan
                .manifest
                .entries
                .iter()
                .map(|entry| entry.point_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            search_projection_planner::diff_manifests(
                &old_at_epoch.plan.manifest,
                &new_at_epoch.plan.manifest,
            ),
            Err(search_projection_planner::ProjectionError::ManifestMismatch)
        );
        assert_rejected(&old, new);
    }
}

#[test]
fn exact_retained_points_are_neither_staged_nor_closed() {
    let old = manifest(&[1, 3, 5]);
    let new = old.clone();
    assert_eq!(old.plan.points, new.plan.points);
    let mut machine = durable(Some(old.clone()), new);
    let plan = machine.begin_compensation_plan().unwrap();
    assert!(plan.staged_ids.is_empty());
    assert!(plan.closed_ids.is_empty());
    assert_eq!(
        machine.active().unwrap().prepared.difference().retain,
        old.plan.manifest.entries
    );
    machine
        .compensate_and_restore(compensation(&plan), restoration(&plan))
        .unwrap();
}

#[test]
fn fresh_and_delete_only_plans_remain_usable() {
    let mut fresh = durable(None, manifest(&[2]));
    let (commit, observed) = visible(&mut fresh);
    assert_eq!(
        observed.staged_ids,
        manifest(&[2])
            .entries
            .iter()
            .map(|entry| entry.point_id)
            .collect::<Vec<_>>()
    );
    assert!(observed.closed_ids.is_empty());
    fresh.publish_control_snapshot(snapshot(&commit)).unwrap();
    fresh.complete().unwrap();

    let old = manifest(&[2]);
    let mut retired = PublicationCoordinator::new(32).unwrap();
    retired
        .submit_fixture(publication_support::retirement_proposal(
            old.clone(),
            "retire",
        ))
        .unwrap();
    retired
        .persist_intent(id("persist-retire"), reference("intent-retire"))
        .unwrap();
    let (commit, observed) = visible(&mut retired);
    assert!(observed.staged_ids.is_empty());
    assert_eq!(
        observed.closed_ids,
        old.entries
            .iter()
            .map(|entry| entry.point_id)
            .collect::<Vec<_>>()
    );
    retired.publish_control_snapshot(snapshot(&commit)).unwrap();
    retired.complete().unwrap();
}

#[test]
fn valid_replacement_with_new_ids_reaches_commit_and_snapshot_acknowledgement() {
    let old = manifest(&[1, 3]);
    let new = manifest(&[2, 3]).with_changed_vector_at(0);
    assert_ne!(old.source.revision_id, new.source.revision_id);
    assert_ne!(
        old.scope.membership.representation_id,
        new.scope.membership.representation_id
    );
    assert!(old.plan.manifest.entries.iter().all(|old_entry| {
        new.plan
            .manifest
            .entries
            .iter()
            .all(|new_entry| old_entry.point_id != new_entry.point_id)
    }));
    let expected_new = new.at_epoch(epoch(2)).plan.manifest;
    let mut machine = durable(Some(old.clone()), new.clone());
    let (commit, observed) = visible(&mut machine);
    assert_eq!(
        observed.staged_ids,
        new.entries
            .iter()
            .filter(|e| !old.entries.iter().any(|o| o.point_id == e.point_id))
            .map(|e| e.point_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        observed.closed_ids,
        old.entries
            .iter()
            .filter(|e| !new.entries.iter().any(|n| n.point_id == e.point_id))
            .map(|e| e.point_id)
            .collect::<Vec<_>>()
    );
    let difference = machine.active().unwrap().prepared.difference();
    assert_eq!(difference.create, expected_new.entries);
    assert_eq!(difference.retain, Vec::new());
    assert_eq!(difference.retire, old.plan.manifest.entries);
    assert_eq!(
        recover(machine.active().unwrap(), &observed).unwrap(),
        PublicationRecoveryDecision::PublishSnapshot
    );
    machine.publish_control_snapshot(snapshot(&commit)).unwrap();
    machine.complete().unwrap();
    assert_eq!(machine.current_manifest(), Some(&expected_new));
    assert_eq!(machine.visible_epoch(), Some(epoch(2)));
}

#[test]
fn rejecting_a_collision_does_not_prevent_a_corrected_submission() {
    let old = manifest(&[1]);
    let mut machine = coordinator(&old);
    let mut bad = old.clone();
    bad.entries[0].payload_identity = digest(9);
    let mut bad_case = proposal(Some(old.clone()), bad, "bad");
    bad_case = seeded(bad_case);
    assert!(machine.submit_fixture(bad_case).is_err());
    let corrected = seeded(proposal(Some(old), manifest(&[2]), "corrected"));
    assert_eq!(machine.submit_fixture(corrected).unwrap(), epoch(10));
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
        machine.submit_fixture(proposal(Some(old), bad, "competitor")),
        Err(PublicationError::PublicationBusy)
    );
    assert_eq!(machine.active(), Some(&before));
    assert_eq!(machine.last_reserved_epoch(), Some(epoch(2)));
}

#[test]
fn exhaustive_small_manifest_pairs_never_overlap_staged_and_retired_ids() {
    fn subset(mask: u8) -> Fixture {
        manifest(
            &(1_u8..=2)
                .filter(|n| mask & (1_u8 << (*n - 1)) != 0)
                .collect::<Vec<_>>(),
        )
    }
    for old_mask in 0_u8..4 {
        for new_mask in 1_u8..4 {
            for changed in [false, true] {
                let old = (old_mask != 0).then(|| subset(old_mask));
                let mut new = subset(new_mask);
                if changed {
                    new = new.with_changed_vector_at(0);
                }
                let mut machine = PublicationCoordinator::new(32).unwrap();
                let same_address_conflict = old_mask != 0 && old_mask == new_mask && changed;
                if same_address_conflict {
                    let old = old.as_ref().unwrap();
                    search_projection_planner::verify_manifest_reconstruction(
                        &old.plan.manifest,
                        &old.plan.points,
                        &old.scope,
                        search_projection_planner::ProjectionBudget::BASELINE,
                    )
                    .expect("old C13 product is valid");
                    search_projection_planner::verify_manifest_reconstruction(
                        &new.plan.manifest,
                        &new.plan.points,
                        &new.scope,
                        search_projection_planner::ProjectionBudget::BASELINE,
                    )
                    .expect("changed-vector C13 product is valid");
                    assert_eq!(
                        search_projection_planner::diff_manifests(
                            &old.plan.manifest,
                            &new.plan.manifest,
                        ),
                        Err(search_projection_planner::ProjectionError::ManifestMismatch)
                    );
                }
                let previous = epoch(4);
                let target = epoch(10);
                let unchanged = old_mask != 0 && old_mask == new_mask;
                let expected_new = new
                    .at_epoch(if unchanged { previous } else { target })
                    .plan
                    .manifest;
                let expected_old = old
                    .as_ref()
                    .map(|fixture| fixture.at_epoch(previous).plan.manifest);
                let result = machine.submit_fixture(seeded(proposal(
                    old.clone(),
                    new.clone(),
                    "enumerated",
                )));
                if same_address_conflict {
                    assert_eq!(result, Err(PublicationError::InvalidPreparedPublication));
                    assert!(machine.active().is_none());
                    assert_eq!(machine.last_reserved_epoch(), None);
                    assert_eq!(machine.visible_epoch(), None);
                } else {
                    assert_eq!(result, Ok(epoch(10)));
                    let admitted = machine.active().unwrap().prepared.difference();
                    if unchanged {
                        assert_eq!(admitted.create, Vec::new());
                        assert_eq!(admitted.retain, expected_new.entries);
                        assert_eq!(admitted.retire, Vec::new());
                    } else {
                        assert_eq!(admitted.create, expected_new.entries);
                        assert_eq!(admitted.retain, Vec::new());
                        assert_eq!(
                            admitted.retire,
                            expected_old.map_or_else(Vec::new, |manifest| manifest.entries)
                        );
                    }
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
    let old_point = legacy.prepared.old_manifest().unwrap().entries[0].point_id;
    legacy.stage_receipt.as_mut().unwrap().staged_ids = vec![old_point];
    seen.staged_ids = vec![old_point];
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
    let mut machine = durable(Some(manifest(&[1])), manifest(&[2]));
    let (_, mut seen) = visible(&mut machine);
    let mut transaction = machine.active().unwrap().clone();
    let old_point = transaction.prepared.old_manifest().unwrap().entries[0].point_id;
    transaction.stage_receipt.as_mut().unwrap().staged_ids = vec![old_point];
    seen.staged_ids = vec![old_point];
    assert_eq!(
        recover(&transaction, &seen).unwrap(),
        PublicationRecoveryDecision::PublicationBlocked
    );
}
