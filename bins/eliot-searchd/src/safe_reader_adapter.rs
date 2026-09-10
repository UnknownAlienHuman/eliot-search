//! Final-handle platform adapter proving containment before shared-kernel reads.
//!
//! The adapter opens one final object under an admitted root, proves
//! final-object and ancestor containment on the opened handle (never on path
//! text alone), enforces the qualified no-follow/reparse/device/ACL profile
//! and then serves the shared [`search_safe_reader`] kernel from that same
//! handle. Source bytes are inert copies; this module never spawns a process,
//! loads a hook/filter driver, prompts for credentials, touches a network or
//! evaluates content as code.
//!
//! Errors and diagnostics are content-free: adapters report closed codes
//! without paths, bytes, credentials or raw OS error text. Relative tokens
//! stay redacted through the kernel [`core::fmt::Debug`] impl.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};
use search_safe_reader::{
    AdapterRead, FinalHandleMetadata, FinalHandleOpenRequest, ReadSecurityDisposition,
    SafeReadBackend, SafeReadError,
};

/// Statically pinned no-execute invariant for this adapter.
pub const ADAPTER_NO_EXECUTE: bool = true;

const _: () = assert!(ADAPTER_NO_EXECUTE);

/// Finite retry budget applied by primary ingestion translation.
pub const ADAPTER_MAX_ATTEMPTS: u8 = 3;

/// Single-read chunk ceiling handed to the kernel (8 MiB, kernel default).
pub const ADAPTER_SINGLE_READ_BYTES: usize = 8 * 1024 * 1024;

/// Qualified platform identity profile proven by this adapter.
#[must_use]
pub const fn qualified_profile() -> &'static str {
    #[cfg(windows)]
    {
        "windows-final-handle/v1"
    }
    #[cfg(all(unix, not(windows)))]
    {
        "unix-final-handle/v1"
    }
    #[cfg(not(any(unix, windows)))]
    {
        "portable-final-handle/v1"
    }
}

/// Closed, content-free adapter failure. No variant carries paths, bytes or
/// raw OS error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterError {
    /// Relative token is absolute, escapes, names a device/stream or is unbounded.
    PathDenied,
    /// Final precheck or opened handle is a symlink/reparse object.
    LinkDenied,
    /// An authoritative ancestor traversal crossed a reparse boundary.
    AncestorReparseDenied,
    /// Final object canonicalizes outside the admitted root.
    EscapeDenied,
    /// Admitted root canonical identity moved between construction and open.
    RootRelocated,
    /// Final object is not a regular file (directory at precheck).
    NotRegular,
    /// Opened handle is not a regular file or is a device/pipe object.
    FinalObjectInvalid,
    /// Multi-link object; hardlink-outside-domain fails closed.
    HardlinkDenied,
    /// FIFO, socket, block/char device or another special object.
    DeviceDenied,
    /// OS open/read/metadata failed (ACL, revocation, races); no detail kept.
    AccessDenied,
    /// Source exceeds the caller-supplied finite byte budget.
    TooLarge,
    /// Content-free receipt/identity text could not be constructed.
    ReceiptDenied,
}

impl AdapterError {
    /// Stable machine-readable reason code without content.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PathDenied => "SAFE_ADAPTER_PATH_DENIED",
            Self::LinkDenied => "SAFE_ADAPTER_LINK_DENIED",
            Self::AncestorReparseDenied => "SAFE_ADAPTER_ANCESTOR_REPARSE_DENIED",
            Self::EscapeDenied => "SAFE_ADAPTER_ESCAPE_DENIED",
            Self::RootRelocated => "SAFE_ADAPTER_ROOT_RELOCATED",
            Self::NotRegular => "SAFE_ADAPTER_NOT_REGULAR",
            Self::FinalObjectInvalid => "SAFE_ADAPTER_FINAL_OBJECT_INVALID",
            Self::HardlinkDenied => "SAFE_ADAPTER_HARDLINK_DENIED",
            Self::DeviceDenied => "SAFE_ADAPTER_DEVICE_DENIED",
            Self::AccessDenied => "SAFE_ADAPTER_ACCESS_DENIED",
            Self::TooLarge => "SAFE_ADAPTER_TOO_LARGE",
            Self::ReceiptDenied => "SAFE_ADAPTER_RECEIPT_DENIED",
        }
    }
}

impl core::fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for AdapterError {}

/// Validates an opaque relative token without following any link.
///
/// Accepts `/`-separated relative components only: no absolute paths, drive
/// prefixes, UNC/device prefixes, `.`/`..`, empty segments, backslashes,
/// colons (alternate streams), NUL bytes or Windows reserved device names.
/// The byte length must fit the kernel token ceiling.
pub fn validate_relative_token(value: &str, max_token_bytes: usize) -> Result<(), AdapterError> {
    if value.is_empty() || value.len() > max_token_bytes || value.contains('\0') {
        return Err(AdapterError::PathDenied);
    }
    if value.starts_with('/') || value.starts_with('\\') {
        return Err(AdapterError::PathDenied);
    }
    if value.contains('\\') || value.contains(':') {
        return Err(AdapterError::PathDenied);
    }
    // Drive (`C:/..`), UNC (`//host/..`) and NT device (`\\?\..`, `\\.\..`)
    // prefixes must never reach the join, even when spelled with slashes.
    let upper = value.to_ascii_uppercase();
    if upper.starts_with("//") || upper.starts_with("\\\\") {
        return Err(AdapterError::PathDenied);
    }
    if value.len() >= 2 && value.as_bytes()[1] == b':' && value.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err(AdapterError::PathDenied);
    }
    for component in value.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(AdapterError::PathDenied);
        }
        if is_reserved_device_name(component) {
            return Err(AdapterError::PathDenied);
        }
    }
    Ok(())
}

