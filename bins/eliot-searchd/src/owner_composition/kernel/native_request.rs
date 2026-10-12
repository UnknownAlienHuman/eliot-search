//! Original native root request and cumulative admission work.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_contracts::{
    BoundedBytes, CanonicalDigestDomain, CanonicalValue, DigestInputLimit, RequestId,
    blake3_canonical_array_with_usage,
};
use search_runtime_owner::{DataRootOpenMode, OwnerError};

/// Caller-selected positive limits within this native profile's hard ceilings.
#[derive(Clone, Copy)]
pub struct NativeRootLimits {
    arguments: usize,
    input_bytes: usize,
    encoded_bytes: usize,
    read_bytes: usize,
    native_calls: usize,
    opens: usize,
}

impl NativeRootLimits {
    pub fn new(values: [usize; 6]) -> Result<Self, OwnerError> {
        let maxima = [256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256];
        if values
            .iter()
            .zip(maxima)
            .any(|(value, max)| *value == 0 || *value > max)
        {
            return Err(OwnerError::ContractExhausted);
        }
        let [
            arguments,
            input_bytes,
            encoded_bytes,
            read_bytes,
            native_calls,
            opens,
        ] = values;
        Ok(Self {
            arguments,
            input_bytes,
            encoded_bytes,
            read_bytes,
            native_calls,
            opens,
        })
    }
}

/// A request is consumed once by admission. It grants no native authority.
pub struct NativeRootRequest {
    pub(super) mode: DataRootOpenMode,
    root: PathBuf,
    deadline: Instant,
    cancellation: Arc<AtomicBool>,
    id: RequestId,
    digest: [u8; 32],
    digest_bytes: usize,
    // Native argv survives the caller; it is never logged or reinterpreted.
    arguments: Vec<Vec<u8>>,
    pub(super) limits: NativeRootLimits,
}

impl core::fmt::Debug for NativeRootRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("NativeRootRequest(<opaque>)")
    }
}

impl NativeRootRequest {
    /// Retains the complete native invocation while selecting the service root
    /// through the existing configuration argument parser. Admission stays read-only.
    pub(crate) fn for_service(
        arguments: &[OsString],
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
        limits: NativeRootLimits,
    ) -> Result<Self, OwnerError> {
        // The existing constructor applies count/native-byte/hash bounds before
        // the configuration parser can traverse or copy the invocation.
        let mut request = Self::new(
            DataRootOpenMode::InspectExistingReadOnly,
            arguments,
            deadline,
            cancellation,
            limits,
        )?;
        let (selected, _) = crate::config_composition::strip_config_args_os(arguments)
            .map_err(|_| OwnerError::OwnerOperationConflict)?;
        match selected.as_slice() {
            [command, root] if command == "--serve-data-root" && !root.is_empty() => {
                request.root = PathBuf::from(root);
            }
            _ => return Err(OwnerError::OwnerOperationConflict),
        }
        request.check()?;
        Ok(request)
    }

    /// A separately admitted service command has its own original context and
    /// finite meter. It retains the startup argv/root; it cannot renew startup
    /// or any unfinished command and grants no native authority on its own.
    pub(super) fn service_command(
        &self,
        command: &str,
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
    ) -> Result<Self, OwnerError> {
        check_context(deadline, &cancellation)?;
        if self.arguments.len() >= self.limits.arguments {
            return Err(OwnerError::ContractExhausted);
        }
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(self.arguments.len() + 1)
            .map_err(|_| OwnerError::ContractExhausted)?;
        for argument in &self.arguments {
            check_context(deadline, &cancellation)?;
            arguments.push(restore_native_argument(argument)?);
        }
        arguments.push(OsString::from(command));
        let mut request = Self::new(
            DataRootOpenMode::InspectExistingReadOnly,
            &arguments,
            deadline,
            cancellation,
            self.limits,
        )?;
        request.root.clone_from(&self.root);
        request.check()?;
        Ok(request)
    }

