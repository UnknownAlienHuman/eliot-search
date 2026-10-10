//! Single live data-root owner, OS exclusion and observation-root restoration.

use std::fs::{self, File, Metadata, OpenOptions, TryLockError};
use std::io;
#[cfg(test)]
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

use search_contracts::{DataRootId, InstallationIncarnationId, OwnerEpoch};
use search_domain::{
    AdmissionState, CancellationState, MutationObservation, MutationOutcomeClass, MutationStart,
    PostconditionState, SafetyState,
};
use search_runtime_owner::{DrainReason, OwnerError, OwnerGuard, classify_owner_mutation_boundary};

use crate::owner_composition::{LiveOwner, ShutdownReceipt};
use crate::sealed_root_lock::SealedRootLease;
#[cfg(windows)]
use crate::sealed_root_lock::SealedRootLockError;
use crate::source_roots::SourceRootCatalog;

/// Named OS ownership-primitive profile for one data-root lock file.
///
/// Conservative Phase-1 (move-map) adapter vocabulary: the ownership policy
/// stays in `search-runtime-owner` while the file effect stays a thin
/// qualified adapter here. Profiles select only the lock file name; OS
/// exclusion, validation, record and drop semantics stay byte-identical
/// across instances. The sealed effect remains owned by [`SealedRootLease`]
/// (Windows co-hold) and is never opened through this profile; the offline
/// migration borrow reuses the primary lock file under a distinct borrower
/// name. The migration-output artifact lock (`ImportOutputGuard`,
/// `DIRECT_MIGRATION_OUTPUT_*` in `control_migration_redb`) is a separate
/// mechanism with its own empty-file invariant and stays owned there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerLockProfile {
    /// Primary live owner: `acquire` and borrowed reads.
    Direct,
    /// Sealed co-hold file name; the effect is owned by [`SealedRootLease`].
    Sealed,
    /// Offline migration borrower of the primary lock (no marker cleanup).
    MigrationOutput,
}

impl OwnerLockProfile {
    /// Primary live-owner lock file beside the durable owner-state slots.
    pub(crate) const DIRECT_LOCK_FILE: &'static str = ".eliot-search-owner.lock";
    /// Sealed co-hold lock file; the authoritative opener is [`SealedRootLease`].
    pub(crate) const SEALED_LOCK_FILE: &'static str = ".eliot-search-sealed-owner.lock";

    /// Lock file name for this profile. `Direct` and `MigrationOutput` share
    /// the primary file; `Sealed` names the co-held file for documentation
    /// and coherence checks only and is never opened here.
    #[must_use]
    pub(crate) const fn file_name(self) -> &'static str {
        match self {
            Self::Direct | Self::MigrationOutput => Self::DIRECT_LOCK_FILE,
            Self::Sealed => Self::SEALED_LOCK_FILE,
        }
    }

    /// Joins the profile lock file under an already-canonical root.
    #[must_use]
    pub(crate) fn lock_path(self, canonical_root: &Path) -> PathBuf {
        canonical_root.join(self.file_name())
    }
}

/// Closed policy denial for a live owner, reusing the owner-policy code.
///
/// Takes the policy [`OwnerGuard`] type only to prove the daemon reuses the
/// declared `search-runtime-owner` vocabulary instead of duplicating it; the
/// OS exclusion itself stays in this thin adapter and is never duplicated in
/// the policy crate. `None` is the normal call shape: liveness is the held OS
/// lock alone, never the presence of a policy guard value.
#[must_use]
pub const fn live_owner_denial(_policy: Option<&OwnerGuard>) -> &'static str {
    OwnerError::DataRootAlreadyOwned.code()
}

/// Classifies an ambiguous owner-mutation boundary through the shared
/// owner-policy transition classifier (invariant 18).
///
/// A mutation that may have started with an unverified postcondition is
/// `Unknown` and requires exact readback before any retry. This helper only
/// names that classification so file-effect sites report their existing
/// closed codes consistently without duplicating the policy; it changes no
/// error string (O3 decision deferred).
#[must_use]
pub const fn classify_ambiguous_owner_write() -> MutationOutcomeClass {
    let outcome = classify_owner_mutation_boundary(MutationObservation {
        mutation_start: MutationStart::MayHaveStarted,
        postcondition: PostconditionState::Unverified,
        cancellation: CancellationState::NotCancelled,
        safety: SafetyState::Consistent,
        admission: AdmissionState::Admitted,
    });
    outcome.class
}