fn is_reserved_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// Canonicalizes an admitted root directory and proves its own identity.
///
/// The root itself must be a non-link, non-reparse directory; otherwise the
/// containment base is ambiguous and every dependent open fails closed.
pub fn canonicalize_admitted_root(root: &Path) -> Result<PathBuf, AdapterError> {
    let precheck = fs::symlink_metadata(root).map_err(|_| AdapterError::AccessDenied)?;
    if precheck.file_type().is_symlink() || is_reparse(&precheck) {
        return Err(AdapterError::RootRelocated);
    }
    if !precheck.is_dir() {
        return Err(AdapterError::NotRegular);
    }
    let canonical = fs::canonicalize(root).map_err(|_| AdapterError::AccessDenied)?;
    let recheck = fs::symlink_metadata(&canonical).map_err(|_| AdapterError::AccessDenied)?;
    if recheck.file_type().is_symlink() || is_reparse(&recheck) || !recheck.is_dir() {
        return Err(AdapterError::RootRelocated);
    }
    Ok(canonical)
}

/// Computes the stable logical-root identity digest for one canonical root.
pub fn root_identity_digest(canonical_root: &Path) -> Result<Blake3Digest32, AdapterError> {
    let material = root_identity_material(canonical_root)?;
    Ok(blake3_digest(
        b"eliot-search/safe-adapter-root/v1",
        &material,
    ))
}

/// Computes the stable file identity digest for already-gathered material.
#[must_use]
pub fn file_identity_digest(identity_material: &[u8]) -> Blake3Digest32 {
    blake3_digest(b"eliot-search/safe-adapter-file/v1", identity_material)
}

fn blake3_digest(domain: &[u8], material: &[u8]) -> Blake3Digest32 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    // Bind every adapter digest to the qualified platform profile so a
    // digest minted under one profile never verifies under another.
    hasher.update(qualified_profile().as_bytes());
    hasher.update(&material.len().to_be_bytes());
    hasher.update(material);
    Blake3Digest32::from_bytes(*hasher.finalize().as_bytes())
}

/// Computes the stable logical-root identity digest for one canonical root.
/// Unix uses device/inode (fallible); Windows uses the canonical verbatim
/// path because directories cannot be opened without backup semantics.
#[allow(clippy::unnecessary_wraps)]
fn root_identity_material(canonical_root: &Path) -> Result<Vec<u8>, AdapterError> {
    #[cfg(all(unix, not(windows)))]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(canonical_root).map_err(|_| AdapterError::AccessDenied)?;
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&metadata.dev().to_be_bytes());
        bytes.extend_from_slice(&metadata.ino().to_be_bytes());
        Ok(bytes)
    }
    // Windows cannot `File::open` a directory without backup semantics, so
    // the root binding is the canonical verbatim path itself. Relocation is
    // still detected: every open re-canonicalizes and compares the exact
    // string, and the final-object walk re-observes each ancestor. File
    // identity below remains a native NTFS identity.
    #[cfg(windows)]
    {
        Ok(path_identity_bytes(canonical_root))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(path_identity_bytes(canonical_root))
    }
}

/// Stable identity material for one already-open handle.
///
/// Matches the historical DIRECT identity encoding (device/inode on Unix,
/// NTFS volume/file-index on Windows) so retained source identities keep
/// their existing values across the kernel translation. Returns the material,
/// whether the object is a multi-link hardlink, and whether the material is
/// a native stable identity (false means a path-bound fallback). Unix
/// metadata reads are fallible; the other profiles always succeed, hence the
/// uniform `Result`.
#[allow(clippy::unnecessary_wraps)]
fn handle_identity_material(
    file: &File,
    canonical_final: &Path,
) -> Result<(Vec<u8>, bool, bool), AdapterError> {
    #[cfg(all(unix, not(windows)))]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata().map_err(|_| AdapterError::AccessDenied)?;
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&metadata.dev().to_be_bytes());
        bytes.extend_from_slice(&metadata.ino().to_be_bytes());
        Ok((bytes, metadata.nlink() != 1, true))
    }
    #[cfg(windows)]
    {
        let _ = canonical_final;
        Ok(eliot_searchd::native_file::observe(file).map_or_else(
            |_| (path_identity_bytes(canonical_final), false, false),
            |observed| {
                // The observer already proved this handle; an unavailable
                // link count fails closed instead of assuming single-link.
                let links = eliot_searchd::native_file::hardlink_count(file).unwrap_or(u32::MAX);
                (observed.legacy_identity_bytes().to_vec(), links != 1, true)
            },
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok((path_identity_bytes(canonical_final), false, false))
    }
}

#[cfg(unix)]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(windows)]
fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &std::fs::Metadata) -> bool {
    false
}

/// Walks every authoritative ancestor of `canonical_final` up to and
/// including `canonical_root`, denying symlink/reparse boundaries.
///
/// Textual prefix alone is never containment evidence: each ancestor is
/// re-observed with `symlink_metadata` on the already-canonicalized path.
fn verify_ancestor_containment(
    canonical_final: &Path,
    canonical_root: &Path,
) -> Result<(), AdapterError> {
    if !canonical_final.starts_with(canonical_root) {
        return Err(AdapterError::EscapeDenied);
    }
    let mut current: &Path = canonical_final;
    loop {
        let metadata = fs::symlink_metadata(current).map_err(|_| AdapterError::AccessDenied)?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            if current == canonical_final {
                return Err(AdapterError::LinkDenied);
            }
            return Err(AdapterError::AncestorReparseDenied);
        }
        if current == canonical_root {
            return Ok(());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return Err(AdapterError::EscapeDenied),
        }
    }
}

/// Final opened handle: an owned file plus its proven identities.
///
/// `Debug` is redacted to digests and lengths; it never prints paths or
/// raw identity material.
pub struct FinalHandle {
    file: File,
    root_digest: Blake3Digest32,
    file_digest: Blake3Digest32,
    identity_material: Vec<u8>,
    identity_native: bool,
    canonical_final: PathBuf,
    source_bytes: u64,
}