    /// Refusal binds exact transport bytes separately from native argv admission.
    /// Its private caller emits only a closed error; this grants no root authority.
    pub(super) fn service_refusal(
        &self,
        command: &str,
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
    ) -> Result<Self, OwnerError> {
        check_context(deadline, &cancellation)?;
        if command.len() > 256 * 1024 {
            return Err(OwnerError::ContractExhausted);
        }
        let domain = CanonicalDigestDomain::parse("eliot/cbor/native-service-refusal/v1")
            .map_err(|_| OwnerError::DataRootInvalid)?;
        let rows = [
            self.id.as_bytes().as_slice(),
            self.digest.as_slice(),
            command.as_bytes(),
        ]
        .into_iter()
        .map(|bytes| BoundedBytes::new(bytes.to_vec()).map(CanonicalValue::Bytes));
        let output = blake3_canonical_array_with_usage(
            &domain,
            3,
            rows,
            DigestInputLimit::new(self.limits.encoded_bytes)
                .map_err(|_| OwnerError::ContractExhausted)?,
        )
        .map_err(|_| OwnerError::ContractExhausted)?;
        check_context(deadline, &cancellation)?;
        let mut id = [0; 16];
        crate::qualified_entropy::fill_qualified_entropy(&mut id)
            .map_err(|_| OwnerError::OwnerIdentityAmbiguous)?;
        if id == [0; 16] {
            return Err(OwnerError::OwnerIdentityAmbiguous);
        }
        let request = Self {
            mode: DataRootOpenMode::InspectExistingReadOnly,
            root: self.root.clone(),
            deadline,
            cancellation,
            id: RequestId::from_bytes(id),
            digest: *output.digest().as_bytes(),
            digest_bytes: output.complete_preimage_bytes(),
            arguments: self.arguments.clone(),
            limits: self.limits,
        };
        request.check()?;
        Ok(request)
    }

    pub fn new(
        mode: DataRootOpenMode,
        arguments: &[OsString],
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
        limits: NativeRootLimits,
    ) -> Result<Self, OwnerError> {
        check_context(deadline, &cancellation)?;
        if arguments.len() < 2
            || arguments.len() > limits.arguments
            || arguments[0].is_empty()
            || arguments[1].is_empty()
        {
            return Err(OwnerError::OwnerOperationConflict);
        }
        let mut native = Vec::new();
        native
            .try_reserve_exact(arguments.len())
            .map_err(|_| OwnerError::ContractExhausted)?;
        let mut remaining = limits.input_bytes;
        for argument in arguments {
            check_context(deadline, &cancellation)?;
            let bytes = native_argument(argument, remaining)?;
            remaining -= bytes.len();
            native.push(bytes);
        }
        let mode_tag = match mode {
            DataRootOpenMode::InspectExistingReadOnly => 0,
            DataRootOpenMode::OpenExistingMutating => 1,
            DataRootOpenMode::InitializeNew => 2,
            DataRootOpenMode::OpenNamedRecovery => 3,
        };
        let domain = CanonicalDigestDomain::parse(if cfg!(windows) {
            "eliot/cbor/native-root-request/windows/v1"
        } else {
            "eliot/cbor/native-root-request/unix/v1"
        })
        .map_err(|_| OwnerError::DataRootInvalid)?;
        let rows = std::iter::once(Ok(CanonicalValue::U64(mode_tag))).chain(
            native
                .iter()
                .map(|bytes| BoundedBytes::new(bytes.clone()).map(CanonicalValue::Bytes)),
        );
        let output = blake3_canonical_array_with_usage(
            &domain,
            native.len() + 1,
            rows,
            DigestInputLimit::new(limits.encoded_bytes)
                .map_err(|_| OwnerError::ContractExhausted)?,
        )
        .map_err(|_| OwnerError::ContractExhausted)?;
        check_context(deadline, &cancellation)?;
        let mut id = [0; 16];
        crate::qualified_entropy::fill_qualified_entropy(&mut id)
            .map_err(|_| OwnerError::OwnerIdentityAmbiguous)?;
        check_context(deadline, &cancellation)?;
        if id == [0; 16] {
            return Err(OwnerError::OwnerIdentityAmbiguous);
        }
        Ok(Self {
            mode,
            root: PathBuf::from(&arguments[1]),
            deadline,
            cancellation,
            id: RequestId::from_bytes(id),
            digest: *output.digest().as_bytes(),
            digest_bytes: output.complete_preimage_bytes(),
            arguments: native,
            limits,
        })
    }