/// Formats an ambiguous owner-record write failure without relabelling it.
///
/// The shared transition classifier (invariant 18) is consulted while the
/// closed code stays byte-identical in Phase 1 (O3 deferred).
#[cfg(test)]
fn record_write_error(_error: &io::Error) -> String {
    debug_assert_eq!(
        classify_ambiguous_owner_write(),
        MutationOutcomeClass::Unknown
    );
    "DATA_ROOT_OWNER_RECORD_ERROR".to_owned()
}

/// Process-local exclusive owner guard and restored observation registration.
///
/// Exactly one live guard exists per data root. It holds the OS exclusion
/// (co-held with the sealed lock on Windows so no second owner type can go
/// live concurrently) together with the durable installation, physical-root,
/// executable and monotone-epoch bindings established under that exclusion.
/// The guard is non-cloneable; dropping it releases exclusion without
/// rewriting durable ownership evidence.
pub struct DataRootGuard {
    canonical_root: PathBuf,
    source_roots: SourceRootCatalog,
    owner: LiveOwner,
    native_objects: Option<crate::owner_composition::NativeLayoutPins>,
    root_file: File,
    clear_marker_on_drop: bool,
    // Field-drop order closes root/layout resources before native exclusions.
    sealed: Option<SealedRootLease>,
    file: File,
}

#[derive(Clone, Copy)]
enum NativeOwnerOpen {
    ExistingMutating,
    #[cfg(test)]
    LegacyHarness,
}

impl NativeOwnerOpen {
    const fn creates_layout(self) -> bool {
        match self {
            Self::ExistingMutating => false,
            #[cfg(test)]
            Self::LegacyHarness => true,
        }
    }
}

/// Read-only root authority valid only while the existing native exclusions
/// and exact durable predecessor binding remain held by the inspection call.
/// It has no constructor, clone, lifecycle or registration-mutation methods.
pub struct InspectedDataRoot {
    canonical_root: PathBuf,
    snapshot: crate::owner_composition::ExistingOwnerSnapshot,
    source_roots: SourceRootCatalog,
    native_objects: crate::owner_composition::NativeLayoutPins,
}

impl InspectedDataRoot {
    /// Verify the existing barrier/owner and retained native layout before a
    /// child read. This never advances an epoch or resolves a secret.
    pub(crate) fn verify_existing(&self) -> Result<(), String> {
        check_existing_root_barriers(&self.canonical_root)?;
        self.native_objects
            .verify(&self.canonical_root)
            .and_then(|()| {
                crate::owner_composition::verify_native_installation(&self.canonical_root)
            })
            .and_then(|()| self.snapshot.verify_unchanged(&self.canonical_root))
            .map_err(|error| error.code().to_owned())
    }
    /// Existing canonical root under the held native exclusions.
    pub(crate) fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    /// Read-only observation registration without file recovery.
    pub(crate) const fn source_roots(&self) -> &SourceRootCatalog {
        &self.source_roots
    }

    /// Existing root/owner binding, never a new owner epoch.
    pub(crate) fn journal_owner_inputs(
        &self,
    ) -> (InstallationIncarnationId, DataRootId, OwnerEpoch) {
        self.snapshot.journal_owner_inputs()
    }

    /// Exact durable record revision observed under the native exclusion.
    pub(crate) fn generation(&self) -> u64 {
        self.snapshot.generation()
    }
}

impl DataRootGuard {
    /// Executes an existing read under both native exclusions without creating
    /// locks, credentials, catalogs or owner records, or performing recovery.
    pub(crate) fn with_inspection<T>(
        path: &Path,
        operation: impl FnOnce(&InspectedDataRoot) -> Result<T, String>,
    ) -> Result<T, String> {
        Self::with_existing_lock(path, |root| {
            check_existing_root_barriers(root)?;
            let native_objects = crate::owner_composition::retain_native_installation(root)
                .map_err(|error| error.code().to_owned())?;
            let snapshot = crate::owner_composition::inspect_existing_owner(root)
                .map_err(|error| error.code().to_owned())?;
            snapshot
                .require_released()
                .map_err(|error| error.code().to_owned())?;
            crate::catalog_presence::require_existing(root)?;
            let source_roots = SourceRootCatalog::load_existing_owned(root)
                .map_err(|error| error.code().to_owned())?;
            let inspected = InspectedDataRoot {
                canonical_root: root.to_owned(),
                snapshot,
                source_roots,
                native_objects,
            };
            let result = operation(&inspected);
            inspected.verify_existing()?;
            result
        })
    }

