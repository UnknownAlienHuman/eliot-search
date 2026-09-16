//! Qualified OS entropy for opaque continuation tokens.
//!
//! The daemon-wide native owner lives in `crate::qualified_entropy`; this
//! compatibility surface preserves the established continuation API without
//! creating a second CSPRNG implementation.

/// Stable code when qualified OS entropy cannot be read.
pub const QUALIFIED_ENTROPY_UNAVAILABLE: &str =
    crate::qualified_entropy::QUALIFIED_ENTROPY_UNAVAILABLE;

/// Reads 32 bytes of qualified OS entropy for opaque token material.
///
/// # Errors
///
/// Returns [`QUALIFIED_ENTROPY_UNAVAILABLE`] when the OS source cannot be read.
pub fn qualified_entropy_32() -> Result<[u8; 32], &'static str> {
    crate::qualified_entropy::qualified_entropy_32()
}
