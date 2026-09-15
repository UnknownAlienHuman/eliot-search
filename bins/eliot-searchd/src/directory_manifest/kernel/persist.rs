//! Immutable manifest publication and exact readback compatibility.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::codec::{encode_manifest, manifest_path};
use super::load::load_manifest_file;
use super::model::DirectoryManifest;
use super::paths::sync_manifest_directory;

pub(super) fn persist_manifest(
    root: &Path,
    manifest: &DirectoryManifest,
) -> Result<(), String> {
    let final_path = manifest_path(root, manifest);
    if final_path.exists() {
        let existing = load_manifest_file(&final_path)?;
        return if existing == *manifest {
            Ok(())
        } else {
            Err("DIRECT_MANIFEST_IMMUTABLE_CONFLICT".to_owned())
        };
    }

    let encoded = encode_manifest(manifest)?;
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
    if let Err(error) = file
        .write_all(encoded.as_bytes())
        .and_then(|()| file.sync_all())
    {
        let _ = fs::remove_file(&temporary);
        return Err(format!("DIRECT_MANIFEST_WRITE_ERROR:{error}"));
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, &final_path) {
        let _ = fs::remove_file(&temporary);
        if final_path.exists()
            && load_manifest_file(&final_path)? == *manifest
        {
            return Ok(());
        }
        return Err(format!("DIRECT_MANIFEST_RENAME_ERROR:{error}"));
    }
    #[cfg(unix)]
    sync_manifest_directory(root)?;
    #[cfg(not(unix))]
    sync_manifest_directory(root);
    Ok(())
}
