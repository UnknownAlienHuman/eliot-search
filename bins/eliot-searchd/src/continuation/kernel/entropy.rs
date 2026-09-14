//! Qualified OS entropy for opaque continuation tokens.

#![allow(unsafe_code)]

#[cfg(windows)]
use core::ffi::c_void;

/// Stable code when qualified OS entropy cannot be read.
pub const QUALIFIED_ENTROPY_UNAVAILABLE: &str =
    "DIRECT_QUALIFIED_ENTROPY_UNAVAILABLE";

/// Reads 32 bytes of qualified OS entropy for opaque token material.
///
/// Unix reads the kernel CSPRNG; Windows uses `BCryptGenRandom` with the
/// system-preferred RNG. Unsupported platforms and source failures fail
/// closed; process identifiers or wall-clock data are never substituted.
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
fn fill_qualified_entropy(bytes: &mut [u8]) -> Result<(), &'static str> {
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
fn fill_qualified_entropy(bytes: &mut [u8]) -> Result<(), &'static str> {
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
fn fill_qualified_entropy(_bytes: &mut [u8]) -> Result<(), &'static str> {
    Err(QUALIFIED_ENTROPY_UNAVAILABLE)
}
