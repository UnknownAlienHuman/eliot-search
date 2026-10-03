use super::super::*;

/// Asserts exact typed S9.5 payload/vector parity between oracle and real
/// readback. Qdrant payload is still not source evidence; this is transport
/// contract verification only.
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
        assert_eq!(actual.payload, expected.payload);
        assert_eq!(actual.vectors, expected.vectors);
    }
}