impl core::fmt::Debug for FinalHandle {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FinalHandle")
            .field("root_digest", &self.root_digest)
            .field("file_digest", &self.file_digest)
            .field("source_bytes", &self.source_bytes)
            .finish_non_exhaustive()
    }
}

/// Platform backend serving the shared kernel from one final handle.
pub struct FinalHandleBackend {
    canonical_root: PathBuf,
    root_digest: Blake3Digest32,
    token_text: String,
    barrier: NonZeroRevision,
    max_single_read_bytes: usize,
}

impl core::fmt::Debug for FinalHandleBackend {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FinalHandleBackend")
            .field("root_digest", &self.root_digest)
            .field("token_bytes", &self.token_text.len())
            .field("barrier", &self.barrier.get())
            .finish_non_exhaustive()
    }
}

impl FinalHandleBackend {
    /// Binds one admitted root and one validated relative token.
    ///
    /// No file is opened here; [`SafeReadBackend::open_final`] opens exactly
    /// once per attempt under this binding.
    pub fn bind(
        admitted_root: &Path,
        relative_token: &str,
        barrier: NonZeroRevision,
        max_single_read_bytes: usize,
    ) -> Result<Self, AdapterError> {
        if max_single_read_bytes == 0 {
            return Err(AdapterError::PathDenied);
        }
        validate_relative_token(relative_token, 32_768)?;
        let canonical_root = canonicalize_admitted_root(admitted_root)?;
        let root_digest = root_identity_digest(&canonical_root)?;
        Ok(Self {
            canonical_root,
            root_digest,
            token_text: relative_token.to_owned(),
            barrier,
            max_single_read_bytes,
        })
    }

    /// Stable logical-root digest proven at bind time.
    #[must_use]
    pub const fn root_digest(&self) -> Blake3Digest32 {
        self.root_digest
    }

    fn open_handle(&self) -> Result<FinalHandle, AdapterError> {
        // The token was validated component-wise at bind time, so pushing
        // one component at a time cannot escape the admitted root textually;
        // the ancestor walk below proves it on the opened object anyway.
        let mut joined = self.canonical_root.clone();
        for component in self.token_text.split('/') {
            joined.push(component);
        }
        // Path precheck closes the trivial swap window; the handle rebinding
        // below closes the remaining replacement window.
        let precheck = fs::symlink_metadata(&joined).map_err(|_| AdapterError::AccessDenied)?;
        if precheck.file_type().is_symlink() || is_reparse(&precheck) {
            return Err(AdapterError::LinkDenied);
        }
        if precheck.is_dir() {
            return Err(AdapterError::NotRegular);
        }
        if !precheck.is_file() {
            return Err(AdapterError::FinalObjectInvalid);
        }
        deny_special_precheck(&precheck)?;

        let file = File::open(&joined).map_err(|_| AdapterError::AccessDenied)?;
        let handle_metadata = file.metadata().map_err(|_| AdapterError::AccessDenied)?;
        if !handle_metadata.is_file() {
            return Err(AdapterError::FinalObjectInvalid);
        }
        if is_reparse(&handle_metadata) {
            return Err(AdapterError::LinkDenied);
        }
        deny_special_handle(&file, &handle_metadata)?;

        let canonical_final = fs::canonicalize(&joined).map_err(|_| AdapterError::AccessDenied)?;
        verify_ancestor_containment(&canonical_final, &self.canonical_root)?;

        let (identity_material, is_hardlink, identity_native) =
            handle_identity_material(&file, &canonical_final)?;
        if is_hardlink {
            return Err(AdapterError::HardlinkDenied);
        }
        verify_handle_rebinding(&file, &canonical_final, &identity_material)?;

        let source_bytes = handle_metadata.len();
        let file_digest = file_identity_digest(&identity_material);
        // Validate receipt-text construction now so a malformed change
        // encoding fails at open, not mid-read. Stability itself is proven
        // by the kernel before/after inspection pair.
        let _ = change_text(
            &file_digest,
            source_bytes,
            modified_nanos(&handle_metadata),
            attribute_bits(&handle_metadata),
        )?;
        Ok(FinalHandle {
            file,
            root_digest: self.root_digest,
            file_digest,
            identity_material,
            identity_native,
            canonical_final,
            source_bytes,
        })
    }
}

#[cfg(unix)]
fn deny_special_precheck(metadata: &std::fs::Metadata) -> Result<(), AdapterError> {
    use std::os::unix::fs::FileTypeExt;
    let file_type = metadata.file_type();
    if file_type.is_fifo()
        || file_type.is_socket()
        || file_type.is_block_device()
        || file_type.is_char_device()
    {
        return Err(AdapterError::DeviceDenied);
    }
    Ok(())
}

#[cfg(not(unix))]
fn deny_special_precheck(metadata: &std::fs::Metadata) -> Result<(), AdapterError> {
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(AdapterError::DeviceDenied);
    }
    Ok(())
}

#[cfg(unix)]
fn deny_special_handle(_file: &File, metadata: &std::fs::Metadata) -> Result<(), AdapterError> {
    deny_special_precheck(metadata)
}

