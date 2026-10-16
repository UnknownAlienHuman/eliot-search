//! Native DPAPI execution and allocation ownership.

use super::{
    DpapiError, LegacyRevisionDpapiError, ProtectedSecret, ProtectionScope,
    SecretBytes,
};
use crate::RevisionDekContext;

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

#[path = "dpapi/context.rs"]
mod context;
pub use context::{
    ContextualLegacyRevisionDpapiError, ContextualLegacyRevisionDpapiResult,
    LegacyRevisionDpapiContext, LegacyRevisionDpapiWork,
};

/// Protects frozen inner-envelope bytes under the original caller context.
///
/// Native output capacity is charged after unchanged 65MiB validation and before
/// copying. No credential, envelope, source policy or persistence is introduced.
pub fn protect_legacy_revision_current_user_with_context<C: LegacyRevisionDpapiContext>(
    context: &mut C,
    input: &mut [u8],
    entropy: &[u8; 32],
) -> ContextualLegacyRevisionDpapiResult<Vec<u8>, C::Error> {
    #[cfg(windows)]
    {
        windows::legacy_revision_with_context(context, input, entropy, true)
    }
    #[cfg(not(windows))]
    {
        let _ = (input, entropy);
        context::stage(context, || {
            Err(LegacyRevisionDpapiError::UnsupportedPlatform)
        })
        .map_err(Into::into)
    }
}

/// Decrypts frozen ciphertext under the original caller context.
///
/// The result remains untrusted inner-envelope bytes; the caller must verify the
/// existing bindings/digest and precharge its own subsequent plaintext copy.
pub fn unprotect_legacy_revision_current_user_with_context<C: LegacyRevisionDpapiContext>(
    context: &mut C,
    input: &mut [u8],
    entropy: &[u8; 32],
) -> ContextualLegacyRevisionDpapiResult<Vec<u8>, C::Error> {
    #[cfg(windows)]
    {
        windows::legacy_revision_with_context(context, input, entropy, false)
    }
    #[cfg(not(windows))]
    {
        let _ = (input, entropy);
        context::stage(context, || {
            Err(LegacyRevisionDpapiError::UnsupportedPlatform)
        })
        .map_err(Into::into)
    }
}

/// Protects one fresh revision DEK through the shared contextual DPAPI kernel.
pub(crate) fn protect_revision_dek_with_context<C: RevisionDekContext>(
    context: &mut C,
    key: &[u8; 32],
    entropy: &[u8],
) -> ContextualLegacyRevisionDpapiResult<Vec<u8>, C::Error> {
    #[cfg(windows)]
    {
        windows::revision_dek_with_context(context, key, entropy, true)
    }
    #[cfg(not(windows))]
    {
        let _ = (key, entropy);
        context::stage(context, || {
            Err(LegacyRevisionDpapiError::UnsupportedPlatform)
        })
        .map_err(Into::into)
    }
}

/// Unprotects one stored revision DEK through the shared contextual DPAPI kernel.
pub(crate) fn unprotect_revision_dek_with_context<C: RevisionDekContext>(
    context: &mut C,
    protected: &[u8],
    entropy: &[u8],
) -> ContextualLegacyRevisionDpapiResult<Vec<u8>, C::Error> {
    #[cfg(windows)]
    {
        windows::revision_dek_with_context(context, protected, entropy, false)
    }
    #[cfg(not(windows))]
    {
        let _ = (protected, entropy);
        context::stage(context, || {
            Err(LegacyRevisionDpapiError::UnsupportedPlatform)
        })
        .map_err(Into::into)
    }
}

#[cfg(windows)]
mod windows {
    use super::context::{self, CopiedBytes, KernelError, LegacyContext};
    use super::{ContextualLegacyRevisionDpapiResult, LegacyRevisionDpapiContext};
    use crate::RevisionDekContext;
    use core::ffi::c_void;
    use core::ptr::{null, null_mut};
    use core::slice;

