//! Package-local deterministic fingerprints.
//!
//! These fingerprints provide stable equality and replay identities. They are
//! deliberately not advertised as cryptographic signatures.

use search_contracts::Blake3Digest32;

/// Incremental domain-separated deterministic fingerprint builder.
#[derive(Clone, Debug)]
pub(crate) struct FingerprintBuilder {
    lanes: [u64; 4],
    bytes_seen: u64,
}

impl FingerprintBuilder {
    /// Starts a new domain-separated fingerprint.
    pub(crate) fn new(domain: &[u8]) -> Self {
        let mut builder = Self {
            lanes: [
                0xcbf2_9ce4_8422_2325,
                0x8422_2325_cbf2_9ce4,
                0x9e37_79b9_7f4a_7c15,
                0xc2b2_ae3d_27d4_eb4f,
            ],
            bytes_seen: 0,
        };
        builder.push_bytes(domain);
        builder
    }

    /// Adds a length-delimited byte sequence.
    pub(crate) fn push_bytes(&mut self, value: &[u8]) {
        self.mix(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
        self.mix(value);
    }

    /// Adds a UTF-8 string.
    pub(crate) fn push_text(&mut self, value: &str) {
        self.push_bytes(value.as_bytes());
    }

    /// Adds a digest.
    pub(crate) fn push_digest(&mut self, value: Blake3Digest32) {
        self.push_bytes(value.as_bytes());
    }

    /// Adds an unsigned integer.
    pub(crate) fn push_u64(&mut self, value: u64) {
        self.mix(&value.to_be_bytes());
    }

    /// Adds a signed integer.
    pub(crate) fn push_i64(&mut self, value: i64) {
        self.mix(&value.to_be_bytes());
    }

    /// Adds a Boolean.
    pub(crate) fn push_bool(&mut self, value: bool) {
        self.mix(&[u8::from(value)]);
    }

    /// Adds a finite floating-point value by exact IEEE-754 bits.
    pub(crate) fn push_f64(&mut self, value: f64) {
        self.mix(&value.to_bits().to_be_bytes());
    }

    /// Finishes as the shared 32-byte digest type.
    pub(crate) fn finish(mut self) -> Blake3Digest32 {
        self.mix(&self.bytes_seen.to_be_bytes());
        for round in 0_u32..8 {
            let snapshot = self.lanes;
            for lane in 0..4 {
                let next = snapshot[(lane + 1) % 4];
                self.lanes[lane] ^= next.rotate_left(7 + round + lane as u32);
                self.lanes[lane] = self.lanes[lane]
                    .wrapping_mul(0x9e37_79b1_85eb_ca87)
                    .rotate_left(11 + lane as u32 * 3);
            }
        }
        let mut output = [0_u8; 32];
        for (index, lane) in self.lanes.into_iter().enumerate() {
            output[index * 8..index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
        }
        Blake3Digest32::from_bytes(output)
    }

    fn mix(&mut self, bytes: &[u8]) {
        for (index, byte) in bytes.iter().copied().enumerate() {
            let lane = (self.bytes_seen as usize + index) % self.lanes.len();
            self.lanes[lane] ^= u64::from(byte);
            self.lanes[lane] = self.lanes[lane]
                .wrapping_mul(0x0000_0100_0000_01b3)
                .rotate_left(13 + lane as u32 * 5);
        }
        self.bytes_seen = self
            .bytes_seen
            .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
    }
}
