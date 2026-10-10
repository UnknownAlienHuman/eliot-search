//! Observation-root composition behind the stable daemon-local facade.

#[path = "kernel/catalog.rs"]
mod catalog;
#[path = "kernel/error.rs"]
mod error;
#[path = "kernel/model.rs"]
mod model;
#[path = "kernel/path.rs"]
mod path;
#[path = "kernel/registry.rs"]
mod registry;
#[path = "kernel/spec.rs"]
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
#[path = "kernel/tests.rs"]
mod tests;