    pub(super) fn check(&self) -> Result<(), OwnerError> {
        check_context(self.deadline, &self.cancellation)
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn arguments(&self) -> &[Vec<u8>] {
        &self.arguments
    }

    pub(super) fn credential_context(&self) -> (Instant, &AtomicBool) {
        (self.deadline, &self.cancellation)
    }

    pub(super) const fn binding(&self) -> (RequestId, [u8; 32]) {
        (self.id, self.digest)
    }

    /// Bounded terminal output retains the original context and cancels on I/O loss.
    pub fn emit<W: std::io::Write>(&self, writer: &mut W, line: &str) -> Result<(), OwnerError> {
        self.check()?;
        if line.len() > 1024 {
            return Err(OwnerError::ContractExhausted);
        }
        let mut attempted = false;
        let result = (|| {
            for part in [line.as_bytes(), b"\n"] {
                let mut remaining = part;
                while !remaining.is_empty() {
                    self.check()?;
                    attempted = true;
                    let count = writer
                        .write(remaining)
                        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
                    self.check()?;
                    if count == 0 || count > remaining.len() {
                        return Err(OwnerError::OwnerAcquireOutcomeUnknown);
                    }
                    remaining = &remaining[count..];
                }
            }
            self.check()?;
            writer
                .flush()
                .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
            self.check()
        })();
        if let Err(error) = result {
            self.cancellation.store(true, Ordering::Release);
            return Err(if attempted {
                OwnerError::OwnerAcquireOutcomeUnknown
            } else {
                error
            });
        }
        Ok(())
    }
}

fn check_context(deadline: Instant, cancellation: &AtomicBool) -> Result<(), OwnerError> {
    if cancellation.load(Ordering::Acquire) {
        return Err(OwnerError::OwnerCancelledBeforeMutation);
    }
    if Instant::now() >= deadline {
        return Err(OwnerError::OwnerDeadlineInvalid);
    }
    Ok(())
}

fn restore_native_argument(bytes: &[u8]) -> Result<OsString, OwnerError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        if !bytes.len().is_multiple_of(2) {
            return Err(OwnerError::OwnerOperationConflict);
        }
        let units: Vec<_> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        Ok(OsString::from_wide(&units))
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(OsString::from_vec(bytes.to_vec()))
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = bytes;
        Err(OwnerError::DataRootInvalid)
    }
}

pub(super) fn native_argument(argument: &OsStr, ceiling: usize) -> Result<Vec<u8>, OwnerError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let units = argument.encode_wide().take(ceiling / 2 + 1).count();
        if units > ceiling / 2 {
            return Err(OwnerError::ContractExhausted);
        }
        Ok(argument.encode_wide().flat_map(u16::to_le_bytes).collect())
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if argument.as_bytes().len() > ceiling {
            return Err(OwnerError::ContractExhausted);
        }
        Ok(argument.as_bytes().to_vec())
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (argument, ceiling);
        Err(OwnerError::DataRootInvalid)
    }
}

