//! Frozen pure codec for the legacy `control/source-roots.v1` catalog.
//!
//! The source registry owns the exact header, line framing, finite file/root
//! bounds and duplicate detection. Callers retain platform path semantics,
//! canonicalization, overlap policy, filesystem reads/publication and crash
//! recovery.

use core::fmt;
use std::collections::BTreeSet;

/// Exact first line of the legacy source-root registration catalog.
pub const LEGACY_SOURCE_ROOT_CATALOG_HEADER: &str =
    "# ELIOT Search source roots v1";
/// Maximum encoded bytes in one legacy source-root registration catalog.
pub const LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES: usize = 64 * 1024;
/// Maximum root locator lines in one legacy source-root registration catalog.
pub const LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS: usize = 32;

/// Closed failure from the legacy source-root catalog codec.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacySourceRootCatalogError {
    /// The input or output exceeded the finite root-count ceiling.
    TooManyRoots,
    /// The encoded catalog exceeded the finite byte ceiling.
    FileTooLarge,
    /// Catalog bytes were not valid UTF-8.
    NotUtf8,
    /// The catalog did not end with the mandatory newline.
    MissingFinalNewline,
    /// The first line was not the exact versioned header.
    InvalidHeader,
    /// The same exact locator line appeared more than once.
    DuplicateRoot,
}

impl LegacySourceRootCatalogError {
    /// Stable package-local reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TooManyRoots => "REGISTRY_LEGACY_ROOT_CATALOG_TOO_MANY_ROOTS",
            Self::FileTooLarge => "REGISTRY_LEGACY_ROOT_CATALOG_TOO_LARGE",
            Self::NotUtf8 => "REGISTRY_LEGACY_ROOT_CATALOG_NOT_UTF8",
            Self::MissingFinalNewline => {
                "REGISTRY_LEGACY_ROOT_CATALOG_FINAL_NEWLINE_MISSING"
            }
            Self::InvalidHeader => "REGISTRY_LEGACY_ROOT_CATALOG_HEADER_INVALID",
            Self::DuplicateRoot => "REGISTRY_LEGACY_ROOT_CATALOG_DUPLICATE_ROOT",
        }
    }
}

impl fmt::Display for LegacySourceRootCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LegacySourceRootCatalogError {}

/// Decodes one exact legacy source-root catalog into locator text lines.
///
/// Locator contents remain opaque here. The caller must still validate platform
/// path syntax, canonicalization, containment, overlap and current filesystem
/// identity before using any decoded line.
pub fn decode_legacy_source_root_catalog(
    bytes: &[u8],
) -> Result<Vec<String>, LegacySourceRootCatalogError> {
    if bytes.len() > LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES {
        return Err(LegacySourceRootCatalogError::FileTooLarge);
    }
    let text = core::str::from_utf8(bytes)
        .map_err(|_| LegacySourceRootCatalogError::NotUtf8)?;
    if !text.ends_with('\n') {
        return Err(LegacySourceRootCatalogError::MissingFinalNewline);
    }
    let mut lines = text.split_terminator('\n');
    if lines.next() != Some(LEGACY_SOURCE_ROOT_CATALOG_HEADER) {
        return Err(LegacySourceRootCatalogError::InvalidHeader);
    }

    let mut roots = Vec::new();
    let mut seen = BTreeSet::new();
    for line in lines {
        if roots.len() >= LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS {
            return Err(LegacySourceRootCatalogError::TooManyRoots);
        }
        if !seen.insert(line) {
            return Err(LegacySourceRootCatalogError::DuplicateRoot);
        }
        roots.push(line.to_owned());
    }
    Ok(roots)
}

/// Encodes locator text lines using the exact legacy catalog representation.
///
/// Input order is preserved for byte compatibility. Callers own canonical
/// ordering and platform path validation before invoking this codec.
pub fn encode_legacy_source_root_catalog<I, S>(
    roots: I,
) -> Result<Vec<u8>, LegacySourceRootCatalogError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut body = String::with_capacity(256);
    body.push_str(LEGACY_SOURCE_ROOT_CATALOG_HEADER);
    body.push('\n');

    let mut seen = BTreeSet::new();
    for (index, root) in roots.into_iter().enumerate() {
        if index >= LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS {
            return Err(LegacySourceRootCatalogError::TooManyRoots);
        }
        let root = root.as_ref();
        if !seen.insert(root.to_owned()) {
            return Err(LegacySourceRootCatalogError::DuplicateRoot);
        }
        body.push_str(root);
        body.push('\n');
        if body.len() > LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES {
            return Err(LegacySourceRootCatalogError::FileTooLarge);
        }
    }
    Ok(body.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_catalog_bytes_round_trip_without_reordering() {
        let roots = ["C:\\source one", "D:\\source-two"];
        let encoded = encode_legacy_source_root_catalog(roots).expect("encode");
        assert_eq!(
            encoded.as_slice(),
            b"# ELIOT Search source roots v1\nC:\\source one\nD:\\source-two\n"
        );
        assert_eq!(
            decode_legacy_source_root_catalog(&encoded).expect("decode"),
            roots
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn empty_catalog_is_exact_header_plus_newline() {
        let encoded = encode_legacy_source_root_catalog(core::iter::empty::<&str>())
            .expect("encode empty");
        assert_eq!(encoded.as_slice(), b"# ELIOT Search source roots v1\n");
        assert!(
            decode_legacy_source_root_catalog(&encoded)
                .expect("decode empty")
                .is_empty()
        );
    }

    #[test]
    fn duplicate_and_noncanonical_framing_fail_closed() {
        assert_eq!(
            decode_legacy_source_root_catalog(
                b"# ELIOT Search source roots v1\nC:\\one\nC:\\one\n"
            ),
            Err(LegacySourceRootCatalogError::DuplicateRoot)
        );
        assert_eq!(
            decode_legacy_source_root_catalog(b"wrong\n"),
            Err(LegacySourceRootCatalogError::InvalidHeader)
        );
        assert_eq!(
            decode_legacy_source_root_catalog(
                b"# ELIOT Search source roots v1\nC:\\one"
            ),
            Err(LegacySourceRootCatalogError::MissingFinalNewline)
        );
        assert_eq!(
            decode_legacy_source_root_catalog(&[0xff, b'\n']),
            Err(LegacySourceRootCatalogError::NotUtf8)
        );
    }

    #[test]
    fn finite_root_and_byte_bounds_are_enforced() {
        let roots = (0..=LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS)
            .map(|index| format!("C:\\root-{index}"))
            .collect::<Vec<_>>();
        assert_eq!(
            encode_legacy_source_root_catalog(&roots),
            Err(LegacySourceRootCatalogError::TooManyRoots)
        );

        let oversized = "x".repeat(LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES);
        assert_eq!(
            encode_legacy_source_root_catalog([oversized]),
            Err(LegacySourceRootCatalogError::FileTooLarge)
        );
        assert_eq!(
            decode_legacy_source_root_catalog(
                &vec![b'x'; LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES + 1]
            ),
            Err(LegacySourceRootCatalogError::FileTooLarge)
        );
    }

    #[test]
    fn reason_codes_are_stable() {
        assert_eq!(
            LegacySourceRootCatalogError::DuplicateRoot.code(),
            "REGISTRY_LEGACY_ROOT_CATALOG_DUPLICATE_ROOT"
        );
        assert_eq!(LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS, 32);
        assert_eq!(LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES, 64 * 1024);
    }
}
