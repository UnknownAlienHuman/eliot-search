//! Same-handle observations for the existing Windows NTFS identity encoding.
//!
//! No path is opened, no file contents are read and no state is persisted here. Native
//! handles/structs never leave the ABI module. An observation alone is not
//! a source-admission, containment, owner-authority or currentness receipt.

use core::fmt;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

/// Closed, content-free native observation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationError {
    /// Windows native observation is unavailable on this target.
    UnsupportedPlatform,
    /// The handle is not a disk file/directory or its type could not be read.
    UnsupportedHandle,
    /// This legacy 64-bit identity representation is only supported on NTFS.
    UnsupportedFileSystem,
    /// The operating system did not return all requested information.
    ObservationFailed,
    /// The exact opened object is a reparse object.
    ReparsePointDenied,
    /// The borrowed regular file was not a direct member of the directory.
    NotDirectChild,
    /// A directory-information record had invalid framing or conflicting identity.
    InvalidDirectoryInformation,
    /// The caller's bounded directory-entry allowance was invalid or exhausted.
    DirectoryEntryLimit,
}

impl ObservationError {
    /// Stable diagnostic without paths, credentials or raw OS error text.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "NATIVE_FILE_UNSUPPORTED_PLATFORM",
            Self::UnsupportedHandle => "NATIVE_FILE_UNSUPPORTED_HANDLE",
            Self::UnsupportedFileSystem => "NATIVE_FILE_UNSUPPORTED_FILESYSTEM",
            Self::ObservationFailed => "NATIVE_FILE_OBSERVATION_FAILED",
            Self::ReparsePointDenied => "NATIVE_FILE_REPARSE_POINT_DENIED",
            Self::NotDirectChild => "NATIVE_FILE_NOT_DIRECT_CHILD",
            Self::InvalidDirectoryInformation => "NATIVE_FILE_DIRECTORY_INFORMATION_INVALID",
            Self::DirectoryEntryLimit => "NATIVE_FILE_DIRECTORY_ENTRY_LIMIT",
        }
    }
}

impl fmt::Display for ObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ObservationError {}

/// Information measured on one already-open object, excluding access time.
///
/// Access time can change because of the read being verified. Identity alone
/// is not a timeless source identity: deleted file IDs can eventually be reused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    /// Observed volume serial; never a replacement for an unavailable value.
    pub volume_serial: u32,
    /// Observed NTFS file index, with both 32-bit words preserved.
    pub file_index: u64,
    /// Exact native attributes at this observation.
    pub attributes: u32,
    /// File byte length (not meaningful for a directory).
    pub length: u64,
    /// Native creation time in Windows FILETIME units.
    pub creation_time: u64,
    /// Native last-write time in Windows FILETIME units.
    pub last_write_time: u64,
}

/// Closed native location classification measured from one qualified physical handle.
///
/// This is a fact for the `SourceV2` producer to compose, not a root grant or an
/// admission decision. `Unknown` is deliberately representable so an optional
/// native fact can never be replaced with `LocalFixed`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NativeLocationClass {
    /// A fixed local disk volume.
    LocalFixed,
    /// A local volume with removable media or a portable device characteristic.
    LocalRemovable,
    /// A remote filesystem share, such as SMB/CIFS.
    NetworkShare,
    /// A remote filesystem that is not classified as a network share.
    Remote,
    /// A known unsupported device or volume type.
    Unsupported,
    /// The native facts were valid but did not establish one supported class.
    Unknown,
}

/// Which documented final-name namespace the native name helper should return.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalizedNameKind {
    /// `VOLUME_NAME_GUID | FILE_NAME_NORMALIZED`.
    VolumeGuid,
    /// `VOLUME_NAME_DOS | FILE_NAME_NORMALIZED`.
    Dos,
}

/// Maximum UTF-16 units accepted from `GetFinalPathNameByHandleW`.
///
/// A required size at or above this bound is refused; the helper never grows
/// the buffer in response to an OS-reported path length.
pub const NORMALIZED_NAME_MAX_UTF16: usize = 32 * 1024;

/// A private, synchronous, read-only physical source handle.
///
/// The only production constructor opens a regular NTFS object with
/// `FILE_SHARE_READ`, no overlapped flag and final-object no-follow semantics,
/// then qualifies the same handle. This type is physical I/O custody only: it
/// does not establish a semantic root, containment, source grant or authority.
pub struct SyncReadFile {
    file: File,
}

