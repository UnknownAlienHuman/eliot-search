//! Persistent quarantine marker for uncertain catalog effects.
//!
//! The primary `--serve-data-root` session arms this marker immediately before
//! a possibly durable catalog mutation and clears it only after exact readback
//! succeeds. While the marker exists the service refuses queries, mutations,
//! GC and READY claims and invalidates handles. The marker survives restarts;
//! reopen observes it instead of inventing empty state. Explicit disposable-root
//! recovery removes it; normal serving never performs implicit repair.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Service refusal while a quarantine marker is active.
pub const QUARANTINE_ERROR: &str = "SERVICE_CATALOG_QUARANTINED";
/// Persistent arm failed before any storage effect was dispatched.
pub const QUARANTINE_ARM_FAILED: &str = "SERVICE_CATALOG_QUARANTINE_ARM_FAILED";
/// Persistent clear failed after storage success; the marker is retained.
pub const QUARANTINE_CLEAR_FAILED: &str = "SERVICE_CATALOG_QUARANTINE_CLEAR_FAILED";

const QUARANTINE_FILE: &str = "catalog-quarantine.marker";
const QUARANTINE_TMP: &str = "catalog-quarantine.tmp";
const MAX_MARKER_BYTES: u64 = 256;
const MARKER_PAYLOAD: &[u8] =
    b"ELIOT_SEARCH_CATALOG_QUARANTINE_V1\nreason=SERVICE_MUTATION_OUTCOME_UNKNOWN\n";

/// Returns true when the persistent marker is present or cannot be proven absent.
///
/// # Panics
///
/// Never panics; inspection failures are fail-closed as quarantined.
pub fn is_quarantined(root: &Path) -> bool {
    !matches!(
        fs::symlink_metadata(marker_path(root)),
        Err(error) if error.kind() == io::ErrorKind::NotFound
    )
}

/// Refuses serving while quarantined without touching the marker.
///
/// # Errors
///
/// Returns `SERVICE_CATALOG_QUARANTINED` when the marker is present or
/// cannot be proven absent.
pub fn check(root: &Path) -> Result<(), String> {
    if is_quarantined(root) {
        Err(QUARANTINE_ERROR.to_owned())
    } else {
        Ok(())
    }
}

/// Reports whether a service error code is a quarantine fatal.
///
/// Quarantine refusals must terminate the session even without a dispatched
/// mutation attempt so no queued command can use stale memory.
#[must_use]
pub fn is_quarantine_error(code: &str) -> bool {
    matches!(
        code,
        QUARANTINE_ERROR | QUARANTINE_ARM_FAILED | QUARANTINE_CLEAR_FAILED
    )
}

/// Arms the persistent marker before a possibly durable catalog mutation.
///
/// The write is bounded to 75 exact bytes and atomic (`temp + sync + rename`).
/// An existing marker of any shape is preserved; corrupt markers are never
/// repaired here.
///
/// # Errors
///
/// Returns `SERVICE_CATALOG_QUARANTINE_ARM_FAILED` when the marker cannot be
/// persisted. The caller must not dispatch storage effects in that case.
pub fn arm(root: &Path) -> Result<(), String> {
    if is_quarantined(root) {
        return Ok(());
    }
    let control = root.join("control");
    let tmp = control.join(QUARANTINE_TMP);
    let marker = control.join(QUARANTINE_FILE);
    let _ = fs::remove_file(&tmp);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&tmp)
        .map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
    if let Err(error) = file
        .write_all(MARKER_PAYLOAD)
        .and_then(|()| file.sync_all())
    {
        let _ = fs::remove_file(&tmp);
        let _ = error;
        return Err(QUARANTINE_ARM_FAILED.to_owned());
    }
    drop(file);
    if fs::rename(&tmp, &marker).is_err() {
        let _ = fs::remove_file(&tmp);
        return Err(QUARANTINE_ARM_FAILED.to_owned());
    }
    #[cfg(unix)]
    sync_directory(&control).map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
    #[cfg(not(unix))]
    sync_directory(&control);
    if is_quarantined(root) {
        Ok(())
    } else {
        Err(QUARANTINE_ARM_FAILED.to_owned())
    }
}