#[cfg(not(unix))]
fn deny_special_handle(file: &File, metadata: &std::fs::Metadata) -> Result<(), AdapterError> {
    deny_special_precheck(metadata)?;
    #[cfg(windows)]
    {
        // Pipes, consoles and other non-disk objects never become sources.
        match eliot_searchd::native_file::observe(file) {
            Ok(_) => Ok(()),
            Err(error) => {
                if error.code() == "NATIVE_FILE_REPARSE_POINT_DENIED" {
                    return Err(AdapterError::LinkDenied);
                }
                Err(AdapterError::FinalObjectInvalid)
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Ok(())
    }
}

/// Rebinds the already-open handle to its canonical path identity.
///
/// A deterministic replacement between precheck and open (path swapped for a
/// different object) surfaces here as an identity mismatch instead of
/// silently reading the substituted object.
fn verify_handle_rebinding(
    file: &File,
    canonical_final: &Path,
    handle_material: &[u8],
) -> Result<(), AdapterError> {
    #[cfg(all(unix, not(windows)))]
    {
        use std::os::unix::fs::MetadataExt;
        let final_metadata =
            fs::metadata(canonical_final).map_err(|_| AdapterError::AccessDenied)?;
        let _ = file;
        let mut expected = Vec::with_capacity(16);
        expected.extend_from_slice(&final_metadata.dev().to_be_bytes());
        expected.extend_from_slice(&final_metadata.ino().to_be_bytes());
        if expected != handle_material {
            return Err(AdapterError::AccessDenied);
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        let second = File::open(canonical_final).map_err(|_| AdapterError::AccessDenied)?;
        let first =
            eliot_searchd::native_file::observe(file).map_err(|_| AdapterError::AccessDenied)?;
        let other =
            eliot_searchd::native_file::observe(&second).map_err(|_| AdapterError::AccessDenied)?;
        if first.volume_serial != other.volume_serial || first.file_index != other.file_index {
            return Err(AdapterError::AccessDenied);
        }
        let _ = handle_material;
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, canonical_final, handle_material);
        Ok(())
    }
}

fn modified_nanos(metadata: &std::fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos())
}

fn attribute_bits(metadata: &std::fs::Metadata) -> u64 {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        u64::from(metadata.file_attributes())
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        u64::from(metadata.mode())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        0
    }
}

fn change_text(
    file_digest: &Blake3Digest32,
    source_bytes: u64,
    modified_nanos: u128,
    attributes: u64,
) -> Result<String, AdapterError> {
    let text = format!("v1:{file_digest}:{source_bytes}:{modified_nanos}:{attributes}");
    if text.len() > 256 {
        return Err(AdapterError::ReceiptDenied);
    }
    Ok(text)
}

fn metadata_receipt(source_bytes: u64, change_text: &str) -> Result<ReceiptRef, AdapterError> {
    ReceiptRef::new(format!("safe-adapter-meta-v1:{source_bytes}:{change_text}"))
        .map_err(|_| AdapterError::ReceiptDenied)
}

fn read_receipt(offset: u64, length: usize) -> Result<ReceiptRef, AdapterError> {
    ReceiptRef::new(format!("safe-adapter-read-v1:{offset}:{length}"))
        .map_err(|_| AdapterError::ReceiptDenied)
}

fn change_token(change_text: &str) -> Result<OpaqueId, AdapterError> {
    OpaqueId::new(format!("safe-adapter-change-v1:{change_text}"))
        .map_err(|_| AdapterError::ReceiptDenied)
}

impl SafeReadBackend for FinalHandleBackend {
    type Handle = FinalHandle;
    type BackendError = AdapterError;

    fn open_final(
        &mut self,
        request: &FinalHandleOpenRequest,
    ) -> Result<Self::Handle, Self::BackendError> {
        if request.relative_path.as_str() != self.token_text {
            return Err(AdapterError::PathDenied);
        }
        // The kernel compares the reported live root digest against the
        // expected binding and reports ROOT_IDENTITY_MISMATCH precisely, so
        // never invent bytes here: only refuse a relocated containment base.
        // Re-prove the root base on every open: a relocated admitted root
        // must never silently authorize a stale binding.
        let live_root = canonicalize_admitted_root(&self.canonical_root)
            .map_err(|_| AdapterError::RootRelocated)?;
        if live_root != self.canonical_root {
            return Err(AdapterError::RootRelocated);
        }
        self.open_handle()
    }

    fn inspect(
        &mut self,
        handle: &Self::Handle,
    ) -> Result<FinalHandleMetadata, Self::BackendError> {
        let metadata = handle
            .file
            .metadata()
            .map_err(|_| AdapterError::AccessDenied)?;
        let kind = if metadata.is_file() {
            search_safe_reader::FinalHandleKind::RegularFile
        } else if metadata.is_dir() {
            search_safe_reader::FinalHandleKind::Directory
        } else {
            search_safe_reader::FinalHandleKind::Other
        };
        let final_is_reparse = is_reparse(&metadata);
        let (live_material, live_hardlink, _) =
            handle_identity_material(&handle.file, &handle.canonical_final)
                .map_err(|_| AdapterError::AccessDenied)?;
        if live_hardlink {
            return Err(AdapterError::HardlinkDenied);
        }
        let live_digest = file_identity_digest(&live_material);
        let source_bytes = metadata.len();
        let change = change_text(
            &live_digest,
            source_bytes,
            modified_nanos(&metadata),
            attribute_bits(&metadata),
        )?;
        Ok(FinalHandleMetadata {
            root_identity_digest: handle.root_digest,
            stable_file_identity_digest: live_digest,
            kind,
            source_bytes,
            final_object_is_reparse: final_is_reparse,
            ancestor_reparse_observed: false,
            security_disposition: ReadSecurityDisposition::Permitted,
            security_barrier_revision: self.barrier,
            change_token: change_token(&change)?,
            metadata_receipt: Some(metadata_receipt(source_bytes, &change)?),
        })
    }

    fn read_exact_at(
        &mut self,
        handle: &Self::Handle,
        offset: u64,
        length: usize,
    ) -> Result<AdapterRead, Self::BackendError> {
        if length == 0 || length > self.max_single_read_bytes {
            return Err(AdapterError::AccessDenied);
        }
        let mut file = handle
            .file
            .try_clone()
            .map_err(|_| AdapterError::AccessDenied)?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|_| AdapterError::AccessDenied)?;
        let mut bytes = Vec::new();
        // Pre-size exactly; the length was validated finite by the caller and
        // the kernel range check, so this allocation is bounded.
        bytes
            .try_reserve_exact(length)
            .map_err(|_| AdapterError::TooLarge)?;
        file.take(length as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| AdapterError::AccessDenied)?;
        Ok(AdapterRead {
            bytes,
            read_receipt: Some(read_receipt(offset, length)?),
        })
    }

    fn map_backend_error(error: &Self::BackendError) -> SafeReadError {
        match error {
            AdapterError::TooLarge => SafeReadError::SourceSizeInvalid,
            _ => SafeReadError::BackendFailure,
        }
    }
}