    /// Harness-only create-or-open retained for historical isolated fixtures.
    #[cfg(test)]
    pub(crate) fn acquire(path: &Path) -> Result<Self, String> {
        Self::acquire_root(path, NativeOwnerOpen::LegacyHarness)
    }

    /// Establishes one mutating owner over a fully existing released root.
    /// Missing locks/installation/catalog require explicit initialization.
    pub(crate) fn open_existing(path: &Path) -> Result<Self, String> {
        Self::acquire_root(path, NativeOwnerOpen::ExistingMutating)
    }

    fn acquire_root(path: &Path, mode: NativeOwnerOpen) -> Result<Self, String> {
        let metadata = fs::symlink_metadata(path).map_err(|_| "DATA_ROOT_OPEN_ERROR".to_owned())?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err("DATA_ROOT_LINK_DENIED".to_owned());
        }
        if !metadata.is_dir() {
            return Err("DATA_ROOT_NOT_DIRECTORY".to_owned());
        }
        let canonical_root =
            fs::canonicalize(path).map_err(|_| "DATA_ROOT_CANONICALIZE_ERROR".to_owned())?;
        let canonical_metadata =
            fs::symlink_metadata(&canonical_root).map_err(|_| "DATA_ROOT_OPEN_ERROR".to_owned())?;
        if canonical_metadata.file_type().is_symlink()
            || is_reparse(&canonical_metadata)
            || !canonical_metadata.is_dir()
        {
            return Err("DATA_ROOT_IDENTITY_AMBIGUOUS".to_owned());
        }

