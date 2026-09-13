use super::super::*;

/// Asserts per-point digest/epoch parity between oracle and real readback.
pub(crate) fn assert_readback_parity(
    oracle_readback: &BoundedPointReadback,
    real_readback: &BoundedPointReadback,
) {
    assert_eq!(real_readback.missing_ids, oracle_readback.missing_ids);
    assert!(real_readback.missing_ids.is_empty());
    assert!(real_readback.unexpected_ids.is_empty());
    for expected in &oracle_readback.points {
        let actual = real_readback
            .points
            .iter()
            .find(|point| point.point_id == expected.point_id)
            .expect("parity point present");
        assert_eq!(
            actual.payload.payload_digest,
            expected.payload.payload_digest
        );
        assert_eq!(
            actual.payload.identity_digest,
            expected.payload.identity_digest
        );
        assert_eq!(
            actual.payload.valid_from_epoch,
            expected.payload.valid_from_epoch
        );
        assert_eq!(
            actual.payload.valid_until_epoch_exclusive,
            expected.payload.valid_until_epoch_exclusive
        );
    }
}
