/// Deterministic 256-bit development fingerprint with independent lanes.
pub fn fingerprint(bytes: &[u8]) -> [u8; 32] {
    let mut lanes = [
        0xcbf2_9ce4_8422_2325_u64,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0xc2b2_ae3d_27d4_eb4f,
    ];
    for (index, byte) in bytes.iter().copied().enumerate() {
        for (lane_index, lane) in lanes.iter_mut().enumerate() {
            let mixed = byte.wrapping_add(
                u8::try_from((index + lane_index * 29) & 0xff).unwrap_or(0),
            );
            *lane ^= u64::from(mixed);
            *lane = lane
                .wrapping_mul(
                    0x0000_0100_0000_01b3_u64
                        .wrapping_add(u64::try_from(lane_index * 2).unwrap_or(0)),
                )
                .rotate_left(u32::try_from(11 + lane_index * 7).unwrap_or(11));
        }
    }
    let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    for (index, lane) in lanes.iter_mut().enumerate() {
        *lane ^= length.rotate_left(u32::try_from(index * 13).unwrap_or(0));
        *lane = avalanche(*lane);
    }
    let mut output = [0_u8; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        output[index * 8..index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    output
}

const fn avalanche(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
}

pub fn hex32(bytes: [u8; 32]) -> String {
    hex_bytes(&bytes)
}

pub(super) fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

pub(super) fn count_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        0
    } else {
        let mut lines = 0_usize;
        for byte in bytes {
            if *byte == b'\n' {
                lines += 1;
            }
        }
        lines.saturating_add(1)
    }
}
