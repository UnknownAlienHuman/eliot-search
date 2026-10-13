//! One existing-only native root owner, with no catalog/store child authority.

use std::fs::{self, File, TryLockError};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use search_contracts::{OwnerEpoch, RequestId};
use search_runtime_owner::{
    DataRootOpenMode, DrainReason, OwnerError, RootOpenDecision, RootOpenState, classify_root_open,
};

use super::ShutdownReceipt;
use super::native_objects::{NativePins, metadata, native_identity, open_existing, read_bytes};
use super::native_request::{NativeRootRequest, NativeWork, native_argument};
use super::native_snapshot::{NativeInstallation, barriers, read_snapshot};
use super::record::DurableOwnerRecord;
use super::spec::{DrainReasonText, INSTALLATION_FILE, LifecycleState, MAX_STATE_BYTES};
use super::succession::plan_successor;

/// An unavailable classification carries no native capability.
pub enum NativeRootAdmission {
    Unavailable(RootOpenState, NativeRootRequest),
    Inspection(Box<NativeRootCapability>),
    Mutation(Box<NativeRootCapability>),
}

/// Exact owner metadata is usable only after current native/context revalidation.
pub struct NativeRootMetadata {
    pub owner_epoch: OwnerEpoch,
    pub generation: u64,
    pub request_id: RequestId,
    pub request_digest: [u8; 32],
}

/// Non-cloneable native authority. Fields and constructors remain owner-private.
/// Pin/record resources drop before both existing OS exclusions; Drop never writes.
pub struct NativeRootCapability {
    pins: NativePins,
    root: PathBuf,
    installation: NativeInstallation,
    record: DurableOwnerRecord,
    target: usize,
    selected_key_binding: Option<[u8; 32]>,
    request: NativeRootRequest,
    work: NativeWork,
    mutation: bool,
    poisoned: Option<OwnerError>,
    effect_observed: bool,
    bound_root: File,
    exclusions: Vec<File>,
}

impl core::fmt::Debug for NativeRootCapability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("NativeRootCapability(<opaque>)")
    }
}

/// Consumes one original request and acquires only already-existing objects.
/// Initialize/recovery, redb and sealed stores remain unavailable in this profile.
pub fn admit_existing(request: NativeRootRequest) -> Result<NativeRootAdmission, OwnerError> {
    admit_with_work(request, NativeWork::new())
}

fn admit_with_work(
    request: NativeRootRequest,
    mut work: NativeWork,
) -> Result<NativeRootAdmission, OwnerError> {
    let acquired = match acquire_existing(&request, &mut work)? {
        Ok(acquired) => acquired,
        Err(state) => return unavailable(request, work, state),
    };
    let (capability, successor) = acquired.bind(request, work);
    finish_admission(capability, successor)
}

// Acquisition owns RELEASED custody, but borrows the caller's one original
// context/meter. Semantic preflight may therefore finish before ACTIVE exists.
struct AcquiredNativeRoot {
    pins: NativePins,
    root: PathBuf,
    installation: NativeInstallation,
    record: DurableOwnerRecord,
    target: usize,
    selected_key_binding: Option<[u8; 32]>,
    mutation: bool,
    successor: Option<DurableOwnerRecord>,
    bound_root: File,
    exclusions: Vec<File>,
}

impl AcquiredNativeRoot {
    fn bind(
        self,
        request: NativeRootRequest,
        work: NativeWork,
    ) -> (NativeRootCapability, Option<DurableOwnerRecord>) {
        let Self {
            pins,
            root,
            installation,
            record,
            target,
            selected_key_binding,
            mutation,
            successor,
            bound_root,
            exclusions,
        } = self;
        (
            NativeRootCapability {
                pins,
                root,
                installation,
                record,
                target,
                selected_key_binding,
                request,
                work,
                mutation,
                poisoned: None,
                effect_observed: false,
                bound_root,
                exclusions,
            },
            successor,
        )
    }
}

