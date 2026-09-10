//! Concrete bounded runtime helpers for the primary daemon.
//!
//! Data-root exclusion uses the operating-system file-lock API. Observation-root
//! registration is restored only while that lock is held. One-shot file scanning
//! verifies identity and metadata through one final handle before and after read.
//!
//! Live ownership is the single [`DataRootGuard`]: the OS exclusion plus the
//! durable installation/root/executable/epoch protocol from
//! `owner_composition`, reusing the `search-runtime-owner` policy codes. No
//! second owner type exists on this path.

use std::fs::{self, File, Metadata, OpenOptions, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use search_contracts::{DataRootId, InstallationIncarnationId, OwnerEpoch};
use search_runtime_owner::DrainReason;

use crate::owner_composition::{LiveOwner, ShutdownReceipt};
use crate::sealed_root_lock::SealedRootLease;
#[cfg(windows)]
use crate::sealed_root_lock::SealedRootLockError;
use crate::source_roots::SourceRootCatalog;

pub const MAX_SCAN_INPUT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_SCAN_QUERY_BYTES: usize = 64 * 1024;
pub const MAX_SCAN_MATCHES: usize = 100_000;

/// Composition-prerequisite readiness for one daemon state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionReadiness {
    pub(crate) configuration_ready: bool,
    pub(crate) runtime_owner_ready: bool,
    pub(crate) control_store_ready: bool,
}

/// Store and channel readiness for one daemon state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreReadiness {
    pub(crate) secret_store_ready: bool,
    pub(crate) endpoint_ready: bool,
    pub(crate) direct_store_ready: bool,
}

/// Capability availability for one daemon state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityAvailability {
    pub(crate) source_backed_search_available: bool,
    pub(crate) development_stdin_scan_available: bool,
    pub(crate) development_file_scan_available: bool,
}

/// Truthful capability summary for one daemon composition state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Health {
    pub(crate) composition: CompositionReadiness,
    pub(crate) stores: StoreReadiness,
    pub(crate) capabilities: CapabilityAvailability,
}

impl Health {
    /// Process shell before a data root is opened.
    pub(crate) const SHELL: Self = Self {
        composition: CompositionReadiness {
            configuration_ready: true,
            runtime_owner_ready: false,
            control_store_ready: false,
        },
        stores: StoreReadiness {
            secret_store_ready: false,
            endpoint_ready: true,
            direct_store_ready: false,
        },
        capabilities: CapabilityAvailability {
            source_backed_search_available: false,
            development_stdin_scan_available: true,
            development_file_scan_available: true,
        },
    };

    /// Owner-fenced persistent DIRECT source store is open and verified.
    pub(crate) const DIRECT_STORE: Self = Self {
        composition: CompositionReadiness {
            configuration_ready: true,
            runtime_owner_ready: true,
            control_store_ready: true,
        },
        stores: StoreReadiness {
            secret_store_ready: cfg!(windows),
            endpoint_ready: true,
            direct_store_ready: true,
        },
        capabilities: CapabilityAvailability {
            source_backed_search_available: true,
            development_stdin_scan_available: true,
            development_file_scan_available: true,
        },
    };

