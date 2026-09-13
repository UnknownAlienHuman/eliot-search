use super::support::*;

#[test]
fn full_ledger_rejects_every_mutation_without_changing_points() {
    for kind in [Kind::Upsert, Kind::Close, Kind::Delete] {
        let (mut bridge, route) = seeded(1);
        let before = state(&bridge, &route);
        assert_eq!(
            apply(&mut bridge, &route, kind, mutation("rejected", 2)),
            Err(BridgeError::MutationTooLarge),
            "{kind:?}"
        );
        assert_eq!(state(&bridge, &route), before, "{kind:?}");
    }
}

#[test]
fn full_ledger_still_replays_and_reports_identity_conflicts() {
    for kind in [Kind::Upsert, Kind::Close, Kind::Delete] {
        let (mut bridge, route) = seeded(2);
        let mut expected =
            apply(&mut bridge, &route, kind, mutation("accepted", 2))
                .expect("accepted mutation");
        let before = state(&bridge, &route);
        expected.replayed = true;
        assert_eq!(
            apply(&mut bridge, &route, kind, mutation("accepted", 2))
                .expect("idempotent replay"),
            expected
        );
        assert_eq!(
            apply(&mut bridge, &route, kind, mutation("accepted", 3)),
            Err(BridgeError::OperationConflict)
        );
        assert_eq!(state(&bridge, &route), before);
    }
}

#[test]
fn invalid_upsert_batch_neither_writes_nor_consumes_a_receipt() {
    let (mut bridge, route) = seeded(2);
    let before = state(&bridge, &route);
    let mut invalid = point(2, 2.0);
    invalid
        .vectors
        .get_mut(VECTOR)
        .expect("fixture vector")
        .values = vec![(0, 1.0), (0, 2.0)];
    assert_eq!(
        bridge.upsert_exact(
            &route,
            vec![point(1, 9.0), invalid],
            mutation("retry", 2),
        ),
        Err(BridgeError::VectorDimensionMismatch)
    );
    assert_eq!(state(&bridge, &route), before);
    let receipt = bridge
        .upsert_exact(&route, vec![point(2, 2.0)], mutation("retry", 3))
        .expect("valid retry");
    assert!(!receipt.replayed);
}

#[test]
fn invalid_close_batch_neither_writes_nor_consumes_a_receipt() {
    let (mut bridge, route) = seeded(2);
    let before = state(&bridge, &route);
    assert_eq!(
        bridge.close_exact(
            &route,
            vec![id(1), id(2)],
            epoch(20),
            mutation("retry", 2),
        ),
        Err(BridgeError::PointNotFound)
    );
    assert_eq!(state(&bridge, &route), before);
    let receipt = bridge
        .close_exact(
            &route,
            vec![id(1)],
            epoch(20),
            mutation("retry", 3),
        )
        .expect("valid retry");
    assert!(!receipt.replayed);
}

#[test]
fn duplicate_delete_neither_writes_nor_consumes_a_receipt() {
    let (mut bridge, route) = seeded(2);
    let before = state(&bridge, &route);
    assert_eq!(
        bridge.delete_exact(
            &route,
            vec![id(1), id(1)],
            mutation("retry", 2),
        ),
        Err(BridgeError::DuplicatePointId)
    );
    assert_eq!(state(&bridge, &route), before);
    let receipt = bridge
        .delete_exact(&route, vec![id(1)], mutation("retry", 3))
        .expect("valid retry");
    assert!(!receipt.replayed);
}
