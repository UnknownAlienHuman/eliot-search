//! Relative-token grammar, admitted-root proof and locator derivation.

use std::fs;
use std::path::{Component, Path, PathBuf};

use super::spec::AdapterError;

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

/// Walks every authoritative ancestor of `canonical_final` up to and
/// including `canonical_root`, denying symlink/reparse boundaries.
pub(super) fn verify_ancestor_containment(
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

#[cfg(unix)]
pub(super) fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
pub(super) fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
pub(super) fn path_identity_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(windows)]
pub(super) fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(super) fn is_reparse(_metadata: &std::fs::Metadata) -> bool {
    false
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
