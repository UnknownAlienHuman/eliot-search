use super::super::*;

/// Asserts exact typed S9.5 payload and vector parity between oracle and real
/// readback. The projection manifest owns expected digests; the bridge proves
/// the actual typed fields and vector values round-trip without extra payload.
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
        assert_eq!(actual, expected);
    }
}
