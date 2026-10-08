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
mod persist;
#[path = "kernel/spec.rs"]
mod spec;
#[path = "kernel/sync.rs"]
mod sync;

pub use load::verify_directory_manifests;
pub use migration::{migration_manifest, migration_manifest_files};
pub use model::{
    DirectoryEntry, DirectoryManifest, DirectoryManifestVerification,
    DirectorySyncResult,
};
pub use paths::path_identity_bytes;
pub use sync::sync_directory;
