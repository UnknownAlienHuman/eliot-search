//! Disclosure fixtures for the one secret-derived contract digest.
//!
//! `HandleTokenDigest` keeps the same storage and decode surface as the
//! other digest-shaped contract values but emits constant redacted strings
//! from `Debug` and `Display`. These fixtures pin that guarantee, because the
//! shared macro used to give every digest value full hexadecimal output.

use search_contracts::HandleTokenDigest;
use std::str::FromStr;

/// Secret-shaped bytes that must never appear in formatted output.
const SECRET: [u8; 32] = [0x5a; 32];
const SECRET_HEX: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";

const fn secret_digest() -> HandleTokenDigest {
    HandleTokenDigest::from_stored_bytes(SECRET)
}

#[test]
fn debug_never_emits_secret_bytes() {
    let rendered = format!("{:?}", secret_digest());
    assert!(
        !rendered.contains(SECRET_HEX),
        "Debug disclosed the secret: {rendered}"
    );
    for byte in SECRET {
        assert!(
            !rendered.contains(&format!("{byte:02x}")),
            "Debug emitted secret byte {byte:02x}"
        );
    }
    assert!(rendered.contains("<redacted>"));
}

#[test]
fn display_never_emits_secret_bytes() {
    let rendered = secret_digest().to_string();
    assert_eq!(rendered, "<redacted>");
    assert!(!rendered.contains(SECRET_HEX));
}

#[test]
fn formatting_is_constant_regardless_of_secret() {
    // Redaction must not leak length, prefix or equality through formatting.
    let other = HandleTokenDigest::from_stored_bytes([0xff; 32]);
    assert_eq!(format!("{:?}", secret_digest()), format!("{other:?}"));
    assert_eq!(secret_digest().to_string(), other.to_string());
    // Distinct secrets remain unequal, so redaction changed nothing semantically.
    assert_ne!(secret_digest(), other);
}

#[test]
fn explicit_restore_reads_exact_stored_bytes() {
    // Restore is the only way exact bytes become available, and it is exact.
    let restored = HandleTokenDigest::from_stored_bytes(SECRET);
    assert_eq!(restored.as_bytes(), &SECRET);

    let parsed = HandleTokenDigest::parse_hex(SECRET_HEX).expect("hex restore");
    assert_eq!(parsed.as_bytes(), &SECRET);
    assert_eq!(parsed, restored);

    assert_eq!(
        HandleTokenDigest::from_str(SECRET_HEX).expect("FromStr restore"),
        restored
    );
}

#[test]
fn decode_surface_stays_strict_and_length_bounded() {
    assert!(HandleTokenDigest::parse_hex(&"5a".repeat(31)).is_err());
    assert!(HandleTokenDigest::parse_hex(&"5a".repeat(33)).is_err());
    assert!(HandleTokenDigest::parse_hex(&"AA".repeat(32)).is_err());
    assert!(HandleTokenDigest::parse_hex(&"zz".repeat(32)).is_err());
}
