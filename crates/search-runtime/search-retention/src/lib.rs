//! Transitional empty package after removal of the generic lifecycle platform.
//!
//! Current local-object collection, archive and fresh-destination import are
//! concrete responsibilities of the selected store/control/publication owners.
//! This package intentionally exposes no compatibility API. Its workspace and
//! lockfile entries remain only until the dedicated Cargo-manifest pruning cut.

#![forbid(unsafe_code)]
