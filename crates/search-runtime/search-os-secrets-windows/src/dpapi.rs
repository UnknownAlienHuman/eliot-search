//! Native DPAPI execution and allocation ownership.

use super::{
    DpapiError, LegacyRevisionDpapiError, ProtectedSecret, ProtectionScope,
    SecretBytes,
};

/// Protects a short secret for the current Windows user and exact authority scope.
#[cfg(windows)]
pub fn protect_current_user(
    secret: &SecretBytes,
    scope: &ProtectionScope,
) -> Result<ProtectedSecret, DpapiError> {
    windows::protect_short_secret(secret, scope)
}

/// Protects a short secret for the current Windows user and exact authority scope.
#[cfg(not(windows))]
pub fn protect_current_user(
    _secret: &SecretBytes,
    _scope: &ProtectionScope,
) -> Result<ProtectedSecret, DpapiError> {
    Err(DpapiError::UnsupportedPlatform)
}

/// Decrypts a short secret for the current Windows user and exact authority scope.
#[cfg(windows)]
pub fn unprotect_current_user(
    protected: &ProtectedSecret,
    scope: &ProtectionScope,
) -> Result<SecretBytes, DpapiError> {
    windows::unprotect_short_secret(protected, scope)
}

/// Decrypts a short secret for the current Windows user and exact authority scope.
#[cfg(not(windows))]
pub fn unprotect_current_user(
    _protected: &ProtectedSecret,
    _scope: &ProtectionScope,
) -> Result<SecretBytes, DpapiError> {
    Err(DpapiError::UnsupportedPlatform)
}

/// Protects one frozen legacy revision inner envelope with exact 32-byte entropy.
///
/// The returned vector contains only DPAPI ciphertext. The caller retains
/// ownership of envelope framing, persistence and source/revision policy.
#[cfg(windows)]
pub fn protect_legacy_revision_current_user(
    input: &mut [u8],
    entropy: &[u8; 32],
) -> Result<Vec<u8>, LegacyRevisionDpapiError> {
    windows::protect_legacy_revision(input, entropy)
}

/// Protects one frozen legacy revision inner envelope with exact 32-byte entropy.
#[cfg(not(windows))]
pub fn protect_legacy_revision_current_user(
    _input: &mut [u8],
    _entropy: &[u8; 32],
) -> Result<Vec<u8>, LegacyRevisionDpapiError> {
    Err(LegacyRevisionDpapiError::UnsupportedPlatform)
}

/// Decrypts one frozen legacy revision DPAPI ciphertext with exact 32-byte entropy.
///
/// The returned vector is untrusted inner-envelope bytes. The caller must still
/// validate the package-owned legacy inner binding and plaintext digest before
/// using any plaintext.
#[cfg(windows)]
pub fn unprotect_legacy_revision_current_user(
    input: &mut [u8],
    entropy: &[u8; 32],
) -> Result<Vec<u8>, LegacyRevisionDpapiError> {
    windows::unprotect_legacy_revision(input, entropy)
}

/// Decrypts one frozen legacy revision DPAPI ciphertext with exact 32-byte entropy.
#[cfg(not(windows))]
pub fn unprotect_legacy_revision_current_user(
    _input: &mut [u8],
    _entropy: &[u8; 32],
) -> Result<Vec<u8>, LegacyRevisionDpapiError> {
    Err(LegacyRevisionDpapiError::UnsupportedPlatform)
}

#[cfg(windows)]
mod windows {
    use core::ffi::c_void;
    use core::ptr::{null, null_mut};
    use core::slice;

    use super::super::model::{clear_bytes, validate_secret};
    use super::super::{
        DpapiError, LegacyRevisionDpapiError, MAX_LEGACY_REVISION_DPAPI_BYTES,
        MAX_PROTECTED_BYTES, MAX_SECRET_BYTES, PROTECTED_SECRET_VERSION,
        ProtectedSecret, ProtectionScope, SecretBytes,
    };

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x0000_0001;

    #[repr(C)]
    struct DataBlob {
        size: u32,
        data: *mut u8,
    }

