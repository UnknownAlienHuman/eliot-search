//! Qualified operating-system entropy shared by daemon-local capability owners.
//!
//! This is the sole daemon owner for direct OS CSPRNG access. Callers receive
//! exact random bytes or a closed failure; process identity, clocks and stored
//! tokens are never substituted for entropy.

#![allow(unsafe_code)]

#[cfg(windows)]
use core::ffi::c_void;

/// Stable code when qualified OS entropy cannot be read.
pub const QUALIFIED_ENTROPY_UNAVAILABLE: &str =
    "DIRECT_QUALIFIED_ENTROPY_UNAVAILABLE";

/// Reads exactly `output.len()` bytes from the qualified OS CSPRNG.
///
/// Unix reads the kernel CSPRNG; Windows uses `BCryptGenRandom` with the
/// system-preferred RNG. Empty requests, unsupported platforms and source
/// failures fail closed.
///
/// # Errors
///
/// Returns [`QUALIFIED_ENTROPY_UNAVAILABLE`] when the request is empty or the
/// operating-system source cannot fill the complete buffer.
pub fn fill_qualified_entropy(
    output: &mut [u8],
) -> Result<(), &'static str> {
    if output.is_empty() {
        return Err(QUALIFIED_ENTROPY_UNAVAILABLE);
    }
    platform_fill(output)
}

/// Reads 32 bytes of qualified OS entropy for opaque token material.
///
/// # Errors
///
/// Returns [`QUALIFIED_ENTROPY_UNAVAILABLE`] when the OS source cannot be read.
pub fn qualified_entropy_32() -> Result<[u8; 32], &'static str> {
    let mut output = [0_u8; 32];
    fill_qualified_entropy(&mut output)?;
    Ok(output)
}

#[cfg(unix)]
fn platform_fill(bytes: &mut [u8]) -> Result<(), &'static str> {
    use std::io::Read as _;

    std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(bytes))
        .map_err(|_| QUALIFIED_ENTROPY_UNAVAILABLE)
}

#[cfg(windows)]
#[link(name = "Bcrypt")]
unsafe extern "system" {
    #[link_name = "BCryptGenRandom"]
    fn bcrypt_gen_random(
        algorithm: *mut c_void,
        buffer: *mut u8,
        buffer_bytes: u32,
        flags: u32,
    ) -> i32;
}

#[cfg(windows)]
fn platform_fill(bytes: &mut [u8]) -> Result<(), &'static str> {
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
    const STATUS_SUCCESS: i32 = 0;
    let length =
        u32::try_from(bytes.len()).map_err(|_| QUALIFIED_ENTROPY_UNAVAILABLE)?;
    // SAFETY: the system-preferred synchronous RNG fills exactly the checked
    // mutable slice or returns failure. The slice is alive and exclusively
    // borrowed for the duration of the call.
    let status = unsafe {
        bcrypt_gen_random(
            core::ptr::null_mut(),
            bytes.as_mut_ptr(),
            length,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status == STATUS_SUCCESS {
        Ok(())
    } else {
        Err(QUALIFIED_ENTROPY_UNAVAILABLE)
    }
}

#[cfg(not(any(unix, windows)))]
fn platform_fill(_bytes: &mut [u8]) -> Result<(), &'static str> {
    Err(QUALIFIED_ENTROPY_UNAVAILABLE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_entropy_request_fails_closed() {
        assert_eq!(
            fill_qualified_entropy(&mut []),
            Err(QUALIFIED_ENTROPY_UNAVAILABLE)
        );
    }

    #[test]
    fn qualified_entropy_fills_exact_bounded_buffers() {
        for length in [16_usize, 32] {
            let mut bytes = vec![0_u8; length];
            fill_qualified_entropy(&mut bytes).expect("qualified entropy");
            assert_eq!(bytes.len(), length);
        }
    }
}
