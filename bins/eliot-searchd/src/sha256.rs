//! SHA-256 byte API shared by the retained snapshot harness and DIRECT runtime.
//!
//! Raw hashes retain their standard SHA-256 meaning. Multipart framing is first
//! specified in `docs/runtime/DIRECT_HASH_FORMAT.md`; it is not a BLAKE3 digest,
//! keyed authenticator, or a compatibility claim for an external legacy store.

const INITIAL_STATE: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const ROUND_CONSTANTS: [u32; 64] = [
    0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5,
    0x3956_c25b, 0x59f1_11f1, 0x923f_82a4, 0xab1c_5ed5,
    0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3,
    0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174,
    0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc,
    0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
    0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7,
    0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967,
    0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13,
    0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85,
    0xa2bf_e8a1, 0xa81a_664b, 0xc24b_8b70, 0xc76c_51a3,
    0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
    0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5,
    0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3,
    0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208,
    0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
];

const PARTS_V1: &[u8] = b"eliot-search/sha256-parts/v1\0";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    pub(crate) fn from_hex(value: &str) -> Result<Self, String> {
        if value.len() != 64 {
            return Err("SHA256_HEX_LENGTH_INVALID".to_owned());
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            bytes[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
        }
        Ok(Self(bytes))
    }

    pub(crate) const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

pub fn digest_bytes(bytes: &[u8]) -> Sha256Digest {
    let mut state = INITIAL_STATE;
    let (blocks, remainder) = bytes.as_chunks::<64>();
    for block in blocks {
        compress(&mut state, block);
    }

    let bit_length = u64::try_from(bytes.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(8);
    let mut tail = [0_u8; 128];
    tail[..remainder.len()].copy_from_slice(remainder);
    tail[remainder.len()] = 0x80;
    let padded_length = if remainder.len() < 56 { 64 } else { 128 };
    tail[padded_length - 8..padded_length].copy_from_slice(&bit_length.to_be_bytes());
    for block in tail[..padded_length].as_chunks::<64>().0 {
        compress(&mut state, block);
    }

    let mut digest = [0_u8; 32];
    for (index, word) in state.into_iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    Sha256Digest(digest)
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0_u32; 64];
    for (index, word) in block.as_chunks::<4>().0.iter().enumerate() {
        schedule[index] = u32::from_be_bytes(*word);
    }
    for index in 16..64 {
        let s0 = schedule[index - 15].rotate_right(7)
            ^ schedule[index - 15].rotate_right(18)
            ^ (schedule[index - 15] >> 3);
        let s1 = schedule[index - 2].rotate_right(17)
            ^ schedule[index - 2].rotate_right(19)
            ^ (schedule[index - 2] >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(s1);
    }

    let mut wa = state[0];
    let mut wb = state[1];
    let mut wc = state[2];
    let mut wd = state[3];
    let mut we = state[4];
    let mut wf = state[5];
    let mut wg = state[6];
    let mut wh = state[7];

    for index in 0..64 {
        let sum1 = we.rotate_right(6) ^ we.rotate_right(11) ^ we.rotate_right(25);
        let choice = (we & wf) ^ ((!we) & wg);
        let temp1 = wh
            .wrapping_add(sum1)
            .wrapping_add(choice)
            .wrapping_add(ROUND_CONSTANTS[index])
            .wrapping_add(schedule[index]);
        let sum0 = wa.rotate_right(2) ^ wa.rotate_right(13) ^ wa.rotate_right(22);
        let majority = (wa & wb) ^ (wa & wc) ^ (wb & wc);
        let temp2 = sum0.wrapping_add(majority);

        wh = wg;
        wg = wf;
        wf = we;
        we = wd.wrapping_add(temp1);
        wd = wc;
        wc = wb;
        wb = wa;
        wa = temp1.wrapping_add(temp2);
    }

    state[0] = state[0].wrapping_add(wa);
    state[1] = state[1].wrapping_add(wb);
    state[2] = state[2].wrapping_add(wc);
    state[3] = state[3].wrapping_add(wd);
    state[4] = state[4].wrapping_add(we);
    state[5] = state[5].wrapping_add(wf);
    state[6] = state[6].wrapping_add(wg);
    state[7] = state[7].wrapping_add(wh);
}

pub fn digest(bytes: &[u8]) -> [u8; 32] {
    digest_bytes(bytes).as_bytes()
}

/// First defined multipart profile; see `docs/runtime/DIRECT_HASH_FORMAT.md`.
/// Concrete slices accept fixed arrays of different lengths without losing bytes.
/// Callers enforce their source/manifest/input byte ceilings before this pure call.
pub fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut framed = Vec::new();
    framed.extend_from_slice(PARTS_V1);
    framed.extend_from_slice(&length(domain.len()));
    framed.extend_from_slice(domain);
    framed.extend_from_slice(&length(parts.len()));
    for part in parts {
        framed.extend_from_slice(&length(part.len()));
        framed.extend_from_slice(part);
    }
    digest(&framed)
}

fn length(value: usize) -> [u8; 8] {
    u64::try_from(value)
        .expect("supported targets have at most 64-bit usize")
        .to_be_bytes()
}

/// Encodes arbitrary bounded bytes, including public revision-range output.
pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::new();
    for &byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

pub fn decode_digest(value: &str) -> Option<[u8; 32]> {
    Sha256Digest::from_hex(value).ok().map(Sha256Digest::as_bytes)
}

fn hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("SHA256_HEX_INVALID".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_sha256_matches_standard_vectors_and_the_existing_newtype_api() {
        for (input, expected) in [
            (b"".as_slice(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
            (b"abc".as_slice(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
            (b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".as_slice(),
             "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"),
        ] {
            assert_eq!(hex(&digest(input)), expected);
            assert_eq!(hex(&digest_bytes(input).as_bytes()), expected);
            assert_eq!(decode_digest(expected), Some(digest_bytes(input).as_bytes()));
        }
    }

    #[test]
    fn byte_encoding_and_digest_decoding_have_distinct_length_contracts() {
        assert_eq!(hex(&[]), "");
        assert_eq!(hex(&[0, 15, 16, 255]), "000f10ff");
        let bytes = std::array::from_fn::<_, 32, _>(|index| u8::try_from(index).unwrap());
        assert_eq!(decode_digest(&hex(&bytes).to_uppercase()), Some(bytes));
        for bad in [String::new(), "00".repeat(31), "00".repeat(33), "gg".repeat(32),
            "é".repeat(32), format!(" {}", "00".repeat(32))] {
            assert!(decode_digest(&bad).is_none());
        }
    }

    #[test]
    fn multipart_known_answer_is_frozen_independently_of_the_implementation() {
        assert_eq!(hex(&digest_parts(b"test/domain/v1", &[b"abc", b"", b"def"])),
            "ab1b291fe982137e0741dc711096200882e40d9cd32fd5a07b6177d7493b9078");
    }

    #[test]
    fn multipart_framing_binds_domain_boundaries_empty_parts_count_and_order() {
        assert_ne!(digest_parts(b"ab", &[b"c"]), digest_parts(b"a", &[b"bc"]));
        assert_ne!(digest_parts(b"d", &[b"ab", b"c"]), digest_parts(b"d", &[b"a", b"bc"]));
        assert_ne!(digest_parts(b"d", &[]), digest_parts(b"d", &[b""]));
        assert_ne!(digest_parts(b"d", &[b"a", b"b"]), digest_parts(b"d", &[b"b", b"a"]));
        assert_ne!(digest_parts(b"d", &[b"a"]), digest_parts(b"d", &[b"a", b""]));
    }

    #[test]
    fn mixed_fixed_arrays_and_vecs_use_the_same_slice_api_as_the_daemon_calls() {
        let nonce = [7_u8; 32];
        let counter = 9_u64;
        let root = b"root".to_vec();
        let a = digest_parts(b"d", &[&nonce, &counter.to_be_bytes()]);
        assert_eq!(a, digest_parts(b"d", &[nonce.as_slice(), counter.to_be_bytes().as_slice()]));
        let b = digest_parts(b"d", &[&root, &counter.to_be_bytes()]);
        assert_eq!(b, digest_parts(b"d", &[root.as_slice(), counter.to_be_bytes().as_slice()]));
    }
}
