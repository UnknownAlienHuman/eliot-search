//! Distinguish a fresh DIRECT root from missing authoritative catalog files.
//!
//! Read-only preflight, executed under the existing data-root owner lock.
//! This does not recover a journal or prove its contents; normal catalog
//! verification still follows. It never infers references from payload files.

use std::fs::{self, Metadata};
use std::io;
use std::path::Path;

/// Initialization is permitted only when neither catalog file nor residual
/// corpus state exists. One missing catalog file is never a new installation.
pub(crate) fn check_before_open(root: &Path) -> Result<(), String> {
    match catalog_files(root)? {
        (true, true) => Ok(()),
        (false, false) => require_fresh_payload_state(root),
        _ => Err("DIRECT_CONTROL_INCOMPLETE_RECOVERY_REQUIRED".to_owned()),
    }
}

/// Verification and GC may inspect existing state, but must not initialize it.
pub(crate) fn require_existing(root: &Path) -> Result<(), String> {
    if catalog_files(root)? == (true, true) {
        Ok(())
    } else {
        Err("DIRECT_CONTROL_INCOMPLETE_RECOVERY_REQUIRED".to_owned())
    }
}

fn catalog_files(root: &Path) -> Result<(bool, bool), String> {
    require_directory(root)?;
    let control = root.join("control");
    match metadata(&control)? {
        None => return Ok((false, false)),
        Some(value) if safe_directory(&value) => {}
        Some(_) => return Err("DIRECT_CONTROL_DIRECTORY_INVALID".to_owned()),
    }
    let namespace = regular_file_present(&control.join("namespace.id"))?;
    let log = regular_file_present(&control.join("source-events.log"))?;
    Ok((namespace, log))
}

fn require_fresh_payload_state(root: &Path) -> Result<(), String> {
    let revisions = root.join("revisions");
    if let Some(value) = metadata(&revisions)? {
        if !safe_directory(&value) {
            return Err("DIRECT_REVISION_DIRECTORY_INVALID".to_owned());
        }
        // Even an empty shard signals prior storage activity. No recursive or
        // unbounded allocation is needed to reject residual corpus state.
        if fs::read_dir(&revisions).map_err(inspect_error)?.next().transpose()
            .map_err(inspect_error)?.is_some()
        {
            return Err("DIRECT_RESIDUAL_CORPUS_RECOVERY_REQUIRED".to_owned());
        }
    }
    let control = root.join("control");
    if metadata(&control)?.is_some() {
        let entries = fs::read_dir(&control).map_err(inspect_error)?;
        for entry in entries {
            let entry = entry.map_err(inspect_error)?;
            // Root registration is allowed before a corpus is first opened.
            // Its bounded parser/recovery is owned by SourceRootCatalog.
            let name = entry.file_name();
            if !matches!(name.to_str(), Some("source-roots.v1" | "source-roots.tmp" | "source-roots.bak"))
                || !regular_file_present(&entry.path())?
            {
                return Err("DIRECT_RESIDUAL_CONTROL_RECOVERY_REQUIRED".to_owned());
            }
        }
    }
    Ok(())
}

fn regular_file_present(path: &Path) -> Result<bool, String> {
    match metadata(path)? {
        None => Ok(false),
        Some(value) if value.is_file() && !is_link(&value) => Ok(true),
        Some(_) => Err("DIRECT_CONTROL_FILE_INVALID".to_owned()),
    }
}

fn require_directory(path: &Path) -> Result<(), String> {
    if metadata(path)?.is_some_and(|value| safe_directory(&value)) {
        Ok(())
    } else {
        Err("DIRECT_ROOT_INVALID".to_owned())
    }
}

fn metadata(path: &Path) -> Result<Option<Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(inspect_error(error)),
    }
}

fn inspect_error(_: io::Error) -> String {
    "DIRECT_CONTROL_INSPECTION_FAILED".to_owned()
}

fn safe_directory(value: &Metadata) -> bool { value.is_dir() && !is_link(value) }
fn is_link(value: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        value.file_type().is_symlink() || value.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    { value.file_type().is_symlink() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let root = std::env::temp_dir().join(format!("eliot-presence-{}-{stamp}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir_all(root.join("control")).unwrap();
            fs::create_dir(root.join("revisions")).unwrap();
            Self(root)
        }
        fn write(&self, name: &str, bytes: &[u8]) { fs::write(self.0.join(name), bytes).unwrap(); }
    }
    impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

    #[test]
    fn empty_root_can_initialize_but_read_only_paths_cannot() {
        let root = Scratch::new();
        assert!(check_before_open(&root.0).is_ok());
        assert!(require_existing(&root.0).is_err());
        assert_eq!(fs::read_dir(root.0.join("control")).unwrap().count(), 0);
    }

    #[test]
    fn either_missing_catalog_file_is_refused_without_recreating_it() {
        for survivor in ["namespace.id", "source-events.log"] {
            let root = Scratch::new();
            root.write(&format!("control/{survivor}"), b"retained-catalog");
            assert_eq!(check_before_open(&root.0), Err("DIRECT_CONTROL_INCOMPLETE_RECOVERY_REQUIRED".to_owned()));
            assert!(require_existing(&root.0).is_err());
            assert_eq!(fs::read(root.0.join("control").join(survivor)).unwrap(), b"retained-catalog");
            assert_eq!(fs::read_dir(root.0.join("control")).unwrap().count(), 1);
        }
    }

    #[test]
    fn loss_of_both_files_cannot_hide_remaining_revision_objects() {
        let root = Scratch::new();
        root.write("revisions/payload", b"irreplaceable");
        assert_eq!(check_before_open(&root.0), Err("DIRECT_RESIDUAL_CORPUS_RECOVERY_REQUIRED".to_owned()));
        assert_eq!(fs::read(root.0.join("revisions/payload")).unwrap(), b"irreplaceable");
        assert_eq!(fs::read_dir(root.0.join("control")).unwrap().count(), 0);
    }

    #[test]
    fn existing_files_still_require_the_normal_content_validator() {
        let root = Scratch::new();
        root.write("control/namespace.id", b"malformed");
        root.write("control/source-events.log", b"malformed");
        // Presence is not integrity. The caller must parse and verify both.
        assert!(require_existing(&root.0).is_ok());
    }

    #[test]
    fn pre_registered_roots_are_allowed_but_other_control_residue_is_not() {
        let root = Scratch::new();
        root.write("control/source-roots.v1", b"registration");
        assert!(check_before_open(&root.0).is_ok());
        root.write("control/control.redb", b"existing-state");
        assert_eq!(check_before_open(&root.0), Err("DIRECT_RESIDUAL_CONTROL_RECOVERY_REQUIRED".to_owned()));
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_not_mistaken_for_an_absent_journal() {
        let root = Scratch::new();
        std::os::unix::fs::symlink("missing-target", root.0.join("control/source-events.log")).unwrap();
        assert_eq!(check_before_open(&root.0), Err("DIRECT_CONTROL_FILE_INVALID".to_owned()));
    }
}
