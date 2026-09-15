//! Closed directory-manifest format and finite resource ceilings.

pub(super) const CONTROL_DIRECTORY: &str = "control";
pub(super) const MANIFEST_DIRECTORY: &str = "directory-manifests";
pub(super) const MANIFEST_HEADER: &str = "ELIOT_SEARCH_DIRECTORY_MANIFEST_V1";
pub(super) const MAX_MANIFEST_BYTES: usize = 128 * 1024 * 1024;
pub(super) const MAX_MANIFEST_ENTRIES: usize = 100_000;
pub(super) const MAX_MANIFEST_FILES: usize = 1_000_000;
pub(super) const MAX_MANIFEST_LINE_BYTES: usize = 1_024;
