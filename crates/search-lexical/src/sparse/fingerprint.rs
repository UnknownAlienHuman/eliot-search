//! Deterministic package-local fingerprints for sparse artifacts.

use crate::code_identifiers::CodeEncodingCheckpoint;
use crate::sparse::SparseError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SparseFingerprint(pub [u8; 32]);

/// Package-local deterministic 32-byte fingerprint over canonical bytes.
// Shared crate-internally with the frozen-profile and scoring-accounting
// modules; the `sparse` module is private as a unit, hence the allow.
#[allow(clippy::redundant_pub_crate)]
pub(crate) fn fingerprint_bytes(bytes: &[u8]) -> SparseFingerprint {
    let mut lanes = initial_lanes();
    mix(&mut lanes, bytes);
    SparseFingerprint(finish_lanes(lanes))
}

/// Same fingerprint algorithm with cooperative checks during byte mixing.
#[allow(clippy::redundant_pub_crate)]
pub(crate) fn fingerprint_bytes_checked(
    bytes: &[u8],
    checkpoint: &mut CodeEncodingCheckpoint<'_>,
) -> Result<SparseFingerprint, SparseError> {
    let mut lanes = initial_lanes();
    mix_checked(&mut lanes, bytes, checkpoint)?;
    Ok(SparseFingerprint(finish_lanes(lanes)))
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
        mix_byte(lanes, index, byte);
    }
}

fn mix_checked(
    lanes: &mut [u64; 4],
    bytes: &[u8],
    checkpoint: &mut CodeEncodingCheckpoint<'_>,
) -> Result<(), SparseError> {
    for (index, byte) in bytes.iter().copied().enumerate() {
        checkpoint.worked()?;
        mix_byte(lanes, index, byte);
    }
    Ok(())
}

fn mix_byte(lanes: &mut [u64; 4], index: usize, byte: u8) {
    let lane = index % lanes.len();
    lanes[lane] ^= u64::from(byte);
    lanes[lane] = lanes[lane]
        .wrapping_mul(0x0000_0100_0000_01b3)
        .rotate_left(u32::try_from(11 + lane * 7).unwrap_or(11));
}

fn finish_lanes(lanes: [u64; 4]) -> [u8; 32] {
    let mut output = [0_u8; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        output[index * 8..index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    output
}
