//! Atomic cutover of the primary control authority from the file journal to
//! the verified redb mapping.
//!
//! Canonical marker schema, codec and replay classification are owned by
//! `search-control-redb::migration`. This daemon module owns only data-root
//! filesystem publication, quarantine interaction, status rendering and the
//! `DirectStore` orchestration that binds live legacy readback to that marker.
//!
//! The file journal stays on disk as preserved evidence. Serve-path query and
//! mutation rerouting consumes the marker in a follow-up wiring step; until
//! then ordinary requests keep appending the legacy journal and receipts state
//! that limitation explicitly.

#[path = "control_migration_cutover/marker_io.rs"]
mod marker_io;
#[path = "control_migration_cutover/operation.rs"]
mod operation;
#[path = "control_migration_cutover/status.rs"]
mod status;

pub(super) use marker_io::gate_staging_against_marker;

#[cfg(test)]
#[path = "control_migration_cutover/tests.rs"]
mod tests;
