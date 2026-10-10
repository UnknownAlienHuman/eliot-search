//! Immutable manifest publication and exact readback compatibility.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::codec::{encode_manifest, manifest_path};
use super::load::load_manifest_file_with_check;
use super::model::DirectoryManifest;
use super::paths::sync_manifest_directory;

pub(super) fn persist_manifest(
    root: &Path,
    manifest: &DirectoryManifest,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<(), String> {
    check()?;
    let final_path = manifest_path(root, manifest);
    if final_path.exists() {
        let existing = load_manifest_file_with_check(&final_path, check)?;
        return if existing == *manifest {
            Ok(())
        } else {
            Err("DIRECT_MANIFEST_IMMUTABLE_CONFLICT".to_owned())
        };
    }

    let encoded = encode_manifest(manifest)?;
    check()?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "DIRECT_MANIFEST_CLOCK_INVALID".to_owned())?
        .as_nanos();
    let temporary = root.join(format!(
        ".{}.{}.{}.tmp",
        manifest.directory_digest,
        std::process::id(),
        timestamp,
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("DIRECT_MANIFEST_CREATE_ERROR:{error}"))?;
    // A cancelled request keeps any partial durable object for reconciliation.
    check()?;
    if let Err(error) = file
        .write_all(encoded.as_bytes())
        .and_then(|()| file.sync_all())
    {
        // An I/O error may coincide with cancellation after a partial write.
        // Preserve the exact attempted object for later reconciliation.
        check()?;
        return Err(format!("DIRECT_MANIFEST_WRITE_ERROR:{error}"));
    }
    drop(file);
    check()?;
    if let Err(error) = fs::rename(&temporary, &final_path) {
        // Reconcile only the exact final object; never erase a pending attempt.
        check()?;
        if final_path.exists() && load_manifest_file_with_check(&final_path, check)? == *manifest {
            return Ok(());
        }
        return Err(format!("DIRECT_MANIFEST_RENAME_ERROR:{error}"));
    }
    #[cfg(unix)]
    sync_manifest_directory(root)?;
    #[cfg(not(unix))]
    sync_manifest_directory(root);
    check()?;
    Ok(())
}