/// Reservations bound requested I/O bytes, native calls and opens cumulatively.
/// No refunds, renewed contexts or stage resets. Blocking OS calls cooperate.
pub(super) struct NativeWork {
    reads: usize,
    calls: usize,
    opens: usize,
    canonical_bytes: usize,
    original_digest_charged: bool,
    #[cfg(test)]
    pub(super) phase_hook: Option<Box<dyn FnMut(&'static str)>>,
    #[cfg(test)]
    pub(super) usage_hook: Option<Box<dyn FnMut(&'static str, [usize; 4], usize)>>,
    #[cfg(test)]
    pub(super) credential_hook: Option<Box<dyn FnMut(bool, Instant, &AtomicBool)>>,
}

impl NativeWork {
    pub(super) const fn new() -> Self {
        Self {
            reads: 0,
            // The request already made exactly one qualified entropy call.
            calls: 1,
            opens: 0,
            canonical_bytes: 0,
            original_digest_charged: false,
            #[cfg(test)]
            phase_hook: None,
            #[cfg(test)]
            usage_hook: None,
            #[cfg(test)]
            credential_hook: None,
        }
    }

    // The mutable receiver is required by the private cfg(test) phase hook;
    // production retains the identical call shape and only polls the request.
    #[cfg_attr(
        not(test),
        allow(clippy::unused_self, clippy::needless_pass_by_ref_mut)
    )]
    pub(super) fn arrive(
        &mut self,
        request: &NativeRootRequest,
        phase: &'static str,
    ) -> Result<(), OwnerError> {
        request.check()?;
        #[cfg(test)]
        if let Some(hook) = &mut self.phase_hook {
            hook(phase);
        }
        #[cfg(not(test))]
        let _ = phase;
        #[cfg(test)]
        self.observe_usage(phase, 0);
        request.check()
    }

    pub(super) fn call<T>(
        &mut self,
        request: &NativeRootRequest,
        operation: impl FnOnce() -> Result<T, OwnerError>,
    ) -> Result<T, OwnerError> {
        request.check()?;
        charge(&mut self.calls, 1, request.limits.native_calls)?;
        let result = operation();
        request.check()?;
        result
    }

    pub(super) fn open(&mut self, request: &NativeRootRequest) -> Result<(), OwnerError> {
        request.check()?;
        charge(&mut self.opens, 1, request.limits.opens)
    }

    pub(super) fn read(
        &mut self,
        request: &NativeRootRequest,
        bytes: usize,
    ) -> Result<(), OwnerError> {
        request.check()?;
        charge(&mut self.reads, bytes, request.limits.read_bytes)
    }

    pub(super) fn read_capacity(
        &self,
        request: &NativeRootRequest,
        bytes: usize,
    ) -> Result<(), OwnerError> {
        request.check()?;
        if self
            .reads
            .checked_add(bytes)
            .is_none_or(|total| total > request.limits.read_bytes)
        {
            return Err(OwnerError::ContractExhausted);
        }
        Ok(())
    }

    /// Initialization charges requested write attempts to the same I/O ceiling.
    pub(super) fn write(
        &mut self,
        request: &NativeRootRequest,
        bytes: usize,
    ) -> Result<(), OwnerError> {
        #[cfg(test)]
        self.observe_usage("write-reservation", bytes);
        self.read(request, bytes)
    }

    #[cfg(test)]
    fn observe_usage(&mut self, phase: &'static str, requested: usize) {
        if let Some(hook) = &mut self.usage_hook {
            hook(
                phase,
                [self.reads, self.calls, self.opens, self.canonical_bytes],
                requested,
            );
        }
    }

    /// Complete canonical hash preimages, including the retained request once.
    pub(super) fn canonical(
        &mut self,
        request: &NativeRootRequest,
        bytes: usize,
    ) -> Result<(), OwnerError> {
        request.check()?;
        let ceiling = request
            .limits
            .encoded_bytes
            .checked_mul(8)
            .ok_or(OwnerError::ContractExhausted)?
            .min(4 * 1024 * 1024);
        if !self.original_digest_charged {
            charge(&mut self.canonical_bytes, request.digest_bytes, ceiling)?;
            self.original_digest_charged = true;
        }
        charge(&mut self.canonical_bytes, bytes, ceiling)
    }
}

fn charge(total: &mut usize, amount: usize, ceiling: usize) -> Result<(), OwnerError> {
    let next = total
        .checked_add(amount)
        .ok_or(OwnerError::ContractExhausted)?;
    if next > ceiling {
        return Err(OwnerError::ContractExhausted);
    }
    *total = next;
    Ok(())
}