/// Locator derived for one kernel read: canonical base, canonical final
/// path and the `/`-separated relative token between them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedLocator {
    /// Canonical admitted root proven at derive time.
    pub canonical_root: PathBuf,
    /// Canonical final path (links resolved lexically for token purposes).
    pub canonical_final: PathBuf,
    /// Relative token handed to the kernel; redacted in kernel `Debug`.
    pub token: String,
}

/// Derives a kernel locator for `path` under `admitted_root_hint`.
///
/// Both sides are canonicalized so verbatim (`\\?\`) and symlinked spellings
/// compare on the same base. Lexical `..`, non-absolute input and a final
/// path outside the admitted root are denied before any open. A stable
/// symlink/reparse final object is denied by the original-locator precheck;
/// a replacement between precheck and open is still closed by the
/// open-handle ancestor walk and identity rebinding.
pub fn derive_locator(
    path: &Path,
    admitted_root_hint: &Path,
) -> Result<DerivedLocator, AdapterError> {
    if !path.is_absolute() || !admitted_root_hint.is_absolute() {
        return Err(AdapterError::PathDenied);
    }
    // Lexical parent escape never reaches canonicalization.
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(AdapterError::PathDenied);
        }
    }
    let precheck = fs::symlink_metadata(path).map_err(|_| AdapterError::AccessDenied)?;
    if precheck.file_type().is_symlink() || is_reparse(&precheck) {
        return Err(AdapterError::LinkDenied);
    }
    if precheck.is_dir() {
        return Err(AdapterError::NotRegular);
    }
    if !precheck.is_file() {
        return Err(AdapterError::FinalObjectInvalid);
    }
    let canonical_root = canonicalize_admitted_root(admitted_root_hint)?;
    let canonical_final = fs::canonicalize(path).map_err(|_| AdapterError::AccessDenied)?;
    let relative = canonical_final
        .strip_prefix(&canonical_root)
        .map_err(|_| AdapterError::EscapeDenied)?;
    if relative.as_os_str().is_empty() {
        return Err(AdapterError::PathDenied);
    }
    let mut components = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                let text = part.to_str().ok_or(AdapterError::PathDenied)?;
                if text.is_empty()
                    || text == "."
                    || text == ".."
                    || text.contains(['\\', ':', '\0'])
                    || is_reserved_device_name(text)
                {
                    return Err(AdapterError::PathDenied);
                }
                components.push(text.to_owned());
            }
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => {
                return Err(AdapterError::PathDenied);
            }
        }
    }
    if components.is_empty() {
        return Err(AdapterError::PathDenied);
    }
    let token = components.join("/");
    validate_relative_token(&token, 32_768)?;
    Ok(DerivedLocator {
        canonical_root,
        canonical_final,
        token,
    })
}

/// Exact full-file product of one kernel-verified read.
#[derive(Debug)]
pub struct FullFileRead {
    /// Inert source bytes; never executed.
    pub bytes: Vec<u8>,
    /// Stable identity material in the historical DIRECT encoding, used by
    /// the caller to derive its retained identity digests.
    pub identity_material: Vec<u8>,
    /// Whether the material is a native stable identity rather than a
    /// path-bound fallback.
    pub identity_native: bool,
    /// Canonical final path proven inside the admitted root.
    pub canonical_final: PathBuf,
    /// Exact source byte size observed before and after the read.
    pub source_bytes: u64,
}

/// Translation failure: precise adapter denial or race-phase kernel denial.
///
/// Both sides are content-free; neither carries paths, bytes or raw OS text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FullReadError {
    /// Precheck/probe denial with the exact sub-cause.
    Adapter(AdapterError),
    /// Kernel denial from the race-phase re-open/read/revalidation.
    Kernel(SafeReadError),
}

impl FullReadError {
    /// Stable machine-readable reason code without content.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Adapter(error) => error.code(),
            Self::Kernel(error) => error.code(),
        }
    }
}

impl core::fmt::Display for FullReadError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for FullReadError {}

fn open_request(
    backend: &FinalHandleBackend,
    token: &str,
) -> Result<search_safe_reader::FinalHandleOpenRequest, FullReadError> {
    let relative_path = search_safe_reader::RelativePathToken::new(
        token,
        search_safe_reader::DEFAULT_SAFE_READ_LIMITS,
    )
    .map_err(|_| FullReadError::Adapter(AdapterError::PathDenied))?;
    Ok(search_safe_reader::FinalHandleOpenRequest {
        relative_path,
        expected_root_identity_digest: backend.root_digest(),
    })
}

fn kernel_limits(max_bytes: usize) -> Result<search_safe_reader::SafeReadLimits, FullReadError> {
    let ceiling = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    search_safe_reader::SafeReadLimits {
        max_source_bytes: ceiling,
        max_single_read_bytes: max_bytes,
        max_path_token_bytes: 32_768,
    }
    .validate()
    .map_err(|_| FullReadError::Adapter(AdapterError::TooLarge))
}

fn barrier() -> NonZeroRevision {
    NonZeroRevision::new(1).expect("non-zero barrier revision")
}

const fn map_factory_error(error: AdapterError) -> SafeReadError {
    match error {
        AdapterError::TooLarge => SafeReadError::SourceSizeInvalid,
        _ => SafeReadError::BackendFailure,
    }
}

