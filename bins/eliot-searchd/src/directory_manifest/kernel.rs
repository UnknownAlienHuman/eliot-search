//! Directory-manifest composition behind the stable module facade.

#[path = "kernel/codec.rs"]
mod codec;
#[path = "kernel/load.rs"]
mod load;
#[path = "kernel/migration.rs"]
mod migration;
#[path = "kernel/model.rs"]
mod model;
#[path = "kernel/paths.rs"]
mod paths;
#[path = "kernel/persist.rs"]
#[cfg(test)]
mod persist;
#[path = "kernel/spec.rs"]
mod spec;
#[path = "kernel/sync.rs"]
#[cfg(test)]
mod sync;

pub use load::verify_directory_manifests;
pub(crate) use migration::inventory_manifest;
#[cfg(test)]
pub use migration::{migration_manifest, migration_manifest_files};
#[cfg(test)]
pub use model::{
    DirectoryEntry, DirectoryManifest, DirectoryManifestVerification,
};
#[cfg(test)]
pub use model::DirectorySyncResult;
#[cfg(test)]
pub use paths::path_identity_bytes;
#[cfg(test)]
pub use sync::sync_directory;