    pub(crate) fn json(self) -> String {
        format!(
            concat!(
                "{{\"status\":\"development_shell\",",
                "\"configuration_ready\":{},",
                "\"runtime_owner_ready\":{},",
                "\"control_store_ready\":{},",
                "\"secret_store_ready\":{},",
                "\"endpoint_ready\":{},",
                "\"direct_store_ready\":{},",
                "\"source_backed_search_available\":{},",
                "\"development_stdin_scan_available\":{},",
                "\"development_file_scan_available\":{}}}"
            ),
            self.composition.configuration_ready,
            self.composition.runtime_owner_ready,
            self.composition.control_store_ready,
            self.stores.secret_store_ready,
            self.stores.endpoint_ready,
            self.stores.direct_store_ready,
            self.capabilities.source_backed_search_available,
            self.capabilities.development_stdin_scan_available,
            self.capabilities.development_file_scan_available,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanMatch {
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) line: usize,
    pub(crate) column_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanCoverage {
    pub(crate) input_bytes: usize,
    pub(crate) complete: bool,
    pub(crate) match_limit_reached: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanResult {
    pub(crate) matches: Vec<ScanMatch>,
    pub(crate) coverage: ScanCoverage,
}

/// Uses the same bounded literal engine as retained DIRECT preparation.
/// Only the historical one-shot LF/byte-coordinate projection belongs here.
pub fn scan_text(
    text: &str,
    query: &str,
    ascii_insensitive: bool,
) -> Result<ScanResult, String> {
    use search_exact::literal::{LiteralError, LiteralLimits, scan_chunks};

    let result = scan_chunks(&[text], query, ascii_insensitive, LiteralLimits {
        max_query_bytes: MAX_SCAN_QUERY_BYTES,
        max_input_bytes: MAX_SCAN_INPUT_BYTES,
        max_chunks: 1,
        max_matches: MAX_SCAN_MATCHES,
    }).map_err(|error| match error {
        LiteralError::EmptyQuery => "SCAN_QUERY_EMPTY",
        LiteralError::QueryTooLarge => "SCAN_QUERY_TOO_LARGE",
        LiteralError::InputTooLarge => "SCAN_INPUT_TOO_LARGE",
        other => other.code(),
    }.to_owned())?;
    let coverage = ScanCoverage {
        input_bytes: result.input_bytes,
        complete: result.complete(),
        match_limit_reached: result.match_limit_reached,
    };
    let (mut consumed, mut line_start, mut line) = (0, 0, 0);
    let matches = result.matches.into_iter().map(|range| {
        // Ranges arrive in increasing start order, including overlaps. Count
        // each prefix byte only once; do not allocate a table for every newline.
        // LF alone advances this legacy API's line coordinate. CR/NUL remain
        // source bytes; retained preparation keeps its own materializer policy.
        for (offset, byte) in text.as_bytes()[consumed..range.start].iter().enumerate() {
            if *byte == b'\n' {
                line += 1;
                line_start = consumed + offset + 1;
            }
        }
        consumed = range.start;
        ScanMatch {
            byte_start: range.start,
            byte_end: range.end,
            line,
            column_bytes: range.start - line_start,
        }
    }).collect();
    Ok(ScanResult { matches, coverage })
}

/// Reads bounded UTF-8 from standard input.
pub fn read_stdin_bounded() -> Result<String, String> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(u64::try_from(MAX_SCAN_INPUT_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("SCAN_READ_ERROR:{error}"))?;
    if bytes.len() > MAX_SCAN_INPUT_BYTES {
        return Err("SCAN_INPUT_TOO_LARGE".to_owned());
    }
    String::from_utf8(bytes).map_err(|_| "SCAN_INPUT_INVALID_UTF8".to_owned())
}

/// Reads one regular non-link file through the shared safe-reader kernel.
///
/// The platform adapter proves final-object/ancestor containment on the
/// opened handle under the file's admitted parent directory and the kernel
/// revalidates the same handle after the read. Bytes are inert copies and
/// are never executed; failures carry closed codes without paths or raw OS
/// text.
pub fn read_file_bounded(path: &Path) -> Result<String, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| "SCAN_FILE_ACCESS_DENIED".to_owned())?
            .join(path)
    };
    let parent = absolute
        .parent()
        .ok_or_else(|| "SCAN_FILE_PATH_DENIED".to_owned())?;
    let full = crate::safe_reader_adapter::read_full_file_via_kernel(
        &absolute,
        parent,
        MAX_SCAN_INPUT_BYTES,
    )
    .map_err(map_full_read_error)?;
    if full.bytes.len() > MAX_SCAN_INPUT_BYTES
        || u64::try_from(full.bytes.len()).unwrap_or(u64::MAX) != full.source_bytes
    {
        return Err("SCAN_FILE_CHANGED_DURING_READ".to_owned());
    }
    String::from_utf8(full.bytes).map_err(|_| "SCAN_INPUT_INVALID_UTF8".to_owned())
}

/// Maps a kernel-verified read failure to the SCAN namespace without paths,
/// bytes or raw OS error text.
fn map_full_read_error(error: crate::safe_reader_adapter::FullReadError) -> String {
    use crate::safe_reader_adapter::{AdapterError, FullReadError};
    use search_safe_reader::SafeReadError;
    match error {
        FullReadError::Adapter(adapter) => match adapter {
            AdapterError::PathDenied => "SCAN_FILE_PATH_DENIED".to_owned(),
            AdapterError::LinkDenied | AdapterError::AncestorReparseDenied => {
                "SCAN_FILE_LINK_DENIED".to_owned()
            }
            AdapterError::EscapeDenied => "SCAN_FILE_ESCAPE_DENIED".to_owned(),
            AdapterError::RootRelocated => "SCAN_FILE_ROOT_RELOCATED".to_owned(),
            AdapterError::NotRegular => "SCAN_FILE_NOT_REGULAR".to_owned(),
            AdapterError::FinalObjectInvalid | AdapterError::DeviceDenied => {
                "SCAN_FILE_FINAL_OBJECT_INVALID".to_owned()
            }
            AdapterError::HardlinkDenied => "SCAN_FILE_HARDLINK_DENIED".to_owned(),
            AdapterError::AccessDenied => "SCAN_FILE_ACCESS_DENIED".to_owned(),
            AdapterError::TooLarge => "SCAN_INPUT_TOO_LARGE".to_owned(),
            AdapterError::ReceiptDenied => {
                "SCAN_FILE_METADATA_ERROR:SAFE_ADAPTER_RECEIPT_DENIED".to_owned()
            }
        },
        FullReadError::Kernel(kernel) => match kernel {
            SafeReadError::RangeOutsideSource
            | SafeReadError::EofMismatch
            | SafeReadError::ReadLengthMismatch
            | SafeReadError::StableIdentityMismatch
            | SafeReadError::HandleChangedDuringRead
            | SafeReadError::BackendFailure => "SCAN_FILE_CHANGED_DURING_READ".to_owned(),
            SafeReadError::RootIdentityMismatch => "SCAN_FILE_ESCAPE_DENIED".to_owned(),
            SafeReadError::UnsupportedFileKind => "SCAN_FILE_FINAL_OBJECT_INVALID".to_owned(),
            SafeReadError::ReparseBoundaryDenied => "SCAN_FILE_LINK_DENIED".to_owned(),
            SafeReadError::SecurityDenied | SafeReadError::SecurityRevisionMismatch => {
                "SCAN_FILE_ACCESS_DENIED".to_owned()
            }
            SafeReadError::SourceSizeInvalid => "SCAN_INPUT_TOO_LARGE".to_owned(),
            SafeReadError::Cancelled => "SCAN_FILE_READ_CANCELLED".to_owned(),
            SafeReadError::InvalidLimits
            | SafeReadError::InvalidPathToken
            | SafeReadError::InvalidReadLength
            | SafeReadError::RangeOverflow
            | SafeReadError::InvalidRetryPolicy
            | SafeReadError::ReceiptMissing => "SCAN_FILE_READ_INVALID".to_owned(),
        },
    }
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
    sealed: Option<SealedRootLease>,
    file: File,
    canonical_root: PathBuf,
    source_roots: SourceRootCatalog,
    owner: LiveOwner,
}

impl DataRootGuard {
    /// Acquires the OS lock before restoring roots or publishing readiness.
    pub(crate) fn acquire(path: &Path) -> Result<Self, String> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| format!("DATA_ROOT_OPEN_ERROR:{error}"))?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err("DATA_ROOT_LINK_DENIED".to_owned());
        }
        if !metadata.is_dir() {
            return Err("DATA_ROOT_NOT_DIRECTORY".to_owned());
        }
        let canonical_root = fs::canonicalize(path)
            .map_err(|error| format!("DATA_ROOT_CANONICALIZE_ERROR:{error}"))?;
        let canonical_metadata = fs::symlink_metadata(&canonical_root)
            .map_err(|error| format!("DATA_ROOT_OPEN_ERROR:{error}"))?;
        if canonical_metadata.file_type().is_symlink()
            || is_reparse(&canonical_metadata)
            || !canonical_metadata.is_dir()
        {
            return Err("DATA_ROOT_IDENTITY_AMBIGUOUS".to_owned());
        }

        let lock_path = canonical_root.join(".eliot-search-owner.lock");
        match fs::symlink_metadata(&lock_path) {
            Ok(metadata) if metadata.file_type().is_symlink()
                || is_reparse(&metadata) || !metadata.is_file() =>
            {
                return Err("DATA_ROOT_LOCK_OBJECT_INVALID".to_owned());
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("DATA_ROOT_LOCK_OPEN_ERROR:{error}")),
        }
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| format!("DATA_ROOT_LOCK_OPEN_ERROR:{error}"))?;
        let opened = file
            .metadata()
            .map_err(|error| format!("DATA_ROOT_LOCK_OPEN_ERROR:{error}"))?;
        if !opened.is_file() || is_reparse(&opened) {
            return Err("DATA_ROOT_LOCK_OBJECT_INVALID".to_owned());
        }
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err("DATA_ROOT_ALREADY_OWNED".to_owned()),
            Err(TryLockError::Error(error)) => return Err(format!("DATA_ROOT_LOCK_ERROR:{error}")),
        }
        // Co-hold the sealed exclusion before reopening durable state so the
        // harness-era sealed authority can never go live on the same root
        // concurrently. A held sealed lease denies exactly like our own lock.
        #[cfg(windows)]
        let sealed = acquire_sealed_exclusion(&canonical_root)?;
        #[cfg(not(windows))]
        let sealed: Option<SealedRootLease> = None;
        // Bind installation, physical root, executable, process-creation
        // token and the next monotone epoch before restoring anything else.
        // Any binding disagreement or unprovable state fails closed here,
        // releasing both exclusions on unwind without touching catalogs.
        let owner = crate::owner_composition::establish(&canonical_root)
            .map_err(|error| error.code().to_owned())?;
        let source_roots = SourceRootCatalog::load_owned(&canonical_root)
            .map_err(|error| error.code().to_owned())?;

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
            .map_err(|error| format!("DATA_ROOT_OWNER_RECORD_ERROR:{error}"))?;

        Ok(Self {
            sealed,
            file,
            canonical_root,
            source_roots,
            owner,
        })
    }

    /// Canonical local root protected by this guard.
    pub(crate) fn canonical_root(&self) -> &Path {
        &self.canonical_root
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

impl Drop for DataRootGuard {
    fn drop(&mut self) {
        // The co-held sealed exclusion must survive the full guard lifetime;
        // it is released here by field-drop order (sealed first) together
        // with the primary lock, after every durable transition finished.
        debug_assert!(self.sealed.as_ref().is_none_or(SealedRootLease::is_held));
        let _ = self.file.set_len(0);
        let _ = self.file.sync_all();
        let _ = self.file.unlock();
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

#[cfg(test)]
mod owner_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Scratch {
        base: PathBuf,
        data: PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let base = std::env::temp_dir().join(format!(
                "eliot-owner-guard-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let data = base.join("data");
            fs::create_dir_all(&data).unwrap();
            Self { base, data }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn registration_reopens_under_the_same_exclusive_owner_lock() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("eliot-owner-{}-{stamp}", std::process::id()));
        let data = base.join("data");
        let source = base.join("source");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir(&source).unwrap();
        {
            let mut owner = DataRootGuard::acquire(&data).unwrap();
            owner.source_roots_mut().add(&source).unwrap();
            assert!(matches!(DataRootGuard::acquire(&data), Err(error) if error == "DATA_ROOT_ALREADY_OWNED"));
        }
        {
            let owner = DataRootGuard::acquire(&data).unwrap();
            assert_eq!(owner.source_roots().configured_count(), 1);
            assert_eq!(owner.source_roots().available_count(), 1);
        }
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn single_guard_binds_epoch_and_stable_identities_across_succession() {
        let scratch = Scratch::new();
        let (incarnation, root, _) = {
            let guard = DataRootGuard::acquire(&scratch.data).unwrap();
            assert_eq!(guard.epoch(), 1);
            assert!(!guard.recovered_previous_active());
            guard.journal_owner_inputs()
        };
        {
            let guard = DataRootGuard::acquire(&scratch.data).unwrap();
            assert_eq!(guard.epoch(), 2);
            assert!(guard.recovered_previous_active());
            let (incarnation_next, root_next, epoch_next) = guard.journal_owner_inputs();
            assert_eq!(incarnation, incarnation_next);
            assert_eq!(root, root_next);
            assert_eq!(epoch_next.get(), 2);
        }
    }

    #[test]
    fn guard_stays_live_until_released_then_successor_advances() {
        let scratch = Scratch::new();
        let first = DataRootGuard::acquire(&scratch.data).unwrap();
        assert!(
            matches!(DataRootGuard::acquire(&scratch.data), Err(error) if error == "DATA_ROOT_ALREADY_OWNED")
        );
        drop(first);
        let second = DataRootGuard::acquire(&scratch.data).unwrap();
        assert_eq!(second.epoch(), 2);
    }

    #[test]
    fn release_requires_prior_drain_and_persists_one_tombstone() {
        use search_runtime_owner::OwnerError;

        let scratch = Scratch::new();
        let guard = DataRootGuard::acquire(&scratch.data).unwrap();
        assert_eq!(
            guard.release_cleanly().map(|_| ()),
            Err(OwnerError::OwnerDrainRequired.code().to_owned())
        );
        let mut guard = DataRootGuard::acquire(&scratch.data).unwrap();
        // The refused release above wrote nothing: the next incarnation
        // still succeeds the dropped (unreleased) guard at epoch two.
        assert_eq!(guard.epoch(), 2);
        guard
            .begin_drain(search_runtime_owner::DrainReason::Shutdown)
            .unwrap();
        let receipt = guard.release_cleanly().unwrap();
        assert_eq!(receipt.epoch.get(), 2);
        let next = DataRootGuard::acquire(&scratch.data).unwrap();
        assert_eq!(next.epoch(), 3);
        assert!(!next.recovered_previous_active());
    }

    #[test]
    fn relocated_copy_is_denied_while_original_advances() {
        let scratch = Scratch::new();
        drop(DataRootGuard::acquire(&scratch.data).unwrap());
        let moved = scratch.base.join("moved");
        fs::create_dir(&moved).unwrap();
        for name in [
            ".eliot-search-installation.v1",
            ".eliot-search-owner-state-a.v1",
        ] {
            let bytes = fs::read(scratch.data.join(name)).unwrap();
            fs::write(moved.join(name), &bytes).unwrap();
        }
        assert!(
            matches!(DataRootGuard::acquire(&moved), Err(error) if error == "OWNER_GUARD_MISMATCH")
        );
        // The denied copy wrote no successor slot of its own.
        assert!(!moved.join(".eliot-search-owner-state-b.v1").exists());
        let guard = DataRootGuard::acquire(&scratch.data).unwrap();
        assert_eq!(guard.epoch(), 2);
    }

    #[test]
    fn corrupt_owner_state_quarantines_without_touching_catalogs() {
        let scratch = Scratch::new();
        drop(DataRootGuard::acquire(&scratch.data).unwrap());
        for name in [
            ".eliot-search-owner-state-a.v1",
            ".eliot-search-owner-state-b.v1",
        ] {
            fs::write(scratch.data.join(name), b"corrupt-and-preserved").unwrap();
        }
        assert!(
            matches!(DataRootGuard::acquire(&scratch.data), Err(error) if error == "OWNER_RECOVERY_QUARANTINED")
        );
        assert_eq!(
            fs::read(scratch.data.join(".eliot-search-owner-state-a.v1")).unwrap(),
            b"corrupt-and-preserved"
        );
    }
}

#[cfg(test)]
mod scan_tests {
    use super::*;

    /// Counts newline bytes with an explicit loop.
    fn count_newlines(bytes: &[u8]) -> usize {
        let mut count = 0_usize;
        for byte in bytes {
            if *byte == b'\n' {
                count += 1;
            }
        }
        count
    }

    #[test]
    fn shared_matcher_preserves_legacy_coordinates_and_ascii_only_folding() {
        for text in ["", "aaaaa", "aAéAa", "ΑαA\0a", "\n\nx\r\nX", "a\rb", "𐀀a𐀀", "a\r\nβ\nz"] {
            for query in ["a", "aa", "aaa", "é", "α", "𐀀", "\n", "\r\n", "\0", "\nX", "absent"] {
                for insensitive in [false, true] {
                    // Independent bounded oracle; never called by runtime code.
                    let expected = text.as_bytes().windows(query.len()).enumerate()
                        .filter(|(start, bytes)| text.is_char_boundary(*start)
                            && text.is_char_boundary(start + query.len())
                            && if insensitive { bytes.eq_ignore_ascii_case(query.as_bytes()) }
                               else { *bytes == query.as_bytes() })
                        .map(|(start, _)| {
                            let prefix = &text.as_bytes()[..start];
                            let line_start = prefix.iter().rposition(|byte| *byte == b'\n').map_or(0, |index| index + 1);
                            ScanMatch { byte_start: start, byte_end: start + query.len(),
                                line: count_newlines(prefix), column_bytes: start - line_start }
                        }).collect::<Vec<_>>();
                    let actual = scan_text(text, query, insensitive).unwrap();
                    assert_eq!(actual.matches, expected, "text={text:?} query={query:?} insensitive={insensitive}");
                    assert_eq!(actual.coverage, ScanCoverage { input_bytes: text.len(), complete: true, match_limit_reached: false });
                }
            }
        }
    }

    #[test]
    fn output_ceiling_is_incomplete_only_when_an_additional_match_exists() {
        for extra in [0, 1] {
            let text = "a".repeat(MAX_SCAN_MATCHES + extra);
            let actual = scan_text(&text, "a", false).unwrap();
            assert_eq!(actual.matches.len(), MAX_SCAN_MATCHES);
            assert_eq!(actual.matches.last().unwrap().byte_start, MAX_SCAN_MATCHES - 1);
            assert_eq!(actual.coverage.complete, extra == 0);
            assert_eq!(actual.coverage.match_limit_reached, extra != 0);
        }
    }

    #[test]
    fn repeated_long_prefix_uses_the_shared_linear_matcher() {
        let count = 1024 * 1024;
        let text = format!("{}b", "a".repeat(count));
        let query = format!("{}b", "a".repeat(8192));
        let actual = scan_text(&text, &query, false).unwrap();
        assert_eq!(actual.matches, vec![ScanMatch { byte_start: count - 8192,
            byte_end: count + 1, line: 0, column_bytes: count - 8192 }]);
        assert!(actual.coverage.complete);
    }

    #[test]
    fn caller_errors_still_use_the_existing_scan_namespace() {
        assert_eq!(scan_text("", "", false), Err("SCAN_QUERY_EMPTY".to_owned()));
        assert_eq!(scan_text("", &"a".repeat(MAX_SCAN_QUERY_BYTES + 1), false), Err("SCAN_QUERY_TOO_LARGE".to_owned()));
        assert_eq!(scan_text(&"a".repeat(MAX_SCAN_INPUT_BYTES + 1), "a", false), Err("SCAN_INPUT_TOO_LARGE".to_owned()));
    }
}