fn acquire_existing(
    request: &NativeRootRequest,
    work: &mut NativeWork,
) -> Result<Result<AcquiredNativeRoot, RootOpenState>, OwnerError> {
    request.check()?;
    if !cfg!(windows)
        || matches!(
            request.mode,
            DataRootOpenMode::InitializeNew | DataRootOpenMode::OpenNamedRecovery
        )
    {
        return Ok(Err(RootOpenState::Unsupported));
    }
    let Some(root_metadata) = metadata(request, work, request.root())? else {
        return Ok(Err(RootOpenState::Absent));
    };
    if !root_metadata.is_dir()
        || root_metadata.file_type().is_symlink()
        || super::codec::is_reparse(&root_metadata)
    {
        return Ok(Err(RootOpenState::Unsupported));
    }
    let root = work.call(request, || {
        fs::canonicalize(request.root()).map_err(|_| OwnerError::DataRootInvalid)
    })?;
    if !local_profile_path(&root)? {
        return Ok(Err(RootOpenState::Unsupported));
    }
    // Retain the physical root before opening either exclusion locator.
    let bound_root = open_existing(request, work, &root, true, false)?;
    if let Some(state) = barriers(request, work, &root)? {
        return Ok(Err(state));
    }
    // Both absent locks plus no installation/layout evidence can be inspected
    // as absent. Partial or legacy layouts never receive a usable capability.
    let installation = metadata(request, work, &root.join(INSTALLATION_FILE))?;
    if installation.is_none() {
        let state = if partial_layout(request, work, &root)? {
            RootOpenState::OutcomeUnknown
        } else {
            RootOpenState::Absent
        };
        return Ok(Err(state));
    }
    let exclusions = acquire_exclusions(request, work, &root)?;
    let mut installation_file =
        open_existing(request, work, &root.join(INSTALLATION_FILE), false, false)?;
    let installation_bytes = read_bytes(request, work, &mut installation_file, 1024)?;
    if installation_bytes.starts_with(b"ELIOT-SEARCH-INSTALLATION-V1\nformat_version=1\n") {
        return Ok(Err(RootOpenState::Unsupported));
    }
    let mutation = request.mode == DataRootOpenMode::OpenExistingMutating;
    let profile = super::native_snapshot::parse_native_installation(&installation_bytes)?;
    let mut pins =
        NativePins::acquire_profile(request, work, &root, mutation, profile.journal_profile)?;
    let snapshot = read_snapshot(request, work, &root, &mut pins)?;
    work.arrive(request, "native-proof")?;
    if snapshot.record.lifecycle != LifecycleState::Released {
        return Ok(Err(RootOpenState::OutcomeUnknown));
    }
    let decision = classify_root_open(request.mode, RootOpenState::Initialized);
    if !matches!(
        (mutation, decision),
        (true, RootOpenDecision::RequireLiveOwner) | (false, RootOpenDecision::ReadExisting)
    ) {
        return Err(OwnerError::OwnerInvalidTransition);
    }
    let successor = if mutation {
        Some(work.call(request, || {
            plan_successor(
                &snapshot.installation.owner_binding(),
                &snapshot.observed,
                snapshot.record.executable_digest,
                Some(&snapshot.record),
            )
        })?)
    } else {
        None
    };
    Ok(Ok(AcquiredNativeRoot {
        pins,
        root,
        installation: snapshot.installation,
        record: snapshot.record,
        target: snapshot.target,
        selected_key_binding: snapshot.selected_key_binding,
        mutation,
        successor,
        bound_root,
        exclusions,
    }))
}

pub(super) fn acquire_exclusions(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    root: &Path,
) -> Result<Vec<File>, OwnerError> {
    let mut exclusions = Vec::with_capacity(2);
    for name in [
        ".eliot-search-owner.lock",
        ".eliot-search-sealed-owner.lock",
    ] {
        let file = open_existing(request, work, &root.join(name), false, true)?;
        work.call(request, || {
            file.try_lock().map_err(|error| match error {
                TryLockError::WouldBlock => OwnerError::DataRootAlreadyOwned,
                TryLockError::Error(_) => OwnerError::OwnerGuardMismatch,
            })
        })?;
        exclusions.push(file);
    }
    Ok(exclusions)
}

pub(super) fn local_profile_path(root: &Path) -> Result<bool, OwnerError> {
    let native_path = native_argument(root.as_os_str(), 256 * 1024)?;
    #[cfg(windows)]
    {
        // Only a canonical local drive path, never UNC or device namespaces.
        let units: Vec<_> = native_path
            .as_chunks::<2>()
            .0
            .iter()
            .take(7)
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        Ok(units.len() == 7
            && units[..4] == [92, 92, 63, 92]
            && matches!(units[4], 65..=90 | 97..=122)
            && units[5..] == [58, 92])
    }
    #[cfg(not(windows))]
    {
        let _ = native_path;
        Ok(false)
    }
}

fn finish_admission(
    mut capability: NativeRootCapability,
    successor: Option<DurableOwnerRecord>,
) -> Result<NativeRootAdmission, OwnerError> {
    finish_in_place(&mut capability, successor)?;
    Ok(if capability.mutation {
        NativeRootAdmission::Mutation(Box::new(capability))
    } else {
        NativeRootAdmission::Inspection(Box::new(capability))
    })
}