    #[link(name = "Crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            input: *const DataBlob,
            description: *const u16,
            optional_entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            output: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            input: *const DataBlob,
            description: *mut *mut u16,
            optional_entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            output: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
        fn GetLastError() -> u32;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum NativeOutputError {
        LengthOverflow,
        Invalid,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RawDpapiError {
        InputTooLarge,
        Output(NativeOutputError),
        Platform(u32),
    }

    struct NativeOutput {
        blob: DataBlob,
        maximum: usize,
    }

    impl NativeOutput {
        const fn new(maximum: usize) -> Self {
            Self {
                blob: DataBlob {
                    size: 0,
                    data: null_mut(),
                },
                maximum,
            }
        }

        fn as_mut_ptr(&mut self) -> *mut DataBlob {
            &raw mut self.blob
        }

        fn into_vec(mut self) -> Result<Vec<u8>, NativeOutputError> {
            let length = usize::try_from(self.blob.size)
                .map_err(|_| NativeOutputError::LengthOverflow)?;
            if self.blob.data.is_null() || length == 0 || length > self.maximum {
                return Err(NativeOutputError::Invalid);
            }
            // SAFETY: the non-null DPAPI allocation is live for its reported,
            // bounded length until this owner clears and frees it.
            let output = unsafe { slice::from_raw_parts(self.blob.data, length) }.to_vec();
            self.clear_and_free(length);
            Ok(output)
        }

        fn clear_and_free(&mut self, length: usize) {
            if self.blob.data.is_null() {
                return;
            }
            for offset in 0..length {
                // SAFETY: DPAPI returned a contiguous `length`-byte allocation.
                unsafe { core::ptr::write_volatile(self.blob.data.add(offset), 0) };
            }
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            // SAFETY: this owner releases the LocalAlloc allocation exactly once.
            let _ = unsafe { LocalFree(self.blob.data.cast()) };
            self.blob.data = null_mut();
            self.blob.size = 0;
        }
    }

    impl Drop for NativeOutput {
        fn drop(&mut self) {
            if self.blob.data.is_null() {
                return;
            }
            let reported = usize::try_from(self.blob.size).unwrap_or(0);
            if reported <= self.maximum {
                self.clear_and_free(reported);
            } else {
                // The reported length is outside the admitted bound. Avoid an
                // unbounded clear but still release the native allocation.
                // SAFETY: successful DPAPI output is LocalAlloc-owned.
                let _ = unsafe { LocalFree(self.blob.data.cast()) };
                self.blob.data = null_mut();
                self.blob.size = 0;
            }
        }
    }

    struct NativeDescription(*mut u16);

    impl NativeDescription {
        const fn new() -> Self {
            Self(null_mut())
        }

        fn as_mut_ptr(&mut self) -> *mut *mut u16 {
            &raw mut self.0
        }
    }

    impl Drop for NativeDescription {
        fn drop(&mut self) {
            if self.0.is_null() {
                return;
            }
            // SAFETY: DPAPI description output is LocalAlloc-owned and this
            // guard releases the pointer exactly once.
            let _ = unsafe { LocalFree(self.0.cast()) };
            self.0 = null_mut();
        }
    }

    struct EntropyCopy(Vec<u8>);

    impl EntropyCopy {
        fn new(bytes: &[u8]) -> Self {
            Self(bytes.to_vec())
        }

        fn blob(&mut self) -> Result<DataBlob, RawDpapiError> {
            let size = u32::try_from(self.0.len())
                .map_err(|_| RawDpapiError::InputTooLarge)?;
            Ok(DataBlob {
                size,
                data: self.0.as_mut_ptr(),
            })
        }
    }

    impl Drop for EntropyCopy {
        fn drop(&mut self) {
            clear_bytes(&mut self.0);
        }
    }

    fn input_blob(bytes: &[u8], maximum: usize) -> Result<DataBlob, RawDpapiError> {
        if bytes.len() > maximum {
            return Err(RawDpapiError::InputTooLarge);
        }
        let size = u32::try_from(bytes.len())
            .map_err(|_| RawDpapiError::InputTooLarge)?;
        Ok(DataBlob {
            size,
            data: bytes.as_ptr().cast_mut(),
        })
    }

    fn platform_error() -> u32 {
        // SAFETY: `GetLastError` has no preconditions and immediately captures
        // this thread's error after a failed DPAPI call.
        unsafe { GetLastError() }
    }

    fn protect_bytes(
        input: &[u8],
        entropy: &[u8],
        maximum_input: usize,
        maximum_output: usize,
    ) -> Result<Vec<u8>, RawDpapiError> {
        let input = input_blob(input, maximum_input)?;
        let mut entropy = EntropyCopy::new(entropy);
        let entropy_blob = entropy.blob()?;
        let mut output = NativeOutput::new(maximum_output);
        // SAFETY: input and entropy blobs point to live slices for the call;
        // output is initialized and becomes owned by `NativeOutput` on success.
        let succeeded = unsafe {
            CryptProtectData(
                &raw const input,
                null(),
                &raw const entropy_blob,
                null_mut(),
                null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                output.as_mut_ptr(),
            )
        };
        if succeeded == 0 {
            return Err(RawDpapiError::Platform(platform_error()));
        }
        output.into_vec().map_err(RawDpapiError::Output)
    }

    fn unprotect_bytes(
        input: &[u8],
        entropy: &[u8],
        maximum_input: usize,
        maximum_output: usize,
    ) -> Result<Vec<u8>, RawDpapiError> {
        let input = input_blob(input, maximum_input)?;
        let mut entropy = EntropyCopy::new(entropy);
        let entropy_blob = entropy.blob()?;
        let mut output = NativeOutput::new(maximum_output);
        let mut description = NativeDescription::new();
        // SAFETY: input and entropy blobs point to live slices for the call;
        // output/description become caller-owned LocalAlloc allocations on success.
        let succeeded = unsafe {
            CryptUnprotectData(
                &raw const input,
                description.as_mut_ptr(),
                &raw const entropy_blob,
                null_mut(),
                null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                output.as_mut_ptr(),
            )
        };
        if succeeded == 0 {
            return Err(RawDpapiError::Platform(platform_error()));
        }
        // No description was supplied during protection. Windows may still
        // return an allocation; `NativeDescription` releases it on every path.
        output.into_vec().map_err(RawDpapiError::Output)
    }

    pub(super) fn protect_short_secret(
        secret: &SecretBytes,
        scope: &ProtectionScope,
    ) -> Result<ProtectedSecret, DpapiError> {
        validate_secret(secret.expose_secret())?;
        let protected = protect_bytes(
            secret.expose_secret(),
            scope.as_entropy(),
            MAX_SECRET_BYTES,
            MAX_PROTECTED_BYTES,
        )
        .map_err(map_short_error)?;
        ProtectedSecret::from_bytes(PROTECTED_SECRET_VERSION, protected)
    }

    pub(super) fn unprotect_short_secret(
        protected: &ProtectedSecret,
        scope: &ProtectionScope,
    ) -> Result<SecretBytes, DpapiError> {
        let plaintext = unprotect_bytes(
            protected.as_bytes(),
            scope.as_entropy(),
            MAX_PROTECTED_BYTES,
            MAX_SECRET_BYTES,
        )
        .map_err(map_short_error)?;
        SecretBytes::new(plaintext)
    }

    fn map_short_error(error: RawDpapiError) -> DpapiError {
        match error {
            RawDpapiError::InputTooLarge
            | RawDpapiError::Output(NativeOutputError::LengthOverflow) => {
                DpapiError::LengthOverflow
            }
            RawDpapiError::Output(NativeOutputError::Invalid) => {
                DpapiError::InvalidPlatformOutput
            }
            RawDpapiError::Platform(code) => DpapiError::PlatformFailure(code),
        }
    }

    pub(super) fn protect_legacy_revision(
        input: &mut [u8],
        entropy: &[u8; 32],
    ) -> Result<Vec<u8>, LegacyRevisionDpapiError> {
        protect_bytes(
            input,
            entropy,
            MAX_LEGACY_REVISION_DPAPI_BYTES,
            MAX_LEGACY_REVISION_DPAPI_BYTES,
        )
        .map_err(|error| map_legacy_error(error, true))
    }

    pub(super) fn unprotect_legacy_revision(
        input: &mut [u8],
        entropy: &[u8; 32],
    ) -> Result<Vec<u8>, LegacyRevisionDpapiError> {
        unprotect_bytes(
            input,
            entropy,
            MAX_LEGACY_REVISION_DPAPI_BYTES,
            MAX_LEGACY_REVISION_DPAPI_BYTES,
        )
        .map_err(|error| map_legacy_error(error, false))
    }

    fn map_legacy_error(
        error: RawDpapiError,
        protecting: bool,
    ) -> LegacyRevisionDpapiError {
        match error {
            RawDpapiError::InputTooLarge => LegacyRevisionDpapiError::InputTooLarge,
            RawDpapiError::Output(NativeOutputError::LengthOverflow) => {
                LegacyRevisionDpapiError::OutputTooLarge
            }
            RawDpapiError::Output(NativeOutputError::Invalid) => {
                LegacyRevisionDpapiError::InvalidPlatformOutput
            }
            RawDpapiError::Platform(code) if protecting => {
                LegacyRevisionDpapiError::ProtectFailed(code)
            }
            RawDpapiError::Platform(code) => {
                LegacyRevisionDpapiError::UnprotectFailed(code)
            }
        }
    }
}