impl SyncReadFile {
    /// Opens and qualifies one regular physical source leaf.
    ///
    /// The caller supplies the semantic-root selection and the original
    /// callback/meter. This helper does not authorize the path or read its
    /// contents.
    /// # Errors
    /// Returns a closed failure for open, callback, disk/NTFS, directory or
    /// reparse qualification failure.
    pub fn open_no_follow(
        path: &Path,
        check: &dyn Fn(bool) -> Result<(), ObservationError>,
    ) -> Result<Self, ObservationError> {
        platform::open_no_follow(path, check)
    }

    /// Borrows the exact qualified handle for the caller's metered body read.
    #[must_use]
    pub const fn file(&self) -> &File { &self.file }
}

/// Measures location facts from the exact qualified synchronous source handle.
///
/// No arbitrary `File`, path or mount-point string is accepted. The query is
/// content-free and does not grant read authority.
/// # Errors
/// Native status, output-length or callback refusal returns no classification.
pub fn location_with_check(
    file: &SyncReadFile,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<NativeLocationClass, ObservationError> {
    platform::location_with_check(file, check)
}

/// Returns one bounded normalized name from an already-held file or directory.
///
/// The returned UTF-16 is converted strictly; malformed UTF-16, embedded NUL,
/// an over-bound OS length or a native failure is refused. The helper never
/// opens or resolves the caller's locator.
/// # Errors
/// Native failure, callback refusal, malformed UTF-16 or a path beyond the
/// fixed bound returns no name.
pub fn normalized_name_with_check(
    file: &File,
    kind: NormalizedNameKind,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<String, ObservationError> {
    platform::normalized_name_with_check(file, kind, check)
}

/// Reads the documented per-directory case-sensitive flag from a held handle.
/// # Errors
/// Non-directory, unsupported-filesystem, native or callback failure returns no
/// case-sensitivity fact.
pub fn case_sensitive_directory_with_check(
    file: &File,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<bool, ObservationError> {
    platform::case_sensitive_directory_with_check(file, check)
}

impl Observation {
    /// Existing persisted NTFS identity layout: volume u32 BE, file index u64 BE.
    #[must_use]
    pub fn legacy_identity_bytes(self) -> [u8; 12] {
        let mut bytes = [0_u8; 12];
        bytes[..4].copy_from_slice(&self.volume_serial.to_be_bytes());
        bytes[4..].copy_from_slice(&self.file_index.to_be_bytes());
        bytes
    }
}

/// Fresh identity of a retained, qualified NTFS directory; no currentness or access grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectoryIdentity {
    /// Current volume serial from the same native snapshot.
    pub volume_serial: u32,
    /// Current complete NTFS file index.
    pub file_index: u64,
    /// Current creation time, never substituted from qualification metadata.
    pub creation_time: u64,
    /// Current attributes, including directory and reparse eligibility.
    pub attributes: u32,
}

/// Retains the exact disk/NTFS directory FD qualified by this native owner.
/// Private fields prevent pairing qualification with a replacement file.
pub struct QualifiedDirectory {
    file: Arc<File>,
    volume_serial: u32,
}

impl QualifiedDirectory {
    /// Runs the original three-ABI disk/NTFS/no-reparse observation on this FD.
    /// Returns the retained owner and the actual initial full snapshot, or None
    /// when the observed disk object is not a directory. No locator is opened.
    /// Every ABI is admitted by check(true), then polled by check(false).
    /// # Errors
    /// Native eligibility failure or caller refusal produces no qualified owner.
    pub fn qualify_with_check(
        file: Arc<File>, check: &dyn Fn(bool) -> Result<(), ObservationError>,
    ) -> Result<Option<(Self, Observation)>, ObservationError> {
        let observed = observe_with_check(&file, check)?;
        if observed.attributes & 0x10 == 0 { return Ok(None); }
        Ok(Some((Self { file, volume_serial: observed.volume_serial }, observed)))
    }

    /// Borrows the original FD without admitting replacement or mutable access.
    #[must_use]
    pub fn file(&self) -> &File { &self.file }

    /// Retains the same FD without duplicating an OS handle or its access check.
    #[must_use]
    pub fn retain_file(&self) -> Arc<File> { Arc::clone(&self.file) }

    /// Takes one fresh `GetFileInformationByHandle` snapshot of the retained FD.
    /// Preserves live attributes, volume, index and creation time; no file size,
    /// write time, directory listing, locator or permission currentness is issued.
    /// # Errors
    /// Refuses ABI/context failure, reparse, non-directory or volume-serial drift.
    pub fn identity_with_check(
        &self, check: &dyn Fn(bool) -> Result<(), ObservationError>,
    ) -> Result<DirectoryIdentity, ObservationError> {
        platform::qualified_directory_identity_with_check(&self.file, self.volume_serial, check)
    }
}

/// Observes a borrowed final handle using stable Win32 APIs.
///
/// Does not reopen a locator, take ownership of the handle or fabricate a
/// missing identity. The old 64-bit encoding is retained only on NTFS: `ReFS`
/// requires a separately versioned 128-bit identity and migration, not truncation.
///
/// # Errors
/// Returns a closed failure for unsupported platform/handle/filesystem, reparse
/// objects or unsuccessful native observation. There is no path/zero fallback.
pub fn observe(file: &File) -> Result<Observation, ObservationError> {
    observe_with_check(file, &|_| Ok(()))
}

/// Reuses the same observation engine with a checkpoint around each native call.
///
/// The hook receives true before dispatch and false after it; only the former
/// admits native work. An unsupported platform only polls with false.
/// The caller owns the original deadline and work admission; this hook creates neither.
/// # Errors
/// A refused checkpoint or unsupported/failed observation returns no identity.
pub fn observe_with_check(file: &File, check: &dyn Fn(bool) -> Result<(), ObservationError>)
    -> Result<Observation, ObservationError> {
    platform::observe_with_check(file, check)
}

/// Returns full observation and live links from one native file-information snapshot.
///
/// The original three disk/NTFS/file-information ABIs each retain their pre/post
/// callback. Repeat this probe at each before/after boundary; no cached path
/// access, source permission or currentness is implied.
/// # Errors
/// Native failure or callback refusal returns no tuple.
pub fn observe_with_links_with_check(file: &File,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<(Observation, u32), ObservationError> {
    platform::observe_with_links_with_check(file, check)
}

/// Returns the exact hardlink count of one already-open object.
///
/// Used by the final-handle adapter to fail closed on multi-link objects
/// whose bytes are shared with a foreign domain. Reports no path.
///
/// # Errors
/// Returns a closed failure when the count is unavailable on this target.
pub fn hardlink_count(file: &File) -> Result<u32, ObservationError> {
    platform::hardlink_count(file)
}

/// Fixed caller-admitted directory query capacity; no helper allocation occurs.
pub const CHILD_MEMBERSHIP_BUFFER_BYTES: usize = 64 * 1024;
/// Existing 2,048-name ceiling plus the two native dot entries, which also count.
pub const CHILD_MEMBERSHIP_MAX_ENTRIES: usize = 2048 + 2;

/// Caller-owned, eight-byte-aligned native query scratch, never a body cache.
///
/// Admit its published capacity and initialization work before construction. Reuse requires an exclusive
/// mutable borrow; the helper initializes it before every native query.
#[repr(C, align(8))]
pub struct ChildMembershipScratch {
    bytes: [u8; CHILD_MEMBERSHIP_BUFFER_BYTES],
}

impl ChildMembershipScratch {
    /// Constructs exactly the published fixed capacity without a heap allocation.
    #[must_use]
    #[expect(clippy::large_stack_arrays, reason = "Published fixed ABI scratch; caller admits capacity and initialization before construction")]
    pub const fn new() -> Self {
        Self { bytes: [0; CHILD_MEMBERSHIP_BUFFER_BYTES] }
    }
}

impl Default for ChildMembershipScratch {
    fn default() -> Self { Self::new() }
}

const _: () = assert!(core::mem::size_of::<ChildMembershipScratch>() == CHILD_MEMBERSHIP_BUFFER_BYTES);
const _: () = assert!(core::mem::align_of::<ChildMembershipScratch>() == 8);

/// Proves direct NTFS membership of an already-open regular file in an
/// already-open non-reparse directory, returning the child's actual observation.
///
/// Both full observations must remain unchanged through bounded enumeration.
/// This proves identity/containment only: the caller must freshly open the leaf
/// by its full path and enforce current READ/effective traverse, live grants,
/// hardlink policy and the final post-body locator guard independently.
///
/// Uses only the same parent handle (restart class 11, then continuation class
/// 10). The caller serializes use of that handle's directory enumeration cursor.
/// No paths/names are accepted, opened, created or resolved by this helper.
/// `max_entries` includes dot entries and cannot exceed the published ceiling.
///
/// `check(true)` admits each ABI dispatch and `check(false)` polls afterward.
/// Neither is called for CPU-only work. Query/error capture is one inseparable pair: both
/// dispatches are admitted before the query, with both polls after `GetLastError`,
/// so caller callbacks cannot overwrite its result. `process(0)` polls original
/// request/poison/deadline without native effects. `process(bytes)` admits each
/// scratch initialization and each record's header/name bytes before parsing;
/// unused buffer capacity is not charged as parsed data. Both hooks borrow the
/// caller's original allowance/deadline and create no new meter.
/// # Errors
/// Refusal, unknown native outcomes, invalid framing, non-NTFS/reparse objects,
/// drift, missing membership or a bounded-entry overflow return no proof.
pub fn verify_direct_file_child_with_check(
    parent: &File, child: &File, scratch: &mut ChildMembershipScratch, max_entries: usize,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
    process: &dyn Fn(usize) -> Result<(), ObservationError>,
) -> Result<Observation, ObservationError> {
    verify_direct_child_with_check(parent, child, scratch, max_entries, false, check, process)
}

/// Proves one direct directory link in a retained NTFS parent chain.
///
/// Uses the same capacity, admission hooks and containment-only contract as
/// [`verify_direct_file_child_with_check`]. The child must be a non-reparse
/// directory, not the parent itself. Directory-entry EOF/mtime do not establish
/// identity; its ID, creation time and exact attributes must match the actual FD.
/// The returned full observation comes from that FD, never the directory record.
/// # Errors
/// Any invalid kind, missing link, drift, native/parser failure or caller refusal
/// returns no membership. Current full-path access remains the caller's duty.
pub fn verify_direct_directory_child_with_check(
    parent: &File, child: &File, scratch: &mut ChildMembershipScratch, max_entries: usize,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
    process: &dyn Fn(usize) -> Result<(), ObservationError>,
) -> Result<Observation, ObservationError> {
    verify_direct_child_with_check(parent, child, scratch, max_entries, true, check, process)
}

fn verify_direct_child_with_check(
    parent: &File, child: &File, scratch: &mut ChildMembershipScratch, max_entries: usize,
    directory: bool, check: &dyn Fn(bool) -> Result<(), ObservationError>,
    process: &dyn Fn(usize) -> Result<(), ObservationError>,
) -> Result<Observation, ObservationError> {
    process(0)?;
    if max_entries == 0 || max_entries > CHILD_MEMBERSHIP_MAX_ENTRIES {
        return Err(ObservationError::DirectoryEntryLimit);
    }
    platform::verify_direct_child_with_check(parent, child, scratch, max_entries, directory, check, process)
}

// FILE_ID_BOTH_DIR_INFO in winbase.h. Parse checked little-endian byte ranges,
// never casts/references to a variable-length C struct. FileId is at 96 and the
// UTF-16 FileName starts at 104 on the documented x86/x64 eight-byte layout.
// https://learn.microsoft.com/windows/win32/api/winbase/ns-winbase-file_id_both_dir_info
#[cfg(any(windows, test))]
fn directory_batch_matches(
    bytes: &[u8], child: Observation, entries: &mut usize, max_entries: usize,
    process: &dyn Fn(usize) -> Result<(), ObservationError>,
) -> Result<bool, ObservationError> {
    let invalid = || ObservationError::InvalidDirectoryInformation;
    if !bytes.as_ptr().addr().is_multiple_of(8) { return Err(invalid()); }
    let mut offset = 0_usize;
    let mut found = false;
    loop {
        process(0)?;
        *entries = entries.checked_add(1).ok_or(ObservationError::DirectoryEntryLimit)?;
        if *entries > max_entries { return Err(ObservationError::DirectoryEntryLimit); }
        process(104)?;
        let record = bytes.get(offset..).ok_or_else(invalid)?;
        record.get(..104).ok_or_else(invalid)?;
        let u32_at = |at: usize| -> Result<u32, ObservationError> {
            Ok(u32::from_le_bytes(record.get(at..at + 4).ok_or_else(invalid)?
                .try_into().map_err(|_| invalid())?))
        };
        let u64_at = |at: usize| -> Result<u64, ObservationError> {
            Ok(u64::from_le_bytes(record.get(at..at + 8).ok_or_else(invalid)?
                .try_into().map_err(|_| invalid())?))
        };
        let next = usize::try_from(u32_at(0)?).map_err(|_| invalid())?;
        let name_bytes = usize::try_from(u32_at(60)?).map_err(|_| invalid())?;
        let short_bytes = *record.get(68).ok_or_else(invalid)?;
        if name_bytes == 0 || !name_bytes.is_multiple_of(2)
            || short_bytes > 24 || !short_bytes.is_multiple_of(2)
        { return Err(invalid()); }
        let end = 104_usize.checked_add(name_bytes).ok_or_else(invalid)?;
        let name = record.get(104..end).ok_or_else(invalid)?;
        if next != 0 && (!next.is_multiple_of(8) || next < end
            || offset.checked_add(next).is_none_or(|next| next >= bytes.len()))
        { return Err(invalid()); }
        process(name_bytes)?;
        if name.as_chunks::<2>().0.iter().any(|unit|
            matches!(u16::from_le_bytes([unit[0], unit[1]]), 0 | 47 | 92 | 58))
        { return Err(invalid()); }
        let dot = name == [46, 0] || name == [46, 0, 46, 0];
        let id = u64_at(96)?;
        if !dot && id == child.file_index {
            // Directory-entry sizes/times need not have file-data semantics.
            // Full before/after observations are separately made on actual FDs.
            let directory = child.attributes & 0x10 != 0;
            if u64_at(8)? != child.creation_time || u32_at(56)? != child.attributes
                || (!directory && (u64_at(24)? != child.last_write_time || u64_at(40)? != child.length))
            { return Err(invalid()); }
            found = true;
        }
        if next == 0 { return Ok(found); }
        offset = offset.checked_add(next).ok_or_else(invalid)?;
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{
        ChildMembershipScratch, DirectoryIdentity, File, NativeLocationClass, NormalizedNameKind,
        Observation, ObservationError, SyncReadFile,
    };
    use std::path::Path;

    pub(super) fn open_no_follow(
        _path: &Path,
        check: &dyn Fn(bool) -> Result<(), ObservationError>,
    ) -> Result<SyncReadFile, ObservationError> {
        check(false)?;
        Err(ObservationError::UnsupportedPlatform)
    }

    pub(super) fn location_with_check(
        _file: &SyncReadFile,
        check: &dyn Fn(bool) -> Result<(), ObservationError>,
    ) -> Result<NativeLocationClass, ObservationError> {
        check(false)?;
        Err(ObservationError::UnsupportedPlatform)
    }

    pub(super) fn normalized_name_with_check(
        _file: &File,
        _kind: NormalizedNameKind,
        check: &dyn Fn(bool) -> Result<(), ObservationError>,
    ) -> Result<String, ObservationError> {
        check(false)?;
        Err(ObservationError::UnsupportedPlatform)
    }

    pub(super) fn case_sensitive_directory_with_check(
        _file: &File,
        check: &dyn Fn(bool) -> Result<(), ObservationError>,
    ) -> Result<bool, ObservationError> {
        check(false)?;
        Err(ObservationError::UnsupportedPlatform)
    }

    pub(super) fn qualified_directory_identity_with_check(
        _file: &File, _volume_serial: u32,
        check: &dyn Fn(bool) -> Result<(), ObservationError>,
    ) -> Result<DirectoryIdentity, ObservationError> {
        check(false)?;
        Err(ObservationError::UnsupportedPlatform)
    }

    pub(super) fn observe_with_check(_file: &File, check: &dyn Fn(bool) -> Result<(), ObservationError>) -> Result<Observation, ObservationError> {
        check(false)?;
        Err(ObservationError::UnsupportedPlatform)
    }

    pub(super) fn observe_with_links_with_check(_file: &File,
        check: &dyn Fn(bool) -> Result<(), ObservationError>,
    ) -> Result<(Observation, u32), ObservationError> {
        check(false)?;
        Err(ObservationError::UnsupportedPlatform)
    }

    pub(super) fn hardlink_count(_file: &File) -> Result<u32, ObservationError> {
        Err(ObservationError::UnsupportedPlatform)
    }

    pub(super) fn verify_direct_child_with_check(
        _parent: &File, _child: &File, _scratch: &mut ChildMembershipScratch, _max_entries: usize,
        _directory: bool,
        _check: &dyn Fn(bool) -> Result<(), ObservationError>,
        process: &dyn Fn(usize) -> Result<(), ObservationError>,
    ) -> Result<Observation, ObservationError> {
        process(0)?;
        Err(ObservationError::UnsupportedPlatform)
    }
}

// The exception covers only the checked ABI calls, not callers or serializers.
#[cfg(windows)]
#[allow(unsafe_code)]
#[path = "native_file/windows.rs"]
mod platform;

#[cfg(test)]
#[path = "native_file/tests.rs"]
mod tests;

#[cfg(all(test, windows))]
#[path = "native_file/observation_links_tests.rs"]
mod observation_links_tests;

#[cfg(test)]
#[path = "native_file/child_membership_tests.rs"]
mod child_membership_tests;
