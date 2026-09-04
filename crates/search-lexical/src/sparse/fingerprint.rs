//! Deterministic package-local fingerprints for sparse artifacts.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SparseFingerprint(pub [u8; 32]);

pub(super) fn fingerprint_bytes(bytes: &[u8]) -> SparseFingerprint {
    let mut lanes = initial_lanes();
    mix(&mut lanes, bytes);
    SparseFingerprint(finish_lanes(lanes))
}

pub(super) fn fingerprint_parts<'a>(
    parts: impl IntoIterator<Item = &'a [u8]>,
) -> SparseFingerprint {
    let mut lanes = initial_lanes();
    for part in parts {
        mix(&mut lanes, part);
    }
    SparseFingerprint(finish_lanes(lanes))
}

const fn initial_lanes() -> [u64; 4] {
    [
        0xcbf2_9ce4_8422_2325,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0xc2b2_ae3d_27d4_eb4f,
    ]
}

fn mix(lanes: &mut [u64; 4], bytes: &[u8]) {
    for (index, byte) in bytes.iter().copied().enumerate() {
        let lane = index % lanes.len();
        lanes[lane] ^= u64::from(byte);
        lanes[lane] = lanes[lane]
            .wrapping_mul(0x0000_0100_0000_01b3)
            .rotate_left(u32::try_from(11 + lane * 7).unwrap_or(11));
    }
}

fn finish_lanes(lanes: [u64; 4]) -> [u8; 32] {
    let mut output = [0_u8; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        output[index * 8..index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    output
}
