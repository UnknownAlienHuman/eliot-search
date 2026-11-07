//! Plain local root inspection, initialization and process-lifetime exclusion.
//!
//! Root operations share one stable lock and the control owner's actual
//! readonly format and bounded emptiness checks.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_lines
)]

pub mod plain_root;

pub use plain_root::{
    CONTROL_DB_FILE, CONTROL_DIR, INCOMPLETE_IMPORT_MARKER, INIT_MARKER, LOCK_FILE, MANIFEST_FILE,
    OBJECT_DIR, PlainRootError, PlainRootGuard, PlainRootInspection, PlainRootResult,
    PlainRootStatus, discard_incomplete_plain, initialize_plain, inspect_plain, open_plain,
    open_plain_for_index,
};