fn finish_in_place(
    capability: &mut NativeRootCapability,
    successor: Option<DurableOwnerRecord>,
) -> Result<(), OwnerError> {
    if let Some(record) = successor {
        capability.publish(record)?;
    } else {
        capability.verify()?;
    }
    capability
        .work
        .arrive(&capability.request, "admission-terminal")
        .map_err(|error| {
            if capability.effect_observed {
                capability.unknown_error()
            } else {
                error
            }
        })?;
    capability.check_context()?;
    Ok(())
}

fn partial_layout(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    root: &Path,
) -> Result<bool, OwnerError> {
    for name in [
        super::initialization_journal::JOURNAL_FILE,
        "control",
        "revisions",
        ".eliot-search-owner.lock",
        ".eliot-search-sealed-owner.lock",
    ] {
        if metadata(request, work, &root.join(name))?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn unavailable(
    request: NativeRootRequest,
    work: NativeWork,
    state: RootOpenState,
) -> Result<NativeRootAdmission, OwnerError> {
    request.check()?;
    if request.mode != DataRootOpenMode::InspectExistingReadOnly {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    Ok(NativeRootAdmission::Unavailable(
        state,
        request.retain_refusal_work(work),
    ))
}

impl NativeRootCapability {
    fn check_context(&mut self) -> Result<(), OwnerError> {
        self.request.check().map_err(|error| self.failure(error))
    }

    fn unknown_error(&self) -> OwnerError {
        self.poisoned.unwrap_or_else(|| {
            if self.record.lifecycle == LifecycleState::Released && self.effect_observed {
                OwnerError::OwnerReleaseOutcomeUnknown
            } else {
                OwnerError::OwnerAcquireOutcomeUnknown
            }
        })
    }

    fn failure(&mut self, error: OwnerError) -> OwnerError {
        if let Some(poisoned) = self.poisoned {
            return poisoned;
        }
        if self.effect_observed {
            let unknown = self.unknown_error();
            self.poisoned = Some(unknown);
            return unknown;
        }
        error
    }

    fn verify(&mut self) -> Result<(), OwnerError> {
        self.verify_bound().map_err(|error| self.failure(error))
    }

    fn verify_bound(&mut self) -> Result<(), OwnerError> {
        if let Some(error) = self.poisoned {
            return Err(error);
        }
        self.check_context()?;
        let snapshot = NativeBindingView {
            pins: &mut self.pins,
            root: &self.root,
            installation: &self.installation,
            record: &self.record,
            selected_key_binding: self.selected_key_binding,
            bound_root: &self.bound_root,
            exclusions: &self.exclusions,
        }
        .verify(&self.request, &mut self.work)?;
        self.target = snapshot.target;
        self.check_context()
    }

    pub(crate) fn read_startup(
        self: Box<Self>,
    ) -> Result<read_child::NativeReadStartup, OwnerError> {
        read_child::NativeReadStartup::open(self)
    }

    pub fn inspect(&mut self) -> Result<NativeRootMetadata, OwnerError> {
        self.verify()?;
        let (request_id, request_digest) = self.request.binding();
        let result = NativeRootMetadata {
            owner_epoch: OwnerEpoch::new(self.record.epoch)
                .map_err(|_| self.failure(OwnerError::ContractExhausted))?,
            generation: self.record.generation,
            request_id,
            request_digest,
        };
        self.check_context()?;
        Ok(result)
    }

    /// Holds exclusions and the original request across terminal emission.
    pub fn emit<W: Write>(&mut self, writer: &mut W, line: &str) -> Result<(), OwnerError> {
        self.verify()?;
        self.work
            .arrive(&self.request, "terminal-emission")
            .map_err(|error| self.failure(error))?;
        self.request
            .emit(writer, line)
            .map_err(|error| self.failure(error))
    }

    pub fn begin_drain(&mut self, reason: DrainReason) -> Result<(), OwnerError> {
        if !self.mutation {
            return Err(self.failure(OwnerError::OwnerInvalidTransition));
        }
        self.check_context()?;
        if self.record.lifecycle == LifecycleState::Draining {
            return self.verify();
        }
        if self.record.lifecycle != LifecycleState::Active {
            return Err(self.failure(OwnerError::OwnerInvalidTransition));
        }
        let mut next = self.record.clone();
        next.lifecycle = LifecycleState::Draining;
        next.drain_reason = DrainReasonText::from_policy(reason);
        self.advance(next)
    }

    pub fn release_cleanly(&mut self) -> Result<ShutdownReceipt, OwnerError> {
        if !self.mutation {
            return Err(self.failure(OwnerError::OwnerInvalidTransition));
        }
        self.check_context()?;
        if self.record.lifecycle != LifecycleState::Draining {
            return Err(self.failure(OwnerError::OwnerDrainRequired));
        }
        let mut next = self.record.clone();
        next.lifecycle = LifecycleState::Released;
        next.drain_reason = DrainReasonText::None;
        self.advance(next)?;
        let epoch = OwnerEpoch::new(self.record.epoch)
            .map_err(|_| self.failure(OwnerError::ContractExhausted))?;
        self.check_context()?;
        Ok(ShutdownReceipt {
            epoch,
            generation: self.record.generation,
            record_digest: self.record.record_digest,
        })
    }

    fn advance(&mut self, mut next: DurableOwnerRecord) -> Result<(), OwnerError> {
        next.generation = self
            .record
            .generation
            .checked_add(1)
            .ok_or_else(|| self.failure(OwnerError::ContractExhausted))?;
        next.refresh_digest();
        self.publish(next)
    }

    fn publish(&mut self, next: DurableOwnerRecord) -> Result<(), OwnerError> {
        self.verify()?;
        next.validate_shape().map_err(|error| self.failure(error))?;
        let bytes = next.encode();
        if bytes.len() > MAX_STATE_BYTES {
            return Err(self.failure(OwnerError::ContractExhausted));
        }
        let mut effect_attempted = false;
        let result = (|| {
            let file = &mut self.pins.files[self.target];
            self.work.call(&self.request, || {
                file.seek(SeekFrom::Start(0))
                    .map(|_| ())
                    .map_err(|_| OwnerError::OwnerGuardMismatch)
            })?;
            self.work.arrive(&self.request, "before-owner-write")?;
            // This check executes after all hashing/readback and immediately
            // before the first possible durable effect, on the retained handle.
            self.work.call(&self.request, || {
                effect_attempted = true;
                file.set_len(0)
                    .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })?;
            let mut offset = 0;
            while offset < bytes.len() {
                self.work.write(&self.request, bytes.len() - offset)?;
                let count = self.work.call(&self.request, || {
                    file.write(&bytes[offset..])
                        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
                })?;
                if count == 0 {
                    return Err(OwnerError::OwnerAcquireOutcomeUnknown);
                }
                offset += count;
            }
            self.work.call(&self.request, || {
                file.sync_all()
                    .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })?;
            if read_bytes(&self.request, &mut self.work, file, MAX_STATE_BYTES)? != bytes {
                return Err(OwnerError::OwnerRecordDigestMismatch);
            }
            self.work.arrive(&self.request, "after-owner-write")?;
            self.request.check()
        })();
        if let Err(error) = result {
            if effect_attempted {
                let unknown = if next.lifecycle == LifecycleState::Released {
                    OwnerError::OwnerReleaseOutcomeUnknown
                } else {
                    OwnerError::OwnerAcquireOutcomeUnknown
                };
                self.poisoned = Some(unknown);
                return Err(unknown);
            }
            return Err(self.failure(error));
        }
        self.effect_observed = true;
        self.record = next;
        self.verify()
    }
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod native_tests;

// A view of the same retained custody. Admission and service operations share
// this verifier; the view cannot be constructed by a product caller.
struct NativeBindingView<'a> {
    pins: &'a mut NativePins,
    root: &'a Path,
    installation: &'a NativeInstallation,
    record: &'a DurableOwnerRecord,
    selected_key_binding: Option<[u8; 32]>,
    bound_root: &'a File,
    exclusions: &'a [File],
}

impl NativeBindingView<'_> {
    fn verify(
        self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
    ) -> Result<super::native_snapshot::NativeSnapshot, OwnerError> {
        request.check()?;
        self.pins.verify_locators(request, work, self.root)?;
        for (held, pinned) in std::iter::once(self.bound_root)
            .chain(self.exclusions.iter())
            .zip(&self.pins.files[..3])
        {
            let identity = work.call(request, || native_identity(held))?;
            if work.call(request, || native_identity(pinned))? != identity {
                return Err(OwnerError::OwnerGuardMismatch);
            }
        }
        if barriers(request, work, self.root)?.is_some() {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let snapshot = read_snapshot(request, work, self.root, self.pins)?;
        if &snapshot.installation != self.installation
            || &snapshot.record != self.record
            || snapshot.selected_key_binding != self.selected_key_binding
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        request.check()?;
        Ok(snapshot)
    }
}

#[path = "read_child.rs"]
mod read_child;
pub use read_child::{NativeReadOperation, NativeReadStartup, RetainedShutdown};

#[path = "source_sync.rs"]
mod source_sync;
pub use source_sync::{NativeSyncPreflightOperation, run_source_sync_preflight};

#[path = "registration_mutation.rs"]
mod registration_mutation;
pub use registration_mutation::{NativeRegistrationOperation, run_source_registration};