        let root_file = crate::owner_composition::open_bound_directory(&canonical_root)
            .map_err(|error| error.code().to_owned())?;
        let lock_path = OwnerLockProfile::Direct.lock_path(&canonical_root);
        // Phase-1 instance coherence: the sealed profile names the co-held
        // file while sharing exclusion semantics (debug-only check).
        debug_assert_eq!(
            OwnerLockProfile::Sealed.file_name(),
            OwnerLockProfile::SEALED_LOCK_FILE
        );
        match fs::symlink_metadata(&lock_path) {
            Ok(metadata)
                if metadata.file_type().is_symlink()
                    || is_reparse(&metadata)
                    || !metadata.is_file() =>
            {
                return Err("DATA_ROOT_LOCK_OBJECT_INVALID".to_owned());
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound && mode.creates_layout() => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err("DATA_ROOT_EXISTING_LOCK_REQUIRED".to_owned());
            }
            Err(_) => return Err("DATA_ROOT_LOCK_OPEN_ERROR".to_owned()),
        }
        let mut options = OpenOptions::new();
        options
            .create(mode.creates_layout())
            .truncate(false)
            .read(true)
            .write(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x0020_0000).share_mode(0x3);
        }
        let file = options
            .open(&lock_path)
            .map_err(|_| "DATA_ROOT_LOCK_OPEN_ERROR".to_owned())?;
        #[cfg(test)]
        let mut file = file;
        let opened = file
            .metadata()
            .map_err(|_| "DATA_ROOT_LOCK_OPEN_ERROR".to_owned())?;
        if !opened.is_file() || is_reparse(&opened) {
            return Err("DATA_ROOT_LOCK_OBJECT_INVALID".to_owned());
        }
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(live_owner_denial(None).to_owned()),
            Err(TryLockError::Error(_)) => return Err("DATA_ROOT_LOCK_ERROR".to_owned()),
        }
        crate::owner_composition::verify_existing_locator(&file, &lock_path)
            .map_err(|error| error.code().to_owned())?;
        // Co-hold the sealed exclusion before reopening durable state so the
        // harness-era sealed authority can never go live on the same root
        // concurrently. A held sealed lease denies exactly like our own lock.
        #[cfg(windows)]
        let sealed = if mode.creates_layout() {
            acquire_sealed_exclusion(&canonical_root)?
        } else {
            Some(SealedRootLease::acquire_existing(&canonical_root).map_err(
                |error| match error {
                    SealedRootLockError::AlreadyOwned => live_owner_denial(None).to_owned(),
                    _ => "DATA_ROOT_LOCK_ERROR".to_owned(),
                },
            )?)
        };
        #[cfg(not(windows))]
        let sealed: Option<SealedRootLease> = None;
        // Bind installation, physical root, executable, process-creation
        // token and the next monotone epoch before restoring anything else.
        // Any binding disagreement or unprovable state fails closed here,
        // releasing both exclusions on unwind without touching catalogs.
        let native_objects = if !mode.creates_layout() {
            check_existing_root_barriers(&canonical_root)?;
            let pins = crate::owner_composition::retain_native_installation(&canonical_root)
                .map_err(|error| error.code().to_owned())?;
            crate::catalog_presence::require_existing(&canonical_root)?;
            crate::owner_composition::inspect_existing_owner(&canonical_root)
                .and_then(|snapshot| snapshot.require_existing_mutation())
                .map_err(|error| error.code().to_owned())?;
            crate::plaintext_direct_store::DirectStore::validate_existing_admission(
                &canonical_root,
            )
            .map_err(|_| "DIRECT_EXISTING_CATALOG_INVALID".to_owned())?;
            Some(pins)
        } else {
            None
        };
        // Registration refusal must happen before durable owner succession.
        let source_roots = match mode {
            #[cfg(test)]
            NativeOwnerOpen::LegacyHarness => SourceRootCatalog::load_owned(&canonical_root),
            NativeOwnerOpen::ExistingMutating => {
                SourceRootCatalog::load_existing_owned(&canonical_root)
            }
        }
        .map_err(|error| error.code().to_owned())?;
        crate::owner_composition::verify_bound_directory(&root_file, &canonical_root)
            .map_err(|error| error.code().to_owned())?;
        let owner = match mode {
            #[cfg(test)]
            NativeOwnerOpen::LegacyHarness => crate::owner_composition::establish(&canonical_root),
            NativeOwnerOpen::ExistingMutating => {
                crate::owner_composition::establish_existing(&canonical_root)
            }
        }
        .map_err(|error| error.code().to_owned())?;
        #[cfg(test)]
        if mode.creates_layout() {
            let created_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "SYSTEM_CLOCK_BEFORE_EPOCH".to_owned())?
                .as_millis();
            let record = format!(
                "{{\"schema\":1,\"pid\":{},\"created_at_unix_ms\":{},\"state\":\"ACTIVE\"}}\n",
                std::process::id(),
                created_at,
            );
            if record.len() > 4 * 1024 {
                let _ = file.unlock();
                return Err("DATA_ROOT_OWNER_RECORD_TOO_LARGE".to_owned());
            }
            file.set_len(0)
                .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
                .and_then(|()| file.write_all(record.as_bytes()))
                .and_then(|()| file.sync_all())
                .map_err(|error| record_write_error(&error))?;
        }

        Ok(Self {
            sealed,
            file,
            canonical_root,
            source_roots,
            owner,
            root_file,
            native_objects,
            clear_marker_on_drop: mode.creates_layout(),
        })
    }

    /// Canonical local root protected by this guard.
    pub(crate) fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    /// Revalidates the retained exclusions, physical root and durable owner.
    /// A mutation marker may be armed, so it is deliberately not cleared here.
    pub(crate) fn verify_existing(&self) -> Result<(), String> {
        if let Some(pins) = &self.native_objects {
            pins.verify(&self.canonical_root)
                .map_err(|error| error.code().to_owned())?;
        }
        if !self.clear_marker_on_drop {
            crate::owner_composition::verify_native_installation(&self.canonical_root)
                .map_err(|error| error.code().to_owned())?;
        }
        crate::owner_composition::verify_bound_directory(&self.root_file, &self.canonical_root)
            .and_then(|()| {
                crate::owner_composition::verify_existing_locator(
                    &self.file,
                    &OwnerLockProfile::Direct.lock_path(&self.canonical_root),
                )
            })
            .and_then(|()| self.owner.verify_existing())
            .map_err(|error| error.code().to_owned())?;
        if let Some(sealed) = &self.sealed {
            sealed
                .verify_existing(&self.canonical_root)
                .map_err(|_| "DATA_ROOT_LOCK_IDENTITY_CHANGED".to_owned())?;
        }
        Ok(())
    }

    /// Clears a dispatched mutation barrier only under the same live binding.
    pub(crate) fn clear_mutation_marker(&self) -> Result<(), String> {
        self.verify_existing()?;
        crate::catalog_quarantine::clear(&self.canonical_root)
    }

    /// Bound monotone owner epoch of this live incarnation.
    pub(crate) const fn epoch(&self) -> u64 {
        self.owner.epoch().get()
    }

    /// Whether the predecessor left an unreleased record behind.
    pub(crate) const fn recovered_previous_active(&self) -> bool {
        self.owner.recovered_previous_active()
    }

    /// Exact owner-side journal identity inputs for the redb follow-up.
    pub(crate) const fn journal_owner_inputs(
        &self,
    ) -> (InstallationIncarnationId, DataRootId, OwnerEpoch) {
        self.owner.journal_owner_inputs()
    }

    /// Persists `DRAINING` intent; new ordinary work must stop first.
    ///
    /// # Errors
    ///
    /// A poisoned or already-released guard is refused with the closed
    /// owner-policy code.
    pub(crate) fn begin_drain(&mut self, reason: DrainReason) -> Result<(), String> {
        self.verify_existing()?;
        self.owner
            .begin_drain(reason)
            .map_err(|error| error.code().to_owned())
    }

    /// Persists the `RELEASED` tombstone and consumes the guard.
    ///
    /// Call only after dependencies shut down in reverse startup order and
    /// every storage and process resource is closed: the following drop
    /// releases exclusion last.
    ///
    /// # Errors
    ///
    /// Release without a prior drain, or an unprovable durable outcome,
    /// fails closed with the closed owner-policy code.
    pub(crate) fn release_cleanly(mut self) -> Result<ShutdownReceipt, String> {
        self.verify_existing()?;
        self.owner
            .release_cleanly()
            .map_err(|error| error.code().to_owned())
    }

    pub(crate) const fn source_roots(&self) -> &SourceRootCatalog {
        &self.source_roots
    }

    pub(crate) const fn source_roots_mut(&mut self) -> &mut SourceRootCatalog {
        &mut self.source_roots
    }
}