/// Reads one exact file through the shared kernel under an admitted root.
///
/// `admitted_root_hint` is the containing directory the caller discovered
/// the file in (immediate parent for single files, batch directory for
/// directory ingestion); it is re-canonicalized inside. `max_bytes` bounds
/// allocation before any byte is read. Empty sources short-circuit through
/// two agreeing probe inspections (no bytes exist to leak); every other
/// source crosses one kernel `safe_read_with_retries` full-range attempt
/// sequence with at most [`ADAPTER_MAX_ATTEMPTS`] opens.
///
/// Cooperative cancellation has no interactive source in batch CLI ingestion
/// (process termination is the cancel path); the kernel cancel points are
/// still honored and proven by kernel tests.
pub fn read_full_file_via_kernel(
    source_path: &Path,
    admitted_root_hint: &Path,
    max_bytes: usize,
) -> Result<FullFileRead, FullReadError> {
    // A zero budget still admits zero-byte sources (they consume nothing);
    // any positive size against it fails as TooLarge below.
    let locator =
        derive_locator(source_path, admitted_root_hint).map_err(FullReadError::Adapter)?;
    // Probe: learn the stable binding without reading bytes.
    let (root_digest, file_digest, source_bytes) = probe_binding(&locator)?;
    if source_bytes == 0 {
        // Zero-byte sources carry no bytes to misdeliver; still require two
        // agreeing same-handle-class inspections before admission.
        let second = probe_binding(&locator)?;
        if second != (root_digest, file_digest, source_bytes) {
            return Err(FullReadError::Kernel(
                SafeReadError::HandleChangedDuringRead,
            ));
        }
        let material = probe_material(&locator, file_digest)?;
        return Ok(FullFileRead {
            bytes: Vec::new(),
            identity_material: material.0,
            identity_native: material.1,
            canonical_final: locator.canonical_final,
            source_bytes: 0,
        });
    }
    if source_bytes > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
        return Err(FullReadError::Adapter(AdapterError::TooLarge));
    }
    let length = usize::try_from(source_bytes)
        .map_err(|_| FullReadError::Adapter(AdapterError::TooLarge))?;
    let token = search_safe_reader::RelativePathToken::new(
        &locator.token,
        search_safe_reader::DEFAULT_SAFE_READ_LIMITS,
    )
    .map_err(|_| FullReadError::Adapter(AdapterError::PathDenied))?;
    let request = search_safe_reader::SafeReadRequest {
        relative_path: token,
        expected_root_identity_digest: root_digest,
        expected_stable_file_identity_digest: file_digest,
        expected_security_barrier_revision: barrier(),
        range: search_safe_reader::ReadRange {
            offset: 0,
            length,
            require_eof: true,
        },
    };
    let limits = kernel_limits(max_bytes)?;
    let policy = search_safe_reader::SafeRetryPolicy::new(ADAPTER_MAX_ATTEMPTS)
        .map_err(|_| FullReadError::Adapter(AdapterError::PathDenied))?;
    let canonical_root = locator.canonical_root.clone();
    let token_text = locator.token.clone();
    let mut make_backend = || {
        FinalHandleBackend::bind(&canonical_root, &token_text, barrier(), max_bytes)
            .map_err(map_factory_error)
    };
    let mut never_cancel = || false;
    let result = search_safe_reader::safe_read_with_retries(
        &mut make_backend,
        &request,
        limits,
        policy,
        &mut never_cancel,
    )
    .map_err(FullReadError::Kernel)?;
    debug_assert_eq!(result.bytes().len(), length);
    let material = probe_material(&locator, file_digest)?;
    Ok(FullFileRead {
        bytes: result.bytes().to_vec(),
        identity_material: material.0,
        identity_native: material.1,
        canonical_final: locator.canonical_final,
        source_bytes,
    })
}

/// Opens once and inspects once to learn the stable binding; reads no bytes.
fn probe_binding(
    locator: &DerivedLocator,
) -> Result<(Blake3Digest32, Blake3Digest32, u64), FullReadError> {
    let mut backend = FinalHandleBackend::bind(
        &locator.canonical_root,
        &locator.token,
        barrier(),
        ADAPTER_SINGLE_READ_BYTES,
    )
    .map_err(FullReadError::Adapter)?;
    let handle = backend
        .open_final(&open_request(&backend, &locator.token)?)
        .map_err(FullReadError::Adapter)?;
    let metadata = backend.inspect(&handle).map_err(FullReadError::Adapter)?;
    if metadata.kind != search_safe_reader::FinalHandleKind::RegularFile {
        return Err(FullReadError::Adapter(AdapterError::FinalObjectInvalid));
    }
    if metadata.final_object_is_reparse || metadata.ancestor_reparse_observed {
        return Err(FullReadError::Adapter(AdapterError::LinkDenied));
    }
    Ok((
        metadata.root_identity_digest,
        metadata.stable_file_identity_digest,
        metadata.source_bytes,
    ))
}