/// Clears the marker only when its bytes exactly match the armed payload.
///
/// Corrupt, oversized or non-regular markers are preserved for explicit
/// recovery; they are never reinterpreted as absent or repaired here.
///
/// # Errors
///
/// Returns `SERVICE_CATALOG_QUARANTINED` when the marker is present but not
/// exactly the armed payload, and `SERVICE_CATALOG_QUARANTINE_CLEAR_FAILED`
/// when the exact marker cannot be removed.
pub fn clear(root: &Path) -> Result<(), String> {
    let marker = marker_path(root);
    let metadata = match fs::symlink_metadata(&marker) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(QUARANTINE_CLEAR_FAILED.to_owned()),
    };
    if !metadata.is_file() || is_link(&metadata) {
        return Err(QUARANTINE_ERROR.to_owned());
    }
    if metadata.len() > MAX_MARKER_BYTES {
        return Err(QUARANTINE_ERROR.to_owned());
    }
    let bytes = fs::read(&marker).map_err(|_| QUARANTINE_CLEAR_FAILED.to_owned())?;
    if bytes.as_slice() != MARKER_PAYLOAD {
        return Err(QUARANTINE_ERROR.to_owned());
    }
    fs::remove_file(&marker).map_err(|_| QUARANTINE_CLEAR_FAILED.to_owned())?;
    if let Some(parent) = marker.parent() {
        #[cfg(unix)]
        sync_directory(parent).map_err(|_| QUARANTINE_CLEAR_FAILED.to_owned())?;
        #[cfg(not(unix))]
        sync_directory(parent);
    }
    match fs::symlink_metadata(&marker) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        _ => Err(QUARANTINE_CLEAR_FAILED.to_owned()),
    }
}

fn marker_path(root: &Path) -> PathBuf {
    root.join("control").join(QUARANTINE_FILE)
}

fn is_link(value: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        value.file_type().is_symlink() || value.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        value.file_type().is_symlink()
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| "SERVICE_CATALOG_QUARANTINE_SYNC_FAILED".to_owned())
}

#[cfg(not(unix))]
const fn sync_directory(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "eliot-quarantine-unit-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("control")).unwrap();
            Self(root)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn arm_creates_bounded_exact_marker_and_check_refuses() {
        let root = Scratch::new();
        assert!(!is_quarantined(&root.0));
        assert!(check(&root.0).is_ok());
        arm(&root.0).unwrap();
        assert!(is_quarantined(&root.0));
        assert_eq!(check(&root.0), Err(QUARANTINE_ERROR.to_owned()));
        let bytes = fs::read(marker_path(&root.0)).unwrap();
        assert_eq!(bytes, MARKER_PAYLOAD);
        assert!(bytes.len() <= 256);
    }

    #[test]
    fn clear_removes_only_exact_payload_and_is_idempotent_when_absent() {
        let root = Scratch::new();
        assert!(clear(&root.0).is_ok());
        arm(&root.0).unwrap();
        clear(&root.0).unwrap();
        assert!(!is_quarantined(&root.0));
        assert!(clear(&root.0).is_ok());
    }

    #[test]
    fn corrupt_and_oversized_markers_are_fail_closed_and_preserved() {
        for payload in [b"corrupt".as_slice(), &[b'X'; 300], b""] {
            let root = Scratch::new();
            fs::write(marker_path(&root.0), payload).unwrap();
            assert!(is_quarantined(&root.0));
            assert_eq!(check(&root.0), Err(QUARANTINE_ERROR.to_owned()));
            assert!(clear(&root.0).is_err());
            assert_eq!(fs::read(marker_path(&root.0)).unwrap(), payload);
            // Arming never repairs a corrupt marker implicitly.
            arm(&root.0).unwrap();
            assert_eq!(fs::read(marker_path(&root.0)).unwrap(), payload);
        }
    }

    #[test]
    fn arm_is_idempotent_and_reopen_observes_persistent_marker() {
        let root = Scratch::new();
        arm(&root.0).unwrap();
        let first = fs::read(marker_path(&root.0)).unwrap();
        arm(&root.0).unwrap();
        assert_eq!(fs::read(marker_path(&root.0)).unwrap(), first);
        // A reopened view of the same canonical root sees the same marker.
        let reopened = root.0.clone();
        assert!(is_quarantined(&reopened));
        assert_eq!(check(&reopened), Err(QUARANTINE_ERROR.to_owned()));
        clear(&reopened).unwrap();
        assert!(!is_quarantined(&root.0));
    }

    #[test]
    fn quarantine_errors_are_fatal_session_codes() {
        assert!(is_quarantine_error(QUARANTINE_ERROR));
        assert!(is_quarantine_error(QUARANTINE_ARM_FAILED));
        assert!(is_quarantine_error(QUARANTINE_CLEAR_FAILED));
        assert!(!is_quarantine_error("SERVICE_COMMAND_INVALID"));
        assert!(!is_quarantine_error("SERVICE_MUTATION_OUTCOME_UNKNOWN"));
    }
}