/// Shared existing-root fences. Metadata failures and unknown markers deny
/// without inspecting source contents or opening a database for repair.
fn check_existing_root_barriers(root: &Path) -> Result<(), String> {
    crate::catalog_quarantine::check(root)?;
    for name in [
        "control/catalog-quarantine.tmp",
        ".eliot-search-initialization-intent.v1",
    ] {
        match fs::symlink_metadata(root.join(name)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            _ => return Err("DATA_ROOT_RECOVERY_REQUIRED".to_owned()),
        }
    }
    // The accepted redb version has no existing-only non-repairing opener.
    // Never call its read/write native opener from an inspection capability.
    match fs::symlink_metadata(root.join("control/control.redb")) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        _ => Err("DATA_ROOT_LAYOUT_UNSUPPORTED".to_owned()),
    }
}

impl Drop for DataRootGuard {
    fn drop(&mut self) {
        // The co-held sealed exclusion must survive the full guard lifetime;
        // its field drops after root/layout resources, together with the
        // primary lock, after every durable transition finished.
        debug_assert!(self.sealed.as_ref().is_none_or(SealedRootLease::is_held));
        if self.clear_marker_on_drop {
            let _ = self.file.set_len(0);
            let _ = self.file.sync_all();
        }
        // Field drops release the native pair only after all durable state.
    }
}

/// Co-holds the sealed exclusion behind the already-held primary lock.
///
/// A live sealed holder denies exactly like a live primary holder, so the
/// two lock files never admit two concurrent owners. Observation-only
/// failures map to the existing primary lock codes.
///
/// # Errors
///
/// Returns `DATA_ROOT_ALREADY_OWNED` for a live sealed holder and the
/// matching primary lock code for unusable roots.
#[cfg(windows)]
fn acquire_sealed_exclusion(canonical_root: &Path) -> Result<Option<SealedRootLease>, String> {
    match SealedRootLease::acquire(canonical_root) {
        Ok(lease) => Ok(Some(lease)),
        Err(SealedRootLockError::AlreadyOwned) => {
            Err(search_runtime_owner::OwnerError::DataRootAlreadyOwned
                .code()
                .to_owned())
        }
        Err(SealedRootLockError::InvalidDataRoot) => Err("DATA_ROOT_NOT_DIRECTORY".to_owned()),
        Err(SealedRootLockError::ReparsePointDenied) => Err("DATA_ROOT_LINK_DENIED".to_owned()),
        Err(_) => Err("DATA_ROOT_LOCK_ERROR".to_owned()),
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &Metadata) -> bool {
    false
}
