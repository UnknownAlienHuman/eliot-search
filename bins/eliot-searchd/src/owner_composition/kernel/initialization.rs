//! Explicit secure initialization and existing-only historical replay.
//!
//! No Drop path writes, deletes or retries. Every possible effect is latched
//! before dispatch; partial state stays fenced for the separate recovery owner.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_runtime_owner::{DataRootOpenMode, OwnerError};

use super::codec::parse_id16;
use super::initialization_journal::{
    InitializationCompletion, InitializationSubject, JOURNAL_FILE, MAX_JOURNAL, decode_journal,
    initialization_namespace, installation_bytes, native_objects_digest,
};
use super::native_admission::{acquire_exclusions, local_profile_path};
use super::native_objects::{
    NativePins, OBJECTS, native_identity, open_existing, open_initialization, read_bytes,
    require_kind,
};
use super::native_request::{NativeRootRequest, NativeWork, native_argument};
use super::native_snapshot::{barriers, executable_digest, read_snapshot};
use super::observation::{mint_installation_ids, mint_owner_token, observe_root_parts};
use super::record::DurableOwnerRecord;
use super::spec::{DrainReasonText, LifecycleState, MAX_STATE_BYTES};
use super::storage_acl::{RootOwnerSeed, StorageAclSnapshot, qualify_existing_root};

/// Only this owner constructs the capability, under a retained native root.
#[derive(Clone, Copy, Eq, PartialEq)]
enum PreparationPhase {
    Layout,
    CredentialDispatched,
    Completed,
}

pub struct InitializingDataRoot {
    direct: Option<super::native_admission::direct_registry::DirectJournal>,
    root_owner_seed: Option<RootOwnerSeed>,
    storage_acl: Option<StorageAclSnapshot>,
    files: [Option<File>; 11],
    identities: [Option<[u8; 20]>; 11],
    request: NativeRootRequest,
    work: NativeWork,
    root: PathBuf,
    subject: InitializationSubject,
    history: Vec<u8>,
    preparation: PreparationPhase,
    effect_observed: bool,
    released: bool,
    poisoned: Option<OwnerError>,
    anchor_root: Option<File>,
    exclusions: Vec<File>,
}

/// Historical completion only; it cannot be used as a normal store capability.
pub struct InitializationResult {
    root: InitializingDataRoot,
    replayed: bool,
}

impl core::fmt::Debug for InitializationResult {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("InitializationResult(<opaque>)")
    }
}

#[cfg(test)]
pub fn initialize_new(request: NativeRootRequest) -> Result<InitializationResult, OwnerError> {
    initialize_with_work(request, NativeWork::new())
}

pub fn initialize_direct(request: NativeRootRequest) -> Result<InitializationResult, OwnerError> {
    initialize_target(request, NativeWork::new(), true)
}

