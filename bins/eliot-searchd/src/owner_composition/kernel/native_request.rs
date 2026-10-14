//! Original native root request and cumulative admission work.

use std::cell::{Cell, RefCell};
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
    terminal_work: Box<RefCell<Option<NativeWork>>>,
    terminal_refusal: Cell<Option<OwnerError>>,
}

impl core::fmt::Debug for NativeRootRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("NativeRootRequest(<opaque>)")
    }
}

impl NativeRootRequest {
    /// Retains native/config argv for the effect-free existing-mutation sync caller.
    pub(crate) fn for_source_sync(
        arguments: &[OsString],
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
        limits: NativeRootLimits,
    ) -> Result<Self, OwnerError> {
        let mut request = Self::new(
            DataRootOpenMode::OpenExistingMutating,
            arguments,
            deadline,
            cancellation,
            limits,
        )?;
        let (selected, _) = crate::config_composition::strip_config_args_os(arguments)
            .map_err(|_| OwnerError::OwnerOperationConflict)?;
        match selected.as_slice() {
            [command, root] if command == "--sync-source-roots" && !root.is_empty() => {
                request.root = PathBuf::from(root);
            }
            _ => return Err(OwnerError::OwnerOperationConflict),
        }
        request.check()?;
        Ok(request)
    }

    /// Closed validation output when no request could be constructed. It retains
    /// the caller's original deadline/flag/limits and mints no root authority.
    pub(crate) fn emit_validation_refusal(
        deadline: Instant,
        cancellation: &AtomicBool,
        limits: NativeRootLimits,
        writer: &mut impl std::io::Write,
        error: OwnerError,
    ) -> Result<(), OwnerError> {
        let line = format!("{{\"error\":\"{}\"}}\n", error.code());
        if line.len() > 1024 {
            return Err(OwnerError::ContractExhausted);
        }
        let mut attempted = false;
        let mut requested_bytes = 0;
        let mut calls = 1; // Reserve construction's at-most-one entropy seam.
        let result = (|| {
            let mut remaining = line.as_bytes();
            while !remaining.is_empty() {
                check_context(deadline, cancellation)?;
                charge(&mut requested_bytes, remaining.len(), limits.read_bytes)?;
                charge(&mut calls, 1, limits.native_calls)?;
                attempted = true;
                let count = writer
                    .write(remaining)
                    .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
                check_context(deadline, cancellation)?;
                if count == 0 || count > remaining.len() {
                    return Err(OwnerError::OwnerAcquireOutcomeUnknown);
                }
                remaining = &remaining[count..];
            }
            check_context(deadline, cancellation)?;
            charge(&mut calls, 1, limits.native_calls)?;
            attempted = true;
            writer
                .flush()
                .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
            check_context(deadline, cancellation)
        })();
        result.map_err(|error| {
            cancellation.store(true, Ordering::Release);
            if attempted {
                OwnerError::OwnerAcquireOutcomeUnknown
            } else {
                error
            }
        })
    }

    /// The paired catalog commands retain the complete native/config invocation.
    /// Selection does not publish owner state or grant mutation authority.
    pub(crate) fn for_source_registration(
        arguments: &[OsString],
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
        limits: NativeRootLimits,
    ) -> Result<(Self, Vec<OsString>), OwnerError> {
        let mut request = Self::new(
            DataRootOpenMode::OpenExistingMutating,
            arguments,
            deadline,
            cancellation,
            limits,
        )?;
        let (selected, _) = crate::config_composition::strip_config_args_os(arguments)
            .map_err(|_| OwnerError::OwnerOperationConflict)?;
        if selected.len() != 3
            || selected[1].is_empty()
            || !matches!(
                selected[0].to_str(),
                Some("--register-source-root" | "--unregister-source-root")
            )
        {
            return Err(OwnerError::OwnerOperationConflict);
        }
        request.root = PathBuf::from(&selected[1]);
        request.check()?;
        Ok((request, selected))
    }

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

    /// Retains the complete native/config invocation for the offline migration
    /// plan command under existing read-only custody. Selection is closed:
    /// command spelling, exact arity, non-empty root and a syntactically valid
    /// non-zero target namespace are refused before any root I/O.
    pub(crate) fn for_control_migration_plan(
        arguments: &[OsString],
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
        limits: NativeRootLimits,
    ) -> Result<(Self, Vec<OsString>), OwnerError> {
        Self::control_migration(
            "--plan-control-migration",
            arguments,
            deadline,
            cancellation,
            limits,
        )
    }