    use super::super::model::{clear_bytes, validate_secret};
    use super::super::{
        DpapiError, LegacyRevisionDpapiError, MAX_LEGACY_REVISION_DPAPI_BYTES, MAX_PROTECTED_BYTES,
        MAX_SECRET_BYTES, PROTECTED_SECRET_VERSION, ProtectedSecret, ProtectionScope, SecretBytes,
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

        fn copy_out<C: LegacyRevisionDpapiContext>(
            &self,
            c: &mut C,
            contextual: bool,
        ) -> Result<CopiedBytes, KernelError<RawDpapiError, C::Error>> {
            let length = context::stage(c, || {
                let length = usize::try_from(self.blob.size)
                    .map_err(|_| RawDpapiError::Output(NativeOutputError::LengthOverflow))?;
                if self.blob.data.is_null() || length == 0 || length > self.maximum {
                    return Err(RawDpapiError::Output(NativeOutputError::Invalid));
                }
                Ok(length)
            })?;
            // SAFETY: exact reported length is validated above while this
            // owner retains the native allocation through explicit finish.
            // copy debits actual accepted capacity BEFORE allocation/copy.
            let bytes = unsafe { slice::from_raw_parts(self.blob.data, length) };
            context::copy(c, bytes, contextual)
        }

        fn finish<C: LegacyRevisionDpapiContext>(
            self,
            c: &mut C,
            result: Result<CopiedBytes, KernelError<RawDpapiError, C::Error>>,
        ) -> Result<CopiedBytes, KernelError<RawDpapiError, C::Error>> {
            // Metadata alone determines mandatory bounded clearing, including
            // native-error and post-refusal paths. An oversized invalid result
            // keeps the accepted free-only policy; it grants no unbounded scan.
            let reported = usize::try_from(self.blob.size).unwrap_or(0);
            let actual = if !self.blob.data.is_null() && reported <= self.maximum {
                reported
            } else {
                0
            };
            context::finish_native_clear(c, actual, result, || drop(self))
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
        fn blob(&mut self) -> Result<DataBlob, RawDpapiError> {
            let size = u32::try_from(self.0.len()).map_err(|_| RawDpapiError::InputTooLarge)?;
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
        let size = u32::try_from(bytes.len()).map_err(|_| RawDpapiError::InputTooLarge)?;
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

    fn run_bytes<C: LegacyRevisionDpapiContext>(
        c: &mut C,
        input: &[u8],
        entropy: &[u8],
        maximum_input: usize,
        maximum_output: usize,
        protecting: bool,
        contextual: bool,
    ) -> Result<Vec<u8>, KernelError<RawDpapiError, C::Error>> {
        let input = context::stage(c, || input_blob(input, maximum_input))?;
        context::reserve(c, !protecting)?;
        let copied = context::copy(c, entropy, contextual);
        let copied = context::after(c, copied)?;
        let mut entropy = EntropyCopy(copied.take());
        let mut output = NativeOutput::new(maximum_output);
        let mut description = NativeDescription::new();
        let result = (|| {
            let entropy_blob = context::stage(c, || entropy.blob())?;
            context::stage(c, || {
                // SAFETY: borrowed input and owned entropy remain live; output
                // and description are initialized native-allocation guards.
                let succeeded = unsafe {
                    if protecting {
                        CryptProtectData(
                            &raw const input,
                            null(),
                            &raw const entropy_blob,
                            null_mut(),
                            null_mut(),
                            CRYPTPROTECT_UI_FORBIDDEN,
                            output.as_mut_ptr(),
                        )
                    } else {
                        CryptUnprotectData(
                            &raw const input,
                            description.as_mut_ptr(),
                            &raw const entropy_blob,
                            null_mut(),
                            null_mut(),
                            CRYPTPROTECT_UI_FORBIDDEN,
                            output.as_mut_ptr(),
                        )
                    }
                };
                if succeeded == 0 {
                    // The possible GetLastError attempt was reserved BEFORE
                    // dispatch; capture immediately before any callback/poll.
                    return Err(RawDpapiError::Platform(platform_error()));
                }
                Ok(())
            })?;
            output.copy_out(c, contextual)
        })();
        // Explicit finish owns every ordinary Result path after native/copy:
        // account actual clear first, then clear/free regardless of refusal.
        // Drop remains the resource-safety fallback for unwinding.
        let result = output.finish(c, result);
        // Preserve output -> description -> entropy -> final checkpoint order.
        drop(description);
        drop(entropy);
        context::after(c, result).map(CopiedBytes::take)
    }
    fn protect_bytes(
        input: &[u8],
        entropy: &[u8],
        maximum_input: usize,
        maximum_output: usize,
    ) -> Result<Vec<u8>, RawDpapiError> {
        context::legacy(run_bytes(
            &mut LegacyContext,
            input,
            entropy,
            maximum_input,
            maximum_output,
            true,
            false,
        ))
    }
    fn unprotect_bytes(
        input: &[u8],
        entropy: &[u8],
        maximum_input: usize,
        maximum_output: usize,
    ) -> Result<Vec<u8>, RawDpapiError> {
        context::legacy(run_bytes(
            &mut LegacyContext,
            input,
            entropy,
            maximum_input,
            maximum_output,
            false,
            false,
        ))
    }
    pub(super) fn legacy_revision_with_context<C: LegacyRevisionDpapiContext>(
        c: &mut C,
        input: &mut [u8],
        entropy: &[u8; 32],
        protecting: bool,
    ) -> ContextualLegacyRevisionDpapiResult<Vec<u8>, C::Error> {
        run_bytes(
            c,
            input,
            entropy,
            MAX_LEGACY_REVISION_DPAPI_BYTES,
            MAX_LEGACY_REVISION_DPAPI_BYTES,
            protecting,
            true,
        )
        .map_err(|error| error.map_object(|e| map_legacy_error(e, protecting)).into())
    }

    pub(super) fn revision_dek_with_context<C: RevisionDekContext>(
        context: &mut C,
        input: &[u8],
        entropy: &[u8],
        protecting: bool,
    ) -> ContextualLegacyRevisionDpapiResult<Vec<u8>, C::Error> {
        run_bytes(
            context,
            input,
            entropy,
            if protecting { 32 } else { search_os_secrets::REVISION_DEK_MAX_PROTECTED_BYTES },
            // The existing short-secret ceiling admits bounded cleanup of an
            // unexpected plaintext shape; the candidate owner still requires 32.
            if protecting { search_os_secrets::REVISION_DEK_MAX_PROTECTED_BYTES } else { MAX_SECRET_BYTES },
            protecting,
            true,
        )
        .map_err(|error| error.map_object(|e| map_legacy_error(e, protecting)).into())
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
            RawDpapiError::Output(NativeOutputError::Invalid) => DpapiError::InvalidPlatformOutput,
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

    fn map_legacy_error(error: RawDpapiError, protecting: bool) -> LegacyRevisionDpapiError {
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
            RawDpapiError::Platform(code) => LegacyRevisionDpapiError::UnprotectFailed(code),
        }
    }
}
