//! Content-free legacy DIRECT preparation receipt.

/// Canonical preparation receipt carrying exact durable-object provenance.
///
/// The receipt carries recomputed representation and profile digests; it is not
/// an authorization receipt and contains no source bytes or paths.
pub struct LegacyDirectPreparationReceipt {
    /// BLAKE3 representation identity bound to source, body and profiles.
    pub representation_id: [u8; 32],
    /// Canonical materializer profile digest bytes.
    pub materializer_digest: [u8; 32],
    /// Canonical unitizer profile digest bytes.
    pub unitizer_digest: [u8; 32],
    /// Closed preparation gap, if the source is not layout-searchable.
    pub gap: Option<&'static str>,
}

impl LegacyDirectPreparationReceipt {
    /// Representation identity as lowercase hexadecimal.
    #[must_use]
    pub fn representation_hex(&self) -> String {
        const TABLE: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(64);
        for byte in self.representation_id {
            output.push(char::from(TABLE[usize::from(byte >> 4)]));
            output.push(char::from(TABLE[usize::from(byte & 0x0f)]));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representation_hex_is_exact_lowercase_and_content_free() {
        let receipt = LegacyDirectPreparationReceipt {
            representation_id: [0xab; 32],
            materializer_digest: [1; 32],
            unitizer_digest: [2; 32],
            gap: Some("DIRECT_REVISION_NOT_UTF8"),
        };
        assert_eq!(receipt.representation_hex(), "ab".repeat(32));
    }
}