    /// The bootstrap sibling of the offline mapping plan. It admits exactly
    /// `--bootstrap-control-migration`, keeps the same existing read-only
    /// profile and closed selection, and retains the raw argv as supplied: the
    /// demanded flag is compared, never rewritten, so the raw-args digest,
    /// request and custody stay bound to the original invocation.
    pub(crate) fn for_control_migration_bootstrap(
        arguments: &[OsString],
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
        limits: NativeRootLimits,
    ) -> Result<(Self, Vec<OsString>), OwnerError> {
        Self::control_migration(
            "--bootstrap-control-migration",
            arguments,
            deadline,
            cancellation,
            limits,
        )
    }

    /// Shared private construction for the two offline migration commands.
    /// Only the demanded flag literal differs; the caller's raw argv, absolute
    /// deadline, cancellation handle and limits decide admission and custody
    /// identically for both.
    fn control_migration(
        command: &str,
        arguments: &[OsString],
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
        limits: NativeRootLimits,
    ) -> Result<(Self, Vec<OsString>), OwnerError> {
        let mut request = Self::new(
            DataRootOpenMode::InspectExistingReadOnly,
            arguments,
            deadline,
            cancellation,
            limits,
        )?;
        let (selected, _) = crate::config_composition::strip_config_args_os(arguments)
            .map_err(|_| OwnerError::OwnerOperationConflict)?;
        let [selected_command, root, target, output] = selected.as_slice() else {
            return Err(OwnerError::OwnerOperationConflict);
        };
        if selected_command != command || root.is_empty() || output.is_empty() {
            return Err(OwnerError::OwnerOperationConflict);
        }
        target
            .to_str()
            .and_then(|value| search_contracts::SourceNamespaceId::parse(value).ok())
            .filter(|value| value.as_bytes() != &[0; 16])
            .ok_or(OwnerError::OwnerOperationConflict)?;
        request.root = PathBuf::from(root);
        request.check()?;
        Ok((request, selected))
    }