/// Re-opens to fetch the historical-encoding identity material after a
/// successful kernel read. Identity is stable by kernel proof; material that
/// no longer matches the proven digest fails closed as a race-phase denial.
fn probe_material(
    locator: &DerivedLocator,
    expected_file_digest: Blake3Digest32,
) -> Result<(Vec<u8>, bool), FullReadError> {
    let mut backend = FinalHandleBackend::bind(
        &locator.canonical_root,
        &locator.token,
        barrier(),
        ADAPTER_SINGLE_READ_BYTES,
    )
    .map_err(FullReadError::Adapter)?;
    let handle = backend
        .open_final(&open_request(&backend, &locator.token)?)
        .map_err(FullReadError::Adapter)?;
    if file_identity_digest(&handle.identity_material) != expected_file_digest {
        return Err(FullReadError::Kernel(
            SafeReadError::HandleChangedDuringRead,
        ));
    }
    Ok((handle.identity_material.clone(), handle.identity_native))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn barrier() -> NonZeroRevision {
        NonZeroRevision::new(1).expect("barrier")
    }

    fn fixture_root(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-safe-adapter-{name}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn token_validation_denies_escape_and_streams() {
        assert!(validate_relative_token("a/b.txt", 32_768).is_ok());
        for bad in [
            "",
            "/absolute",
            "\\absolute",
            "C:/drive",
            "c:/drive",
            "//host/share",
            "../escape",
            "a/../b",
            "a/./b",
            "a//b",
            "a\\b",
            "a:b",
            "CON",
            "con.txt",
            "NUL",
            "com1",
            "LPT9.log",
            "a/\0b",
        ] {
            assert_eq!(
                validate_relative_token(bad, 32_768),
                Err(AdapterError::PathDenied),
                "token={bad:?}"
            );
        }
        assert_eq!(
            validate_relative_token(&"a".repeat(32_769), 32_768),
            Err(AdapterError::PathDenied)
        );
    }

    #[test]
    fn adapter_errors_and_handles_are_redacted() {
        let error = AdapterError::EscapeDenied;
        let debug = format!("{error:?}");
        assert!(debug.contains("EscapeDenied"));
        assert!(!debug.contains('/'));
        assert_eq!(error.to_string(), "SAFE_ADAPTER_ESCAPE_DENIED");
        assert!(qualified_profile().contains("final-handle"));
    }

    #[test]
    fn symlink_final_object_is_denied_before_open() {
        let root = fixture_root("link");
        let target = root.join("target.txt");
        std::fs::write(&target, b"secret").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, root.join("link.txt")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&target, root.join("link.txt")).unwrap();
        let mut backend = FinalHandleBackend::bind(&root, "link.txt", barrier(), 1024).unwrap();
        let request = FinalHandleOpenRequest {
            relative_path: search_safe_reader::RelativePathToken::new(
                "link.txt",
                search_safe_reader::DEFAULT_SAFE_READ_LIMITS,
            )
            .unwrap(),
            expected_root_identity_digest: backend.root_digest(),
        };
        assert_eq!(
            backend.open_final(&request).unwrap_err(),
            AdapterError::LinkDenied
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn locator_derivation_denies_escape_without_opening_bytes() {
        let root = fixture_root("escape");
        let child = root.join("child.txt");
        std::fs::write(&child, b"inside").unwrap();
        let outside_dir = fixture_root("outside");
        let outside = outside_dir.join("secret.txt");
        std::fs::write(&outside, b"outside").unwrap();
        let locator = derive_locator(&child, &root).unwrap();
        assert_eq!(locator.canonical_root, fs::canonicalize(&root).unwrap());
        assert_eq!(locator.token, "child.txt");
        assert_eq!(
            derive_locator(&outside, &root),
            Err(AdapterError::EscapeDenied)
        );
        assert_eq!(
            derive_locator(&PathBuf::from("relative.txt"), &root),
            Err(AdapterError::PathDenied)
        );
        assert_eq!(
            derive_locator(&root.join("..").join("sibling.txt"), &root),
            Err(AdapterError::PathDenied)
        );
        std::fs::remove_dir_all(&root).unwrap();
        std::fs::remove_dir_all(&outside_dir).unwrap();
    }

    fn symlink_file(target: &Path, link: &Path) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(target, link).unwrap();
    }

    fn symlink_dir(target: &Path, link: &Path) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(target, link).unwrap();
    }

    #[test]
    fn replacement_between_derive_and_open_is_denied() {
        let root = fixture_root("replace");
        let victim = root.join("victim.txt");
        std::fs::write(&victim, b"original bytes").unwrap();
        // Derive against the genuine file, then deterministically swap the
        // locator for a symlink to a foreign object before the open.
        let locator = derive_locator(&victim, &root).unwrap();
        std::fs::remove_file(&victim).unwrap();
        let outside_dir = fixture_root("replace-out");
        let outside = outside_dir.join("evil.txt");
        std::fs::write(&outside, b"substituted bytes").unwrap();
        symlink_file(&outside, &victim);
        let mut backend =
            FinalHandleBackend::bind(&locator.canonical_root, &locator.token, barrier(), 1024)
                .unwrap();
        let request = FinalHandleOpenRequest {
            relative_path: search_safe_reader::RelativePathToken::new(
                &locator.token,
                search_safe_reader::DEFAULT_SAFE_READ_LIMITS,
            )
            .unwrap(),
            expected_root_identity_digest: backend.root_digest(),
        };
        assert_eq!(
            backend.open_final(&request).unwrap_err(),
            AdapterError::LinkDenied
        );
        let _ = std::fs::remove_file(&victim);
        std::fs::remove_dir_all(&root).unwrap();
        std::fs::remove_dir_all(&outside_dir).unwrap();
    }

    #[test]
    fn ancestor_junction_escape_is_denied() {
        let root = fixture_root("ancestor");
        let sub = root.join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("file.txt"), b"inside").unwrap();
        let outside_dir = fixture_root("ancestor-out");
        std::fs::write(outside_dir.join("file.txt"), b"foreign bytes").unwrap();
        // Swap the authoritative ancestor for a directory link to a foreign
        // tree holding a same-named file.
        std::fs::remove_file(sub.join("file.txt")).unwrap();
        std::fs::remove_dir(&sub).unwrap();
        symlink_dir(&outside_dir, &sub);
        assert_eq!(
            read_full_file_via_kernel(&sub.join("file.txt"), &root, 1024).unwrap_err(),
            FullReadError::Adapter(AdapterError::EscapeDenied)
        );
        let _ = std::fs::remove_file(&sub);
        #[cfg(unix)]
        let _ = std::fs::remove_file(&sub);
        std::fs::remove_dir_all(&root).unwrap();
        std::fs::remove_dir_all(&outside_dir).unwrap();
    }

    #[test]
    fn hardlink_outside_domain_is_denied() {
        let root = fixture_root("hardlink");
        let outside_dir = fixture_root("hardlink-out");
        let outside = outside_dir.join("shared.txt");
        std::fs::write(&outside, b"shared bytes").unwrap();
        let alias = root.join("alias.txt");
        std::fs::hard_link(&outside, &alias).unwrap();
        assert_eq!(
            read_full_file_via_kernel(&alias, &root, 1024).unwrap_err(),
            FullReadError::Adapter(AdapterError::HardlinkDenied)
        );
        std::fs::remove_file(&alias).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        std::fs::remove_dir_all(&outside_dir).unwrap();
    }

    #[test]
    fn in_root_rename_leaves_no_product() {
        let root = fixture_root("rename");
        let before = root.join("before.txt");
        std::fs::write(&before, b"renamed bytes").unwrap();
        std::fs::rename(&before, root.join("after.txt")).unwrap();
        // The old locator no longer resolves; no bytes may be returned.
        assert_eq!(
            read_full_file_via_kernel(&before, &root, 1024).unwrap_err(),
            FullReadError::Adapter(AdapterError::AccessDenied)
        );
        // The renamed object under the same root still reads exactly.
        let read = read_full_file_via_kernel(&root.join("after.txt"), &root, 1024).unwrap();
        assert_eq!(read.bytes, b"renamed bytes");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn revocation_by_deletion_is_denied() {
        let root = fixture_root("revoke");
        let file = root.join("revoked.txt");
        std::fs::write(&file, b"revoked bytes").unwrap();
        std::fs::remove_file(&file).unwrap();
        assert_eq!(
            read_full_file_via_kernel(&file, &root, 1024).unwrap_err(),
            FullReadError::Adapter(AdapterError::AccessDenied)
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn revocation_by_permission_bits_is_denied() {
        use std::os::unix::fs::PermissionsExt;
        let root = fixture_root("chmod");
        let file = root.join("locked.txt");
        std::fs::write(&file, b"locked bytes").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
        let result = read_full_file_via_kernel(&file, &root, 1024);
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        // Root-owned test runners can still open mode-0 files; either the
        // read is denied or it returns the exact bytes, never a partial mix.
        match result {
            Err(FullReadError::Adapter(AdapterError::AccessDenied)) => {}
            Ok(read) => assert_eq!(read.bytes, b"locked bytes"),
            Err(other) => panic!("unexpected denial: {other}"),
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn empty_source_reads_empty_and_oversized_fails_before_allocation() {
        let root = fixture_root("sizes");
        let empty = root.join("empty.bin");
        std::fs::write(&empty, b"").unwrap();
        let read = read_full_file_via_kernel(&empty, &root, 1024).unwrap();
        assert!(read.bytes.is_empty());
        assert_eq!(read.source_bytes, 0);
        let big = root.join("big.bin");
        std::fs::write(&big, b"0123456789").unwrap();
        assert_eq!(
            read_full_file_via_kernel(&big, &root, 4).unwrap_err(),
            FullReadError::Adapter(AdapterError::TooLarge)
        );
        // Zero budget still admits the empty source and denies the rest.
        let read = read_full_file_via_kernel(&empty, &root, 0).unwrap();
        assert!(read.bytes.is_empty());
        assert_eq!(
            read_full_file_via_kernel(&big, &root, 0).unwrap_err(),
            FullReadError::Adapter(AdapterError::TooLarge)
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn failures_are_content_free() {
        let root = fixture_root("redact");
        let name = "marker-name-7f3a9c.txt";
        let marker = "marker-content-51bd77e2";
        std::fs::write(root.join(name), marker.as_bytes()).unwrap();
        let error =
            read_full_file_via_kernel(&root.join(name), &root, marker.len() - 1).unwrap_err();
        assert_eq!(error, FullReadError::Adapter(AdapterError::TooLarge));
        let rendered = format!("{error}");
        assert_eq!(rendered, "SAFE_ADAPTER_TOO_LARGE");
        assert!(!rendered.contains("marker"));
        assert!(!rendered.contains("7f3a9c"));
        let debug = format!("{error:?}");
        assert!(!debug.contains("marker"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn script_source_is_inert_data_and_byte_exact() {
        let root = fixture_root("noexec");
        // A hostile-looking payload plus non-UTF8 bytes must round-trip as
        // inert data; nothing here may execute or normalize anything.
        let mut payload = b"@echo PWNED-MARKER-9d2c\r\nWrite-Host hi\n".to_vec();
        payload.extend_from_slice(&[0x00, 0xFF, 0xFE, 0x80, 0xC3, 0x28]);
        let file = root.join("payload.bat");
        std::fs::write(&file, &payload).unwrap();
        let read = read_full_file_via_kernel(&file, &root, 1024).unwrap();
        assert_eq!(read.bytes, payload);
        assert_eq!(read.source_bytes, payload.len() as u64);
        // No side effect was executed: no marker file exists anywhere.
        assert!(!root.join("PWNED-MARKER-9d2c").try_exists().unwrap());
        assert!(!root.join("payload.out").try_exists().unwrap());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn root_relocation_is_denied_explicitly() {
        let base = fixture_root("reloc");
        let root_a = base.join("root_a");
        std::fs::create_dir(&root_a).unwrap();
        std::fs::write(root_a.join("file.txt"), b"relocated").unwrap();
        let locator = derive_locator(&root_a.join("file.txt"), &root_a).unwrap();
        let mut backend =
            FinalHandleBackend::bind(&locator.canonical_root, &locator.token, barrier(), 1024)
                .unwrap();
        // Relocate the admitted root: move the directory away and plant a
        // link at its former canonical spelling.
        let root_b = base.join("root_b");
        std::fs::rename(&root_a, &root_b).unwrap();
        symlink_dir(&root_b, &root_a);
        let request = FinalHandleOpenRequest {
            relative_path: search_safe_reader::RelativePathToken::new(
                &locator.token,
                search_safe_reader::DEFAULT_SAFE_READ_LIMITS,
            )
            .unwrap(),
            expected_root_identity_digest: backend.root_digest(),
        };
        // Either the stale base is rejected outright or the live digest
        // mismatch surfaces; both fail closed without bytes.
        match backend.open_final(&request) {
            Err(AdapterError::RootRelocated) => {}
            Ok(handle) => {
                let metadata = backend.inspect(&handle).unwrap();
                assert_ne!(metadata.root_identity_digest, backend.root_digest());
            }
            Err(other) => panic!("unexpected open outcome: {other}"),
        }
        let _ = std::fs::remove_file(&root_a);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
