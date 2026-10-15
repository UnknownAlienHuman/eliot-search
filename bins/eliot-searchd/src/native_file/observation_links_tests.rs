//! Real same-FD snapshot/link probes; callbacks never replace native facts.
use super::*;
use std::cell::{Cell, RefCell};
use std::fs;
use std::io::{Seek, Write};
use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        let path = temp.join(format!("search-observation-links-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if std::thread::panicking() { return; }
        let path = fs::canonicalize(&self.0).unwrap();
        assert_eq!(path, self.0);
        assert_eq!(path.parent(), Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path()));
        assert!(path.file_name().unwrap().to_str().unwrap().starts_with("search-observation-links-"));
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn coherent_fields_and_links_use_one_three_abi_snapshot() {
    let scratch = Scratch::new();
    let path = scratch.0.join("original");
    let alias = scratch.0.join("alias");
    fs::write(&path, b"original").unwrap();
    let mut file = File::open(&path).unwrap();
    let position = file.stream_position().unwrap();
    let before = observe(&file).unwrap();
    let calls = Cell::new(0);
    let events = RefCell::new(Vec::new());
    let (observed, links) = observe_with_links_with_check(&file, &|pre| {
        events.borrow_mut().push(pre);
        if pre {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                // Change real fields and links immediately before the one
                // BY_HANDLE_FILE_INFORMATION dispatch, after disk/NTFS checks.
                fs::hard_link(&path, &alias).unwrap();
                let mut writer = fs::OpenOptions::new().append(true).open(&path).unwrap();
                writer.write_all(b" plus live bytes").unwrap();
                writer.sync_all().unwrap();
            }
        }
        Ok(())
    }).unwrap();
    assert_eq!(&*events.borrow(), &[true, false, true, false, true, false]);
    assert_eq!(calls.get(), 3);
    assert_eq!(links, 2);
    assert_ne!(observed.length, before.length);
    assert_eq!((observed.volume_serial, observed.file_index, observed.creation_time),
        (before.volume_serial, before.file_index, before.creation_time));
    let metadata = file.metadata().unwrap();
    assert_eq!(observed.length, metadata.file_size());
    assert_eq!(observed.attributes, metadata.file_attributes());
    assert_eq!(observed.creation_time, metadata.creation_time());
    assert_eq!(observed.last_write_time, metadata.last_write_time());
    assert_eq!(observe(&file).unwrap(), observed, "generic observation remains byte-for-byte equivalent");
    assert_eq!(hardlink_count(&file).unwrap(), links);
    assert_eq!(file.stream_position().unwrap(), position);
    fs::remove_file(alias).unwrap();
    assert_eq!(observe_with_links_with_check(&file, &|_| Ok(())).unwrap(), (observed, 1));
}

#[test]
fn coherent_callback_refusal_never_returns_a_tuple() {
    let scratch = Scratch::new();
    let path = scratch.0.join("original");
    fs::write(&path, b"unchanged").unwrap();
    let file = File::open(path).unwrap();
    for refused in 0..6 {
        let callbacks = Cell::new(0);
        let observed = observe_with_links_with_check(&file, &|_| {
            let index = callbacks.get();
            callbacks.set(index + 1);
            if index == refused { Err(ObservationError::ObservationFailed) } else { Ok(()) }
        });
        assert_eq!(observed, Err(ObservationError::ObservationFailed));
        assert_eq!(callbacks.get(), refused + 1);
    }
}