    /// Retains all native argv, then selects a closed existing-only one-shot root.
    pub(crate) fn for_read_one_shot(
        arguments: &[OsString],
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
        limits: NativeRootLimits,
    ) -> Result<(Self, Vec<OsString>), OwnerError> {
        let mut request = Self::new(
            DataRootOpenMode::InspectExistingReadOnly,
            arguments,
            deadline,
            cancellation,
            limits,
        )?;
        let (selected, _) = crate::config_composition::strip_config_args_os(arguments)
            .map_err(|_| OwnerError::OwnerOperationConflict)?;
        let expected = match selected.first().and_then(|arg| arg.to_str()) {
            Some("--health-data-root" | "--list-sources" | "--verify-root" | "--source-roots") => 2,
            Some("--search-root" | "--search-root-ascii-insensitive") => 3,
            Some("--read-revision") => 5,
            _ => return Err(OwnerError::OwnerOperationConflict),
        };
        if selected.len() != expected || selected[1].is_empty() {
            return Err(OwnerError::OwnerOperationConflict);
        }
        request.root = PathBuf::from(&selected[1]);
        request.check()?;
        Ok((request, selected))
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
            terminal_work: Box::new(RefCell::new(None)),
            terminal_refusal: Cell::new(None),
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
            terminal_work: Box::new(RefCell::new(None)),
            terminal_refusal: Cell::new(None),
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

    pub(super) const fn inventory_limits(&self) -> [u64; 6] {
        [self.limits.arguments as u64, self.limits.input_bytes as u64,
            self.limits.encoded_bytes as u64, self.limits.read_bytes as u64,
            self.limits.native_calls as u64, self.limits.opens as u64]
    }

    pub(super) fn retain_refusal_work(self, work: NativeWork) -> Self {
        *self.terminal_work.borrow_mut() = Some(work);
        self
    }

    /// Refusal output retains original admission work and a sticky unknown outcome.
    pub fn emit<W: std::io::Write>(&self, writer: &mut W, line: &str) -> Result<(), OwnerError> {
        if let Some(error) = self.terminal_refusal.get() {
            return Err(error);
        }
        if let Err(error) = self.check() {
            self.terminal_refusal.set(Some(error));
            return Err(error);
        }
        if line.len() > 1024 {
            return Err(OwnerError::ContractExhausted);
        }
        let mut retained = self
            .terminal_work
            .try_borrow_mut()
            .map_err(|_| OwnerError::OwnerInvalidTransition)?;
        let work = retained.get_or_insert_with(NativeWork::new);
        self.emit_using_work(work, writer, line)
    }

    // Catalog validation refusals borrow the original admission meter. Public
    // refusal emission continues to retain its own supplied meter above.
    pub(super) fn emit_using_work(
        &self,
        work: &mut NativeWork,
        writer: &mut impl std::io::Write,
        line: &str,
    ) -> Result<(), OwnerError> {
        if let Some(error) = self.terminal_refusal.get() {
            return Err(error);
        }
        if let Err(error) = self.check() {
            self.terminal_refusal.set(Some(error));
            return Err(error);
        }
        if line.len() > 1024 {
            return Err(OwnerError::ContractExhausted);
        }
        let mut attempted = false;
        let result = (|| {
            work.canonical(self, 0)?;
            for part in [line.as_bytes(), b"\n"] {
                let mut remaining = part;
                while !remaining.is_empty() {
                    work.write(self, remaining.len())?;
                    let count = work.call(self, || {
                        attempted = true;
                        writer
                            .write(remaining)
                            .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
                    })?;
                    if count == 0 || count > remaining.len() {
                        return Err(OwnerError::OwnerAcquireOutcomeUnknown);
                    }
                    remaining = &remaining[count..];
                }
            }
            work.call(self, || {
                attempted = true;
                writer
                    .flush()
                    .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })?;
            self.check()
        })();
        if let Err(error) = result {
            self.cancellation.store(true, Ordering::Release);
            let error = if attempted {
                OwnerError::OwnerAcquireOutcomeUnknown
            } else {
                error
            };
            self.terminal_refusal.set(Some(error));
            return Err(error);
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

    /// Record an actual bounded DPAPI clear after ordinary work has refused.
    pub(super) const fn account_native_clear(
        &mut self,
        request: &NativeRootRequest,
        actual_bytes: usize,
    ) -> Result<(), OwnerError> {
        let Some(total) = self.reads.checked_add(actual_bytes) else {
            self.reads = usize::MAX;
            return Err(OwnerError::ContractExhausted);
        };
        self.reads = total;
        if total > request.limits.read_bytes { return Err(OwnerError::ContractExhausted); }
        Ok(())
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

#[cfg(test)]
mod migration_selector_tests {
    use super::*;
    use std::time::Duration;

    fn limits() -> NativeRootLimits {
        NativeRootLimits::new([256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256]).unwrap()
    }

    fn invocation() -> Vec<OsString> {
        ["--plan-control-migration", "unopened-root", "10000000-0000-0000-0000-000000000001", "unopened-output"]
            .map(OsString::from).to_vec()
    }

    #[test]
    fn retains_native_config_argv_and_original_context_without_opening_locators() {
        let selected = invocation();
        let mut arguments = vec![OsString::from("--config-file"), OsString::from("unread-config.toml")];
        arguments.extend(selected.iter().cloned());
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancellation = Arc::new(AtomicBool::new(false));
        let (request, actual) = NativeRootRequest::for_control_migration_plan(
            &arguments, deadline, Arc::clone(&cancellation), limits(),
        ).unwrap();
        assert_eq!(actual, selected);
        assert_eq!(request.root, PathBuf::from("unopened-root"));
        assert_eq!(request.mode, DataRootOpenMode::InspectExistingReadOnly);
        let expected = arguments.iter().map(|value| native_argument(value, 256 * 1024).unwrap()).collect::<Vec<_>>();
        assert_eq!(request.arguments(), expected);
        assert_eq!(request.deadline, deadline);
        assert!(Arc::ptr_eq(&request.cancellation, &cancellation));
        cancellation.store(true, Ordering::Release);
        assert!(request.check().is_err());
    }

    #[test]
    fn rejects_extra_missing_empty_and_invalid_selected_arguments() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let refused = |arguments: &[OsString]| assert!(NativeRootRequest::for_control_migration_plan(
            arguments, deadline, Arc::new(AtomicBool::new(false)), limits(),
        ).is_err());
        let valid = invocation();
        refused(&valid[..3]);
        let mut extra = valid.clone();
        extra.push(OsString::from("extra"));
        refused(&extra);
        for (index, value) in [(0, "--other-command"), (1, ""), (2, "invalid"),
            (2, "00000000-0000-0000-0000-000000000000"), (3, "")] {
            let mut arguments = valid.clone();
            arguments[index] = OsString::from(value);
            refused(&arguments);
        }
        assert!(NativeRootRequest::for_control_migration_plan(
            &valid, Instant::now(), Arc::new(AtomicBool::new(false)), limits(),
        ).is_err());
    }

    fn bootstrap_invocation() -> Vec<OsString> {
        ["--bootstrap-control-migration", "unopened-root", "10000000-0000-0000-0000-000000000002",
            "unopened-output"].map(OsString::from).to_vec()
    }

    #[test]
    fn bootstrap_route_retains_raw_config_argv_and_original_context_without_rewriting_the_flag() {
        let selected = bootstrap_invocation();
        let mut arguments = vec![OsString::from("--config-file"), OsString::from("unread-config.toml")];
        arguments.extend(selected.iter().cloned());
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancellation = Arc::new(AtomicBool::new(false));
        let (request, actual) = NativeRootRequest::for_control_migration_bootstrap(
            &arguments, deadline, Arc::clone(&cancellation), limits(),
        ).unwrap();
        // The selected flag stays the demanded literal; no synthetic argv replaces it.
        assert_eq!(actual, selected);
        assert_eq!(actual[0], OsString::from("--bootstrap-control-migration"));
        assert_eq!(request.root, PathBuf::from("unopened-root"));
        assert_eq!(request.mode, DataRootOpenMode::InspectExistingReadOnly);
        // The raw-args binding is computed from the caller's own argv bytes.
        let expected = arguments.iter()
            .map(|value| native_argument(value, 256 * 1024).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(request.arguments(), expected);
        assert_eq!(request.deadline, deadline);
        assert!(Arc::ptr_eq(&request.cancellation, &cancellation));
        cancellation.store(true, Ordering::Release);
        assert!(request.check().is_err());
    }

    #[test]
    fn bootstrap_route_refuses_malformed_flag_extra_missing_empty_and_invalid_target() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let refused = |arguments: &[OsString]| assert!(NativeRootRequest::for_control_migration_bootstrap(
            arguments, deadline, Arc::new(AtomicBool::new(false)), limits(),
        ).is_err());
        // A plan flag never satisfies the bootstrap constructor.
        refused(&invocation()[..4]);
        for (index, value) in [(0, "--plan-control-migration"), (0, "--bootstrap-control-migratn"),
            (0, "--bootstrap-control-migrationx"), (1, ""), (2, "invalid"),
            (2, "00000000-0000-0000-0000-000000000000"), (3, "")] {
            let mut arguments = bootstrap_invocation();
            arguments[index] = OsString::from(value);
            refused(&arguments);
        }
        let mut extra = bootstrap_invocation();
        extra.push(OsString::from("extra"));
        refused(&extra);
        refused(&bootstrap_invocation()[..3]);
        assert!(NativeRootRequest::for_control_migration_bootstrap(
            &bootstrap_invocation(), Instant::now(), Arc::new(AtomicBool::new(false)), limits(),
        ).is_err());
    }

    #[test]
    fn each_migration_constructor_keeps_its_own_flag_and_profile_bytes_distinct() {
        let plan = invocation();
        let bootstrap = bootstrap_invocation();
        let deadline = Instant::now() + Duration::from_secs(30);
        let (plan_request, _) = NativeRootRequest::for_control_migration_plan(
            &plan, deadline, Arc::new(AtomicBool::new(false)), limits(),
        ).unwrap();
        let (bootstrap_request, _) = NativeRootRequest::for_control_migration_bootstrap(
            &bootstrap, deadline, Arc::new(AtomicBool::new(false)), limits(),
        ).unwrap();
        assert_eq!(plan_request.arguments()[0], native_argument(&plan[0], 256 * 1024).unwrap());
        assert_eq!(bootstrap_request.arguments()[0], native_argument(&bootstrap[0], 256 * 1024).unwrap());
        assert_ne!(plan_request.arguments()[0], bootstrap_request.arguments()[0]);
        assert_eq!(bootstrap_request.mode, plan_request.mode);
        assert_ne!(plan_request.digest, bootstrap_request.digest);
    }
}
