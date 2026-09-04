//! Exact SHA-256 through the Windows CNG provider.
//!
//! This module implements no hash primitive. Windows `bcrypt.dll` owns the
//! algorithm implementation; the adapter only validates finite input/output and
//! converts the exact 32-byte result to a stable lower-case hexadecimal form.

#![cfg_attr(not(windows), allow(dead_code))]

use core::fmt;

/// Exact SHA-256 output.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    /// Creates an exact digest from bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Exact digest bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Stable lower-case hexadecimal encoding.
    #[must_use]
    pub fn to_hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        output
    }

    /// Parses an exact lower-case hexadecimal SHA-256 value.
    pub fn from_hex(value: &str) -> Result<Self, DigestError> {
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(DigestError::InvalidHex);
        }
        let mut output = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            output[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
        }
        Ok(Self(output))
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

/// Closed digest adapter failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DigestError {
    /// Windows CNG is unavailable on the current platform.
    UnsupportedPlatform,
    /// Input length cannot be represented by the CNG call.
    InputTooLarge,
    /// CNG provider open failed.
    ProviderOpenFailed,
    /// CNG hash operation failed.
    HashFailed,
    /// Hexadecimal digest is malformed.
    InvalidHex,
}

impl DigestError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "SEALED_DIGEST_UNSUPPORTED_PLATFORM",
            Self::InputTooLarge => "SEALED_DIGEST_INPUT_TOO_LARGE",
            Self::ProviderOpenFailed => "SEALED_DIGEST_PROVIDER_OPEN_FAILED",
            Self::HashFailed => "SEALED_DIGEST_HASH_FAILED",
            Self::InvalidHex => "SEALED_DIGEST_HEX_INVALID",
        }
    }
}

impl fmt::Display for DigestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for DigestError {}

/// Hashes exact bytes using Windows CNG SHA-256.
pub fn sha256(bytes: &[u8]) -> Result<Sha256Digest, DigestError> {
    platform::sha256(bytes)
}

fn hex_nibble(byte: u8) -> Result<u8, DigestError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(DigestError::InvalidHex),
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{DigestError, Sha256Digest};

    pub(super) fn sha256(_bytes: &[u8]) -> Result<Sha256Digest, DigestError> {
        Err(DigestError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod platform {
    use super::{DigestError, Sha256Digest};
    use core::ffi::c_void;
    use core::ptr::{null, null_mut};

    type AlgorithmHandle = *mut c_void;
    type NtStatus = i32;

    const SHA256_ALGORITHM: [u16; 7] = [
        b'S' as u16,
        b'H' as u16,
        b'A' as u16,
        b'2' as u16,
        b'5' as u16,
        b'6' as u16,
        0,
    ];

    #[link(name = "Bcrypt")]
    unsafe extern "system" {
        fn BCryptOpenAlgorithmProvider(
            algorithm: *mut AlgorithmHandle,
            algorithm_id: *const u16,
            implementation: *const u16,
            flags: u32,
        ) -> NtStatus;

        fn BCryptCloseAlgorithmProvider(
            algorithm: AlgorithmHandle,
            flags: u32,
        ) -> NtStatus;

        fn BCryptHash(
            algorithm: AlgorithmHandle,
            secret: *mut u8,
            secret_bytes: u32,
            input: *mut u8,
            input_bytes: u32,
            output: *mut u8,
            output_bytes: u32,
        ) -> NtStatus;
    }

    struct Provider(AlgorithmHandle);

    impl Provider {
        fn open_sha256() -> Result<Self, DigestError> {
            let mut handle = null_mut();
            // SAFETY: `handle` is valid writable storage; both UTF-16 provider
            // pointers remain live for the duration of the call.
            let status = unsafe {
                BCryptOpenAlgorithmProvider(
                    &mut handle,
                    SHA256_ALGORITHM.as_ptr(),
                    null(),
                    0,
                )
            };
            if status < 0 || handle.is_null() {
                return Err(DigestError::ProviderOpenFailed);
            }
            Ok(Self(handle))
        }
    }

    impl Drop for Provider {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: the handle was returned by
                // `BCryptOpenAlgorithmProvider` and is closed exactly once.
                unsafe {
                    let _ = BCryptCloseAlgorithmProvider(self.0, 0);
                }
                self.0 = null_mut();
            }
        }
    }

    pub(super) fn sha256(bytes: &[u8]) -> Result<Sha256Digest, DigestError> {
        let input_bytes =
            u32::try_from(bytes.len()).map_err(|_| DigestError::InputTooLarge)?;
        let provider = Provider::open_sha256()?;
        let mut output = [0_u8; 32];
        // SAFETY: all pointers refer to live buffers of the exact lengths passed
        // to CNG. SHA-256 is an unkeyed hash, so the secret pointer is null.
        let status = unsafe {
            BCryptHash(
                provider.0,
                null_mut(),
                0,
                if bytes.is_empty() {
                    null_mut()
                } else {
                    bytes.as_ptr().cast_mut()
                },
                input_bytes,
                output.as_mut_ptr(),
                u32::try_from(output.len()).expect("SHA-256 output length fits u32"),
            )
        };
        if status < 0 {
            return Err(DigestError::HashFailed);
        }
        Ok(Sha256Digest::from_bytes(output))
    }
}
