//! Observation-root locator models behind package-owned currentness.

use std::path::PathBuf;

use search_source_registry::SourceRootState;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct SourceRootEntry {
    pub(super) configured_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRootView {
    pub(crate) index: usize,
    pub(crate) path: String,
    pub(crate) state: SourceRootState,
}