#[cfg(test)]
fn initialize_with_work(
    request: NativeRootRequest,
    work: NativeWork,
) -> Result<InitializationResult, OwnerError> {
    initialize_target(request, work, false)
}
fn initialize_target(
    request: NativeRootRequest, mut work: NativeWork, direct: bool,
) -> Result<InitializationResult, OwnerError> {
    request.check()?;
    if !cfg!(windows) || request.mode != DataRootOpenMode::InitializeNew {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let initialization = initialization_id(&request)?;
    let root = work.call(&request, || {
        fs::canonicalize(request.root()).map_err(|_| OwnerError::DataRootInvalid)
    })?;
    if !local_profile_path(&root)? {
        return Err(OwnerError::DataRootInvalid);
    }
    let bound_root = open_initialization(&request, &mut work, &root, true)?;
    // The caller supplied an existing root. Qualify its held ACL without changing it.
    let root_owner_seed = if direct {
        Some(qualify_existing_root(&request, &mut work, &bound_root)?)
    } else {
        None
    };
    let barrier = if direct { super::native_snapshot::barriers_before_direct(&request, &mut work, &root)? }
        else { barriers(&request, &mut work, &root)? };
    if barrier.is_some() {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    work.open(&request)?;
    let mut directory = work.call(&request, || {
        fs::read_dir(&root).map_err(|_| OwnerError::DataRootInvalid)
    })?;
    let empty = work
        .call(&request, || {
            directory
                .next()
                .transpose()
                .map_err(|_| OwnerError::DataRootInvalid)
        })?
        .is_none();
    drop(directory);
    if !empty {
        return replay_existing(request, work, root, bound_root, initialization, direct, root_owner_seed);
    }
    let physical_root: [u8; 20] = work
        .call(&request, || native_identity(&bound_root))?
        .try_into()
        .map_err(|_| OwnerError::OwnerGuardMismatch)?;
    let path_bytes = native_argument(root.as_os_str(), 256 * 1024)?;
    let observed = observe_root_parts(&path_bytes, &physical_root[..12])?;
    let executable = executable_digest(&request, &mut work)?;
    let (installation, incarnation) =
        work.call(&request, || mint_installation_ids(&observed, &executable))?;
    let owner_pid = work.call(&request, || Ok(std::process::id()))?;
    let owner_token = work.call(&request, || mint_owner_token(&observed, &executable))?;
    let namespace = initialization_namespace(
        initialization,
        installation,
        incarnation,
        *observed.data_root_id.as_bytes(),
        executable,
        &mut |bytes| work.canonical(&request, bytes),
    )?;
    let (request_id, request_digest) = request.binding();
    let subject = InitializationSubject {
        original_request: *request_id.as_bytes(),
        request_digest,
        arguments: request.arguments().to_vec(),
        initialization,
        installation,
        incarnation,
        physical_root,
        root_id: *observed.data_root_id.as_bytes(),
        path_digest: observed.canonical_path_digest,
        volume_digest: observed.volume_identity_digest,
        executable,
        namespace,
        owner_token,
        owner_pid,
        journal_identity: [0; 20],
    };
    let mut files = std::array::from_fn(|_| None);
    files[0] = Some(bound_root);
    let mut identities = [None; 11];
    identities[0] = Some(physical_root);
    let mut cap = InitializingDataRoot {
        direct: None,
        root_owner_seed,
        storage_acl: None,
        files,
        identities,
        request,
        work,
        root,
        subject,
        history: Vec::new(),
        preparation: PreparationPhase::Layout,
        effect_observed: false,
        released: false,
        poisoned: None,
        anchor_root: None,
        exclusions: Vec::new(),
    };
    cap.provision_target(direct).map_err(|error| cap.failure(error))?;
    cap.verify_completed().map_err(|error| cap.failure(error))?;
    Ok(InitializationResult {
        root: cap,
        replayed: false,
    })
}

fn initialization_id(request: &NativeRootRequest) -> Result<[u8; 16], OwnerError> {
    let arguments = request.arguments();
    if arguments.len() != 3
        || arguments[0] != native_argument(std::ffi::OsStr::new("--initialize-data-root"), 256)?
        || arguments[2].len() != 64
    {
        return Err(OwnerError::OwnerOperationConflict);
    }
    let units: Vec<_> = arguments[2]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let text = String::from_utf16(&units).map_err(|_| OwnerError::OwnerOperationConflict)?;
    let id = parse_id16(Some(&text))?;
    if id == [0; 16] {
        return Err(OwnerError::OwnerOperationConflict);
    }
    request.check()?;
    Ok(id)
}

fn replay_existing(
    request: NativeRootRequest,
    mut work: NativeWork,
    root: PathBuf,
    bound_root: File,
    initialization: [u8; 16],
    direct: bool,
    root_owner_seed: Option<RootOwnerSeed>,
) -> Result<InitializationResult, OwnerError> {
    let mut exclusions = Vec::new();
    let mut pins = NativePins { files: Vec::new(), direct: None };
    let readback = (|| {
        exclusions = acquire_exclusions(&request, &mut work, &root)?;
        pins = NativePins::acquire_initialization(&request, &mut work, &root)?;
        let snapshot = read_snapshot(&request, &mut work, &root, &mut pins)?;
        if !snapshot.installation.journal_profile
            || snapshot.record.lifecycle != LifecycleState::Released
        {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let history = read_bytes(&request, &mut work, &mut pins.files[10], MAX_JOURNAL)?;
        let journal = decode_journal(&history, &mut |bytes| work.canonical(&request, bytes))?;
        let completion = journal
            .completion
            .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
        if journal.subject.initialization != initialization
            || journal.subject.arguments != request.arguments()
            || journal.subject.request_digest != request.binding().1
        {
            return Err(OwnerError::OwnerOperationConflict);
        }
        Ok((history, journal.subject, completion))
    })();
    let (history, subject, completion) = match readback {
        Ok(value) => value,
        Err(error) => {
            drop(pins);
            drop(bound_root);
            drop(exclusions);
            return Err(error);
        }
    };
    let mut files = std::array::from_fn(|_| None);
    for (slot, file) in files.iter_mut().zip(pins.files) {
        *slot = Some(file);
    }
    let identities = completion.native_objects.map(Some);
    let mut cap = InitializingDataRoot {
        direct: None,
        root_owner_seed,
        storage_acl: None,
        files,
        identities,
        request,
        work,
        root,
        subject,
        history,
        preparation: PreparationPhase::Completed,
        effect_observed: false,
        released: true,
        poisoned: None,
        anchor_root: Some(bound_root),
        exclusions,
    };
    if direct {
        let record = DurableOwnerRecord::decode(&cap.layout_bytes(6, MAX_STATE_BYTES)?)?;
        cap.capture_storage_acl()?;
        cap.direct = Some(super::native_admission::direct_registry::DirectJournal::open(
            &cap.request, &mut cap.work, &cap.root, cap.files[3].as_ref().ok_or(OwnerError::OwnerGuardMismatch)?,
            super::native_admission::direct_registry::journal_owner(cap.subject.incarnation, cap.subject.root_id, record.epoch)?, false,
            cap.storage_acl.as_ref().ok_or(OwnerError::OwnerStorageAclUnqualified)?)?);
        cap.capture_storage_acl()?;
    }
    cap.checkpoint("completed-replay")?;
    if crate::direct_store::existing_key_binding(&mut cap)? != completion.selected_key_binding {
        return Err(cap.failure(OwnerError::OwnerGuardMismatch));
    }
    cap.verify_completed()?;
    Ok(InitializationResult {
        root: cap,
        replayed: true,
    })
}

impl InitializingDataRoot {
    fn release_native_resources(&mut self) {
        drop(self.direct.take());
        if self.files.iter().all(Option::is_none)
            && self.anchor_root.is_none()
            && self.exclusions.is_empty()
        {
            return;
        }
        // Child pins and both root handles close before either real exclusion.
        for index in (3..self.files.len()).rev() {
            drop(self.files[index].take());
        }
        drop(self.files[0].take());
        drop(self.anchor_root.take());
        #[cfg(test)]
        if let Some(hook) = &mut self.work.phase_hook {
            hook("initialization-children-released");
        }
        drop(self.files[2].take());
        drop(self.files[1].take());
        self.exclusions.clear();
        #[cfg(test)]
        if let Some(hook) = &mut self.work.phase_hook {
            hook("initialization-exclusions-released");
        }
    }

    const fn failure(&mut self, error: OwnerError) -> OwnerError {
        if let Some(error) = self.poisoned {
            return error;
        }
        let error = if self.effect_observed {
            if self.released {
                OwnerError::OwnerReleaseOutcomeUnknown
            } else {
                OwnerError::OwnerAcquireOutcomeUnknown
            }
        } else {
            error
        };
        self.poisoned = Some(error);
        error
    }

    pub(crate) fn checkpoint(&mut self, phase: &'static str) -> Result<(), OwnerError> {
        let result = (|| {
            if let Some(error) = self.poisoned {
                return Err(error);
            }
            self.work.arrive(&self.request, phase)?;
            for index in 0..self.files.len() {
                if self.files[index].is_some() {
                    self.verify_held(index)?;
                }
            }
            if let Some(anchor) = &self.anchor_root {
                let identity = self.work.call(&self.request, || native_identity(anchor))?;
                let file = self.files[0]
                    .as_ref()
                    .ok_or(OwnerError::OwnerGuardMismatch)?;
                if self.work.call(&self.request, || native_identity(file))? != identity {
                    return Err(OwnerError::OwnerGuardMismatch);
                }
            }
            for (index, exclusion) in self.exclusions.iter().enumerate() {
                let identity = self
                    .work
                    .call(&self.request, || native_identity(exclusion))?;
                let file = self.files[index + 1]
                    .as_ref()
                    .ok_or(OwnerError::OwnerGuardMismatch)?;
                if self.work.call(&self.request, || native_identity(file))? != identity {
                    return Err(OwnerError::OwnerGuardMismatch);
                }
            }
            if !self.history.is_empty() && self.layout_bytes(10, MAX_JOURNAL)? != self.history {
                return Err(OwnerError::OwnerGuardMismatch);
            }
            self.verify_storage_acl()?;
            self.request.check()
        })();
        result.map_err(|error| self.failure(error))
    }

    pub(crate) const fn namespace(&self) -> [u8; 32] {
        self.subject.namespace
    }

    pub(crate) fn credential_call<T>(
        &mut self,
        create: bool,
        operation: impl FnOnce([u8; 32], Instant, &AtomicBool) -> Result<T, OwnerError>,
    ) -> Result<T, OwnerError> {
        self.checkpoint("before-credential")?;
        self.qualify_storage_file(0)?;
        if create {
            if self.preparation != PreparationPhase::Layout {
                return Err(self.failure(OwnerError::OwnerInvalidTransition));
            }
            self.verify_new_inventory()?;
            crate::plaintext_direct_store::verify_initialization_layout(self, true)?;
            // Consumed before dispatch, never reset by failure or later readback.
            self.preparation = PreparationPhase::CredentialDispatched;
        }
        #[cfg(test)]
        let mut observer = self.work.credential_hook.take();
        let (deadline, cancellation) = self.request.credential_context();
        let result = self.work.call(&self.request, || {
            #[cfg(test)]
            if let Some(hook) = &mut observer {
                hook(create, deadline, cancellation);
            }
            operation(self.subject.namespace, deadline, cancellation)
        });
        #[cfg(test)]
        {
            self.work.credential_hook = observer;
        }
        let value = result.map_err(|error| self.failure(error))?;
        self.checkpoint("after-credential")?;
        Ok(value)
    }

    fn path(&self, index: usize) -> Result<PathBuf, OwnerError> {
        let name = if index == 10 {
            JOURNAL_FILE
        } else {
            OBJECTS.get(index).ok_or(OwnerError::OwnerGuardMismatch)?.0
        };
        Ok(self.root.join(name))
    }

    fn verify_locator(&mut self, index: usize) -> Result<(), OwnerError> {
        self.verify_held(index)?;
        let path = self.path(index)?;
        let directory = matches!(index, 0 | 3 | 4);
        let current = open_existing(&self.request, &mut self.work, &path, directory, false)?;
        let identity = self.identities[index].ok_or(OwnerError::OwnerGuardMismatch)?;
        if self
            .work
            .call(&self.request, || native_identity(&current))?
            .as_slice()
            != identity
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        Ok(())
    }

    fn verify_held(&mut self, index: usize) -> Result<(), OwnerError> {
        let file = self.files[index]
            .as_ref()
            .ok_or(OwnerError::OwnerGuardMismatch)?;
        let observed = self.work.call(&self.request, || {
            file.metadata().map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        require_kind(&observed, matches!(index, 0 | 3 | 4))?;
        let expected = self.identities[index].ok_or(OwnerError::OwnerGuardMismatch)?;
        if self
            .work
            .call(&self.request, || native_identity(file))?
            .as_slice()
            != expected
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        Ok(())
    }

    fn capture(&mut self, index: usize, file: File) -> Result<(), OwnerError> {
        self.files[index] = Some(file);
        let held = self.files[index]
            .as_ref()
            .ok_or(OwnerError::OwnerGuardMismatch)?;
        self.identities[index] = Some(
            self.work
                .call(&self.request, || native_identity(held))?
                .try_into()
                .map_err(|_| OwnerError::OwnerGuardMismatch)?,
        );
        self.verify_held(index)?;
        // Empty files may inherit the token default DACL. No contents enter until qualified.
        self.qualify_storage_file(index)
    }

    fn qualify_storage_file(&mut self, index: usize) -> Result<(), OwnerError> {
        let Some(seed) = &self.root_owner_seed else { return Ok(()); };
        let root = self.files[0].as_ref().ok_or(OwnerError::OwnerGuardMismatch)?;
        seed.qualify_file(&self.request, &mut self.work, root)?;
        if index != 0 {
            let file = self.files[index].as_ref().ok_or(OwnerError::OwnerGuardMismatch)?;
            seed.qualify_file(&self.request, &mut self.work, file)?;
        }
        Ok(())
    }

    fn verify_parent(&mut self, index: usize) -> Result<(), OwnerError> {
        self.verify_locator(0)?;
        if matches!(index, 8 | 9) {
            self.verify_locator(3)?;
        }
        Ok(())
    }

    fn create_file(&mut self, index: usize, lock: bool) -> Result<(), OwnerError> {
        self.checkpoint("before-create-file")?;
        if self.preparation == PreparationPhase::Completed
            || self.files[index].is_some()
            || matches!(index, 0 | 3 | 4)
        {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        if index == 10 {
            self.verify_new_inventory()?;
        }
        let path = self.path(index)?;
        self.verify_parent(index)?;
        self.work.open(&self.request)?;
        let file = self
            .work
            .call(&self.request, || {
                let mut options = OpenOptions::new();
                options.read(true).write(true).create_new(true);
                #[cfg(windows)]
                {
                    use std::os::windows::fs::OpenOptionsExt;
                    options.custom_flags(0x0020_0000).share_mode(1);
                }
                self.effect_observed = true;
                options
                    .open(&path)
                    .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })
            .map_err(|error| self.failure(error))?;
        self.capture(index, file)?;
        if lock {
            let file = self.files[index]
                .as_ref()
                .ok_or(OwnerError::OwnerGuardMismatch)?;
            self.work.call(&self.request, || {
                file.try_lock().map_err(|error| match error {
                    TryLockError::WouldBlock => OwnerError::DataRootAlreadyOwned,
                    TryLockError::Error(_) => OwnerError::OwnerGuardMismatch,
                })
            })?;
        }
        self.verify_locator(index)?;
        self.checkpoint("after-create-file")
    }

    pub(crate) fn create_layout_directory(&mut self, index: usize) -> Result<(), OwnerError> {
        self.checkpoint("before-create-directory")?;
        if self.preparation == PreparationPhase::Completed
            || !matches!(index, 3 | 4)
            || self.files[index].is_some()
        {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        let path = self.path(index)?;
        self.verify_parent(index)?;
        let created = if let Some(seed) = &self.root_owner_seed {
            let root = self.files[0].as_ref().ok_or(OwnerError::OwnerGuardMismatch)?;
            self.effect_observed = true;
            seed.create_directory(&self.request, &mut self.work, &path, root)
        } else {
            // Retained non-DIRECT fixture initialization has no private-storage claim.
            self.work.call(&self.request, || {
                self.effect_observed = true;
                fs::create_dir(&path).map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })
        };
        created.map_err(|error| self.failure(error))?;
        let file = open_initialization(&self.request, &mut self.work, &path, true)?;
        self.capture(index, file)?;
        self.verify_locator(index)?;
        self.checkpoint("after-create-directory")
    }

    pub(crate) fn publish_layout_file(
        &mut self,
        index: usize,
        bytes: &[u8],
    ) -> Result<(), OwnerError> {
        if !matches!(index, 8 | 9) {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        self.create_file(index, false)?;
        self.write_file(index, bytes, false)
    }

    fn write_file(&mut self, index: usize, bytes: &[u8], append: bool) -> Result<(), OwnerError> {
        self.checkpoint("before-file-write")?;
        if bytes.len() > MAX_JOURNAL {
            return Err(OwnerError::ContractExhausted);
        }
        self.verify_locator(index)?;
        self.qualify_storage_file(index)?;
        let file = self.files[index]
            .as_mut()
            .ok_or(OwnerError::OwnerGuardMismatch)?;
        self.work.call(&self.request, || {
            file.seek(if append {
                SeekFrom::End(0)
            } else {
                SeekFrom::Start(0)
            })
            .map(|_| ())
            .map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        if !append {
            self.work.call(&self.request, || {
                self.effect_observed = true;
                file.set_len(0)
                    .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })?;
        }
        let mut offset = 0;
        while offset < bytes.len() {
            self.work.write(&self.request, bytes.len() - offset)?;
            let count = self.work.call(&self.request, || {
                self.effect_observed = true;
                file.write(&bytes[offset..])
                    .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })?;
            if count == 0 || count > bytes.len() - offset {
                return Err(OwnerError::OwnerAcquireOutcomeUnknown);
            }
            offset += count;
        }
        self.work.call(&self.request, || {
            file.sync_all()
                .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
        })?;
        self.request.check()
    }

    pub(crate) fn layout_bytes(
        &mut self,
        index: usize,
        ceiling: usize,
    ) -> Result<Vec<u8>, OwnerError> {
        let file = self.files[index]
            .as_mut()
            .ok_or(OwnerError::OwnerGuardMismatch)?;
        read_bytes(&self.request, &mut self.work, file, ceiling)
    }

    pub(crate) fn layout_prefix(
        &mut self,
        index: usize,
        expected: &[u8],
    ) -> Result<Vec<u8>, OwnerError> {
        if index != 9 || expected.len() > 30 {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        self.verify_locator(index)?;
        let file = self.files[index]
            .as_mut()
            .ok_or(OwnerError::OwnerGuardMismatch)?;
        let before = self.work.call(&self.request, || {
            file.metadata().map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        require_kind(&before, false)?;
        self.work.call(&self.request, || {
            file.seek(SeekFrom::Start(0))
                .map(|_| ())
                .map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        let mut bytes = vec![0; expected.len()];
        let mut offset = 0;
        while offset < bytes.len() {
            self.work.read(&self.request, bytes.len() - offset)?;
            let count = self.work.call(&self.request, || {
                file.read(&mut bytes[offset..])
                    .map_err(|_| OwnerError::OwnerGuardMismatch)
            })?;
            if count == 0 {
                return Err(OwnerError::OwnerGuardMismatch);
            }
            offset += count;
        }
        let after = self.work.call(&self.request, || {
            file.metadata().map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        self.request.check()?;
        Ok(bytes)
    }

    pub(crate) fn require_empty_revisions(&mut self) -> Result<(), OwnerError> {
        self.verify_directory_inventory(4, &[])
    }

    fn verify_new_inventory(&mut self) -> Result<(), OwnerError> {
        self.verify_directory_inventory(0, &[1, 2, 3, 4, 5, 6, 7, 10])?;
        if self.files[3].is_some() {
            self.verify_directory_inventory(3, &[8, 9])?;
        }
        self.checkpoint("new-layout-inventory")
    }

    fn verify_directory_inventory(
        &mut self,
        index: usize,
        children: &[usize],
    ) -> Result<(), OwnerError> {
        self.verify_locator(index)?;
        let path = self.path(index)?;
        self.work.open(&self.request)?;
        let mut entries = self.work.call(&self.request, || {
            fs::read_dir(path).map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        let mut seen = [false; 11];
        loop {
            let next = self.work.call(&self.request, || {
                entries
                    .next()
                    .transpose()
                    .map_err(|_| OwnerError::OwnerGuardMismatch)
            })?;
            let Some(entry) = next else {
                break;
            };
            let name = self.work.call(&self.request, || Ok(entry.file_name()))?;
            if index == 3 && name == "control.redb" && self.direct.is_some() {
                self.direct.as_ref().ok_or(OwnerError::OwnerGuardMismatch)?
                    .verify(&self.request, &mut self.work, &self.root)?;
                continue;
            }
            let child = children
                .iter()
                .copied()
                .find(|child| {
                    self.files[*child].is_some()
                        && self
                            .path(*child)
                            .is_ok_and(|path| path.file_name() == Some(name.as_os_str()))
                })
                .ok_or(OwnerError::OwnerGuardMismatch)?;
            if seen[child] {
                return Err(OwnerError::OwnerGuardMismatch);
            }
            seen[child] = true;
        }
        drop(entries);
        if children
            .iter()
            .any(|child| seen[*child] != self.files[*child].is_some())
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        self.verify_locator(index)
    }

    fn publish_subject(&mut self) -> Result<(), OwnerError> {
        // Recheck after entropy/executable work, immediately before reserving the journal.
        self.verify_new_inventory()?;
        self.create_file(10, true)?;
        self.subject.journal_identity = self
            .work
            .call(&self.request, || {
                native_identity(
                    self.files[10]
                        .as_ref()
                        .ok_or(OwnerError::OwnerGuardMismatch)?,
                )
            })?
            .try_into()
            .map_err(|_| OwnerError::OwnerGuardMismatch)?;
        self.subject
            .validate(&mut |bytes| self.work.canonical(&self.request, bytes))?;
        let body = self.subject.frame()?;
        self.write_file(10, &body, false)?;
        if self.layout_bytes(10, MAX_JOURNAL)? != body {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        self.history = body;
        Ok(())
    }

    fn provision_target(&mut self, direct: bool) -> Result<(), OwnerError> {
        self.publish_subject()?;
        self.create_file(1, true)?;
        self.create_file(2, true)?;
        let selected = crate::direct_store::initialize_only(self)?.finish()?;
        for index in [5, 6, 7] {
            self.create_file(index, false)?;
        }
        self.verify_new_inventory()?;
        let identities = self.native_identities()?;
        let native_digest = native_objects_digest(&identities, &mut |bytes| {
            self.work.canonical(&self.request, bytes)
        })?;
        let installation = installation_bytes(&self.subject, native_digest);
        self.write_file(5, &installation, false)?;
        if self.layout_bytes(5, 1024)? != installation {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        if direct {
            self.capture_storage_acl()?;
            self.effect_observed = true;
            self.direct = Some(super::native_admission::direct_registry::DirectJournal::open(
                &self.request, &mut self.work, &self.root, self.files[3].as_ref().ok_or(OwnerError::OwnerGuardMismatch)?,
                super::native_admission::direct_registry::journal_owner(self.subject.incarnation, self.subject.root_id, 1)?, true,
                self.storage_acl.as_ref().ok_or(OwnerError::OwnerStorageAclUnqualified)?)?);
            self.capture_storage_acl()?;
        }
        let mut record = DurableOwnerRecord {
            installation_id: self.subject.installation,
            installation_incarnation_id: self.subject.incarnation,
            data_root_id: self.subject.root_id,
            epoch: 1,
            previous_epoch: 0,
            previous_record_digest: [0; 32],
            canonical_path_digest: self.subject.path_digest,
            volume_identity_digest: self.subject.volume_digest,
            executable_digest: self.subject.executable,
            owner_token: self.subject.owner_token,
            owner_pid: self.subject.owner_pid,
            lifecycle: LifecycleState::Active,
            drain_reason: DrainReasonText::None,
            generation: 1,
            record_digest: [0; 32],
        };
        record.refresh_digest();
        let active = record.encode();
        self.publish_record(6, &record)?;
        self.publish_record(7, &record)?;
        record.lifecycle = LifecycleState::Draining;
        record.drain_reason = DrainReasonText::Shutdown;
        record.generation = 2;
        record.refresh_digest();
        let draining = record.encode();
        self.publish_record(7, &record)?;
        record.lifecycle = LifecycleState::Released;
        record.drain_reason = DrainReasonText::None;
        record.generation = 3;
        record.refresh_digest();
        let released = record.encode();
        self.publish_record(6, &record)?;
        self.released = true;
        crate::plaintext_direct_store::verify_initialization_layout(self, true)?;
        if crate::direct_store::existing_key_binding(self)? != selected {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let completion = InitializationCompletion {
            subject_digest: self
                .subject
                .digest(&mut |bytes| self.work.canonical(&self.request, bytes))?,
            native_objects: identities,
            native_digest,
            installation_bytes: installation,
            namespace_bytes: self.layout_bytes(8, 65)?,
            log_bytes: self.layout_bytes(9, 30)?,
            lifecycle_bytes: [active, draining, released],
            selected_key_binding: selected,
        };
        completion.validate(&self.subject, &mut |bytes| {
            self.work.canonical(&self.request, bytes)
        })?;
        let seal = completion.frame()?;
        self.write_file(10, &seal, true)?;
        self.history
            .try_reserve_exact(seal.len())
            .map_err(|_| OwnerError::ContractExhausted)?;
        self.history.extend_from_slice(&seal);
        if self.layout_bytes(10, MAX_JOURNAL)? != self.history {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        self.preparation = PreparationPhase::Completed;
        self.checkpoint("completion-seal-readback")
    }

    fn publish_record(
        &mut self,
        index: usize,
        record: &DurableOwnerRecord,
    ) -> Result<(), OwnerError> {
        record.validate_shape()?;
        let bytes = record.encode();
        self.write_file(index, &bytes, false)?;
        if DurableOwnerRecord::decode(&self.layout_bytes(index, MAX_STATE_BYTES)?)? != *record {
            return Err(OwnerError::OwnerRecordDigestMismatch);
        }
        self.checkpoint("owner-record-readback")
    }

    fn native_identities(&mut self) -> Result<[[u8; 20]; 11], OwnerError> {
        let mut identities = [[0; 20]; 11];
        for (index, identity) in identities.iter_mut().enumerate() {
            self.verify_locator(index)?;
            let file = self.files[index]
                .as_ref()
                .ok_or(OwnerError::OwnerGuardMismatch)?;
            *identity = self
                .work
                .call(&self.request, || native_identity(file))?
                .try_into()
                .map_err(|_| OwnerError::OwnerGuardMismatch)?;
        }
        Ok(identities)
    }

    fn verify_completed(&mut self) -> Result<(), OwnerError> {
        if let Some(direct) = &self.direct { direct.verify(&self.request, &mut self.work, &self.root)?; }
        self.checkpoint("verify-completed")?;
        if self.files.iter().any(Option::is_none) {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let files = self
            .files
            .iter_mut()
            .map(|file| file.take().ok_or(OwnerError::OwnerGuardMismatch))
            .collect::<Result<Vec<_>, _>>()?;
        let mut pins = NativePins { files, direct: None };
        let result = (|| {
            pins.verify_locators(&self.request, &mut self.work, &self.root)?;
            let snapshot = read_snapshot(&self.request, &mut self.work, &self.root, &mut pins)?;
            if snapshot.record.lifecycle != LifecycleState::Released {
                return Err(OwnerError::OwnerRecoveryQuarantined);
            }
            // The journal's existing verification above already checked its locator.
            // Borrow its held pin for ACL observation without another path open.
            pins.direct = self.direct.as_mut().and_then(|direct| direct.pin.take());
            if snapshot.installation.journal_profile {
                match &self.storage_acl {
                    Some(storage_acl) => storage_acl.verify(&self.request, &mut self.work, &pins)?,
                    None => {
                        self.storage_acl = Some(match &self.root_owner_seed {
                            Some(seed) => seed.capture_snapshot(&self.request, &mut self.work, &pins)?,
                            None => StorageAclSnapshot::capture(&self.request, &mut self.work, &pins)?,
                        });
                    }
                }
            } else if self.storage_acl.is_some() {
                return Err(OwnerError::OwnerStorageAclUnqualified);
            }
            Ok(())
        })();
        if let Some(direct) = &mut self.direct
            && pins.direct.is_some()
        {
            direct.pin = pins.direct.take();
        }
        for (slot, file) in self.files.iter_mut().zip(pins.files) {
            *slot = Some(file);
        }
        result.map_err(|error| self.failure(error))?;
        self.request.check().map_err(|error| self.failure(error))
    }

    fn capture_storage_acl(&mut self) -> Result<(), OwnerError> {
        if self.files.iter().any(Option::is_none) {
            return Err(OwnerError::OwnerStorageAclUnqualified);
        }
        self.work.read(&self.request, std::mem::size_of::<File>() * self.files.len())?;
        let files = self.files.iter_mut().filter_map(Option::take).collect();
        let direct = self.direct.as_mut().and_then(|direct| direct.pin.take());
        let mut pins = NativePins { files, direct };
        let result = (|| match &self.storage_acl {
            Some(previous) => previous.capture_after_direct_pin(&self.request, &mut self.work, &pins,
                &self.direct.as_ref().ok_or(OwnerError::OwnerStorageAclUnqualified)?.acl_before_open),
            None => match &self.root_owner_seed {
                Some(seed) => seed.capture_snapshot(&self.request, &mut self.work, &pins),
                None => StorageAclSnapshot::capture(&self.request, &mut self.work, &pins),
            },
        })();
        if let Some(direct) = &mut self.direct {
            direct.pin = pins.direct.take();
        }
        for (slot, file) in self.files.iter_mut().zip(pins.files) {
            *slot = Some(file);
        }
        self.storage_acl = Some(result?);
        Ok(())
    }

    fn verify_storage_acl(&mut self) -> Result<(), OwnerError> {
        let Some(storage_acl) = &self.storage_acl else { return Ok(()); };
        if self.files.iter().any(Option::is_none) {
            return Err(OwnerError::OwnerStorageAclUnqualified);
        }
        self.work.read(&self.request, std::mem::size_of::<File>() * self.files.len())?;
        let files = self.files.iter_mut().filter_map(Option::take).collect();
        let direct = self.direct.as_mut().and_then(|direct| direct.pin.take());
        let mut pins = NativePins { files, direct };
        let result = storage_acl.verify(&self.request, &mut self.work, &pins);
        if let Some(direct) = &mut self.direct {
            direct.pin = pins.direct.take();
        }
        for (slot, file) in self.files.iter_mut().zip(pins.files) {
            *slot = Some(file);
        }
        result
    }
}

impl Drop for InitializingDataRoot {
    fn drop(&mut self) {
        // Release resources only; never publish, repair, delete or renew context.
        self.release_native_resources();
    }
}

impl InitializationResult {
    /// Terminal output retains all native resources, then checks after release.
    pub fn emit<W: Write>(mut self, writer: &mut W) -> Result<(), OwnerError> {
        self.root.verify_completed()?;
        let line = format!("{{\"initialized\":true,\"replayed\":{}}}\n", self.replayed);
        let mut remaining = line.as_bytes();
        while !remaining.is_empty() {
            self.root.checkpoint("initialization-terminal")?;
            self.root
                .work
                .write(&self.root.request, remaining.len())
                .map_err(|error| self.root.failure(error))?;
            let count = self
                .root
                .work
                .call(&self.root.request, || {
                    self.root.effect_observed = true;
                    writer
                        .write(remaining)
                        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
                })
                .map_err(|error| self.root.failure(error))?;
            if count == 0 || count > remaining.len() {
                return Err(self.root.failure(OwnerError::OwnerAcquireOutcomeUnknown));
            }
            remaining = &remaining[count..];
        }
        self.root
            .work
            .call(&self.root.request, || {
                writer
                    .flush()
                    .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })
            .map_err(|error| self.root.failure(error))?;
        self.root.checkpoint("after-initialization-terminal")?;
        self.root.release_native_resources();
        self.root
            .work
            .arrive(&self.root.request, "after-initialization-release")
            .map_err(|error| self.root.failure(error))
    }
}

#[cfg(all(test, windows))]
#[path = "initialization_tests.rs"]
mod tests;
