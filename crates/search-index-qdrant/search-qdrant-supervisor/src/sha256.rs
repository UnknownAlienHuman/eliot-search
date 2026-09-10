//! Minimal SHA-256 for executable identity verification.
//!
//! The supervisor closure has no hashing dependency (its `Cargo.toml` is
//! frozen), so file identity is computed here in pure safe Rust. The output
//! is verified against NIST vectors in-module. Only the digest comparison
//! against the pinned qualification manifest grants identity; this module
//! never decides trust itself.

use std::io::Read as _;
use std::path::Path;

const BLOCK_BYTES: usize = 64;
const STATE_WORDS: usize = 8;

const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

const INITIAL: [u32; STATE_WORDS] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

#[derive(Clone, Debug)]
struct Sha256 {
    state: [u32; STATE_WORDS],
    buffer: [u8; BLOCK_BYTES],
    buffered: usize,
    total_len: u64,
}

impl Sha256 {
    const fn new() -> Self {
        Self {
            state: INITIAL,
            buffer: [0; BLOCK_BYTES],
            buffered: 0,
            total_len: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);
        if self.buffered > 0 {
            let room = BLOCK_BYTES - self.buffered;
            let take = room.min(data.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&data[..take]);
            self.buffered += take;
            data = &data[take..];
            if self.buffered == BLOCK_BYTES {
                let block = self.buffer;
                compress(&mut self.state, &block);
                self.buffered = 0;
            }
        }
        while data.len() >= BLOCK_BYTES {
            let mut block = [0_u8; BLOCK_BYTES];
            block.copy_from_slice(&data[..BLOCK_BYTES]);
            compress(&mut self.state, &block);
            data = &data[BLOCK_BYTES..];
        }
        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffered = data.len();
        }
    }

    fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_len.wrapping_mul(8);
        // 0x80 followed by zeros until 56 buffered bytes, then the length.
        // `total_len` is no longer read after this point, so reusing
        // `update` for padding is exact.
        let pad_len = if self.buffered < 56 {
            56 - self.buffered
        } else {
            BLOCK_BYTES + 56 - self.buffered
        };
        let mut padding = [0_u8; BLOCK_BYTES];
        padding[0] = 0x80;
        self.update(&padding[..pad_len]);
        debug_assert_eq!(self.buffered, 56);
        self.update(&bit_len.to_be_bytes());
        debug_assert_eq!(self.buffered, 0);
        let mut digest = [0_u8; 32];
        for (index, word) in self.state.iter().enumerate() {
            let start = index * 4;
            digest[start..start + 4].copy_from_slice(&word.to_be_bytes());
        }
        digest
    }
}

fn compress(state: &mut [u32; STATE_WORDS], block: &[u8; BLOCK_BYTES]) {
    let mut schedule = [0_u32; 64];
    for (index, word) in schedule.iter_mut().enumerate().take(16) {
        let base = index * 4;
        *word = u32::from_be_bytes([
            block[base],
            block[base + 1],
            block[base + 2],
            block[base + 3],
        ]);
    }
    for index in 16..64 {
        let word2 = schedule[index - 2];
        let small_sigma1 = word2.rotate_right(17) ^ word2.rotate_right(19) ^ (word2 >> 10);
        let word15 = schedule[index - 15];
        let small_sigma0 = word15.rotate_right(7) ^ word15.rotate_right(18) ^ (word15 >> 3);
        schedule[index] = schedule[index - 16]
            .wrapping_add(small_sigma0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(small_sigma1);
    }
    let mut working = *state;
    for index in 0..64 {
        let big_sigma1 =
            working[4].rotate_right(6) ^ working[4].rotate_right(11) ^ working[4].rotate_right(25);
        let choice = (working[4] & working[5]) ^ ((!working[4]) & working[6]);
        let temp1 = working[7]
            .wrapping_add(big_sigma1)
            .wrapping_add(choice)
            .wrapping_add(K[index])
            .wrapping_add(schedule[index]);
        let big_sigma0 =
            working[0].rotate_right(2) ^ working[0].rotate_right(13) ^ working[0].rotate_right(22);
        let majority =
            (working[0] & working[1]) ^ (working[0] & working[2]) ^ (working[1] & working[2]);
        let temp2 = big_sigma0.wrapping_add(majority);
        working[7] = working[6];
        working[6] = working[5];
        working[5] = working[4];
        working[4] = working[3].wrapping_add(temp1);
        working[3] = working[2];
        working[2] = working[1];
        working[1] = working[0];
        working[0] = temp1.wrapping_add(temp2);
    }
    for index in 0..STATE_WORDS {
        state[index] = state[index].wrapping_add(working[index]);
    }
}

/// Computes SHA-256 over an in-memory slice.
#[must_use]
pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize()
}

/// Streams SHA-256 over a file without loading it into memory.
pub fn sha256_file(path: &Path) -> Result<[u8; 32], std::io::Error> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::with_capacity(65_536, file);
    let mut hasher = Sha256::new();
    let mut chunk = vec![0_u8; 65_536];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        hasher.update(&chunk[..read]);
    }
    Ok(hasher.finalize())
}

/// Renders a digest as lowercase hexadecimal.
#[must_use]
pub fn hex_lower(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(64);
    for byte in digest {
        text.push(HEX[usize::from(byte >> 4)] as char);
        text.push(HEX[usize::from(byte & 0x0F)] as char);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{hex_lower, sha256_bytes};

    #[test]
    fn empty_input_matches_nist_vector() {
        assert_eq!(
            hex_lower(&sha256_bytes(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn abc_matches_nist_vector() {
        assert_eq!(
            hex_lower(&sha256_bytes(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn two_block_nist_vector_matches() {
        assert_eq!(
            hex_lower(&sha256_bytes(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn one_million_a_matches_reference() {
        let mut hasher = super::Sha256::new();
        let chunk = [b'a'; 10_000];
        for _ in 0..100 {
            hasher.update(&chunk);
        }
        assert_eq!(
            hex_lower(&hasher.finalize()),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn incremental_updates_match_single_shot() {
        let data: Vec<u8> = (0..5000_u32)
            .map(|value| u8::try_from(value % 251).unwrap())
            .collect();
        let mut hasher = super::Sha256::new();
        for piece in data.chunks(137) {
            hasher.update(piece);
        }
        assert_eq!(hasher.finalize(), sha256_bytes(&data));
    }
}
