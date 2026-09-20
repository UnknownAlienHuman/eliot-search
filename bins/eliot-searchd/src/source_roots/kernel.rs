//! Observation-root composition behind the stable daemon-local facade.

mod catalog;
mod error;
mod model;
mod path;
mod registry;
mod spec;

pub use catalog::SourceRootCatalog;
pub use error::SourceRootError;
pub use model::SourceRootView;
pub use registry::{RootMigrationInput, migration_input};
pub use search_source_registry::{
    CurrentWorkspaceTruth, ObservationGap, ObservationGapReason,
    ReconciliationCursor, SourceRootState, WatcherHint, WatcherHintKind,
};
pub use spec::{
    MAX_OBSERVATION_GAPS, MAX_SOURCE_ROOT_FILE_BYTES,
    MAX_SOURCE_ROOT_PATH_BYTES, MAX_SOURCE_ROOTS, MAX_WATCHER_HINTS,
};

#[cfg(test)]
pub(super) use spec::HEADER;

#[cfg(test)]
mod tests;
