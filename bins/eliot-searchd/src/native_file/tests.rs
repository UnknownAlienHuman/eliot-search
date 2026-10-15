use super::*;

#[test]
fn legacy_identity_encoding_preserves_both_words_and_byte_order() {
    let observation = Observation {
        volume_serial: 0x0123_4567,
        file_index: 0x89ab_cdef_0123_4567,
        attributes: 0,
        length: 0,
        creation_time: 0,
        last_write_time: 0,
    };
    assert_eq!(
        observation.legacy_identity_bytes(),
        [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67
        ],
    );
}

#[test]
fn closed_errors_do_not_contain_source_or_native_error_text() {
    for (error, expected) in [
        (
            ObservationError::UnsupportedPlatform,
            "NATIVE_FILE_UNSUPPORTED_PLATFORM",
        ),
        (
            ObservationError::UnsupportedHandle,
            "NATIVE_FILE_UNSUPPORTED_HANDLE",
        ),
        (
            ObservationError::UnsupportedFileSystem,
            "NATIVE_FILE_UNSUPPORTED_FILESYSTEM",
        ),
        (
            ObservationError::ObservationFailed,
            "NATIVE_FILE_OBSERVATION_FAILED",
        ),
        (
            ObservationError::ReparsePointDenied,
            "NATIVE_FILE_REPARSE_POINT_DENIED",
        ),
    ] {
        assert_eq!(error.to_string(), expected);
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::io::{Read, Seek};
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::PathBuf;
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
            let path = std::env::temp_dir().join(format!(
                "eliot-native-file-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn create(&self, name: &str, bytes: &[u8]) -> File {
            let path = self.0.join(name);
            fs::write(&path, bytes).unwrap();
            File::open(path).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn observation_borrows_handle_without_reading_or_repositioning_it() {
        let scratch = Scratch::new();
        let mut file = scratch.create("source", b"exact source");
        let before_position = file.stream_position().unwrap();
        let before = observe(&file).expect("native NTFS test lane required");
        assert_eq!(file.stream_position().unwrap(), before_position);
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"exact source");
        assert_eq!(observe(&file).unwrap(), before);
        assert_eq!(before.length, 12);
    }

    #[test]
    fn distinct_files_have_distinct_identity_even_with_equal_content() {
        let scratch = Scratch::new();
        let first = scratch.create("first", b"same");
        let second = scratch.create("second", b"same");
        assert_ne!(
            observe(&first).unwrap().legacy_identity_bytes(),
            observe(&second).unwrap().legacy_identity_bytes(),
        );
    }

    #[test]
    fn rename_and_hardlink_keep_the_observed_ntfs_identity() {
        let scratch = Scratch::new();
        let original = scratch.create("original", b"same");
        let identity = observe(&original).unwrap().legacy_identity_bytes();
        let renamed = scratch.0.join("renamed");
        let alias = scratch.0.join("alias");
        fs::rename(scratch.0.join("original"), &renamed).unwrap();
        fs::hard_link(&renamed, &alias).unwrap();
        assert_eq!(
            observe(&original).unwrap().legacy_identity_bytes(),
            identity
        );
        for path in [&renamed, &alias] {
            let file = File::open(path).unwrap();
            assert_eq!(observe(&file).unwrap().legacy_identity_bytes(), identity);
        }
    }

    #[test]
    fn replacing_a_locator_does_not_change_identity_of_the_already_open_object() {
        let scratch = Scratch::new();
        let retained = scratch.create("source", b"first");
        let identity = observe(&retained).unwrap().legacy_identity_bytes();
        fs::rename(scratch.0.join("source"), scratch.0.join("old")).unwrap();
        let replacement = scratch.create("source", b"second");
        assert_eq!(
            observe(&retained).unwrap().legacy_identity_bytes(),
            identity
        );
        assert_ne!(
            observe(&replacement).unwrap().legacy_identity_bytes(),
            identity
        );
    }

    #[test]
    fn native_directory_identity_does_not_require_file_contents() {
        let scratch = Scratch::new();
        let directory = OpenOptions::new()
            .access_mode(0x80) // FILE_READ_ATTRIBUTES
            .custom_flags(0x0200_0000 | 0x0020_0000)
            .open(&scratch.0)
            .unwrap();
        let observation = observe(&directory).unwrap();
        assert_ne!(observation.attributes & 0x10, 0); // FILE_ATTRIBUTE_DIRECTORY
        assert_eq!(observe(&directory).unwrap(), observation);
    }

    #[test]
    fn character_device_cannot_supply_a_synthetic_disk_identity() {
        let device = File::open("NUL").unwrap();
        assert_eq!(observe(&device), Err(ObservationError::UnsupportedHandle));
    }

    fn qualified_directory(path: &std::path::Path) -> Arc<File> {
        Arc::new(
            OpenOptions::new()
                .access_mode(0x80)
                .share_mode(7)
                .custom_flags(0x0220_0000)
                .open(path)
                .unwrap(),
        )
    }

    #[test]
    fn qualified_directory_one_abi_keeps_creation_time_and_attributes_live() {
        use std::cell::RefCell;
        use std::fs::FileTimes;
        use std::os::windows::fs::FileTimesExt;
        use std::time::Duration;

        let scratch = Scratch::new();
        let file = qualified_directory(&scratch.0);
        let trace = RefCell::new(Vec::new());
        let check = |before| {
            trace.borrow_mut().push(before);
            Ok(())
        };
        let (qualified, initial) =
            QualifiedDirectory::qualify_with_check(Arc::clone(&file), &check)
                .unwrap()
                .expect("actual NTFS directory");
        assert_eq!(&*trace.borrow(), &[true, false, true, false, true, false]);
        assert!(Arc::ptr_eq(&file, &qualified.retain_file()));
        trace.borrow_mut().clear();
        let fresh = qualified.identity_with_check(&check).unwrap();
        assert_eq!(&*trace.borrow(), &[true, false]);
        assert_eq!(
            (
                fresh.volume_serial,
                fresh.file_index,
                fresh.creation_time,
                fresh.attributes
            ),
            (
                initial.volume_serial,
                initial.file_index,
                initial.creation_time,
                initial.attributes
            )
        );

        let permissions = fs::metadata(&scratch.0).unwrap().permissions();
        let mut toggled = permissions.clone();
        toggled.set_readonly(!permissions.readonly());
        fs::set_permissions(&scratch.0, toggled).unwrap();
        let changed_attributes = qualified.identity_with_check(&|_| Ok(())).unwrap();
        fs::set_permissions(&scratch.0, permissions).unwrap();
        assert_ne!(
            changed_attributes.attributes & 1,
            initial.attributes & 1,
            "attributes must be queried again on the retained FD"
        );

        let writer = OpenOptions::new()
            .access_mode(0x100)
            .share_mode(7)
            .custom_flags(0x0220_0000)
            .open(&scratch.0)
            .unwrap();
        let original = writer.metadata().unwrap().created().unwrap();
        writer
            .set_times(FileTimes::new().set_created(original + Duration::from_secs(2)))
            .unwrap();
        let changed = qualified.identity_with_check(&|_| Ok(())).unwrap();
        writer
            .set_times(FileTimes::new().set_created(original))
            .unwrap();
        assert_eq!(changed.file_index, initial.file_index);
        assert_ne!(
            changed.creation_time, initial.creation_time,
            "creation time must be freshly observed"
        );
        assert_eq!(
            qualified
                .identity_with_check(&|_| Ok(()))
                .unwrap()
                .creation_time,
            initial.creation_time
        );
        assert_eq!(
            super::super::platform::qualified_directory_identity_with_check(
                qualified.file(),
                initial.volume_serial ^ 1,
                &|_| Ok(())
            ),
            Err(ObservationError::ObservationFailed)
        );
    }

    #[test]
    fn qualified_directory_refuses_other_kinds_and_cancelled_recheck() {
        use std::cell::RefCell;
        use std::sync::atomic::AtomicBool;

        let scratch = Scratch::new();
        assert!(
            QualifiedDirectory::qualify_with_check(
                Arc::new(scratch.create("file", b"body")),
                &|_| Ok(())
            )
            .unwrap()
            .is_none()
        );
        assert!(matches!(
            QualifiedDirectory::qualify_with_check(Arc::new(File::open("NUL").unwrap()), &|_| Ok(
                ()
            )),
            Err(ObservationError::UnsupportedHandle)
        ));
        let (qualified, _) =
            QualifiedDirectory::qualify_with_check(qualified_directory(&scratch.0), &|_| Ok(()))
                .unwrap()
                .unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let trace = RefCell::new(Vec::new());
        let check = |before| {
            trace.borrow_mut().push(before);
            if before {
                cancelled.store(true, Ordering::Release);
            }
            if cancelled.load(Ordering::Acquire) {
                Err(ObservationError::ObservationFailed)
            } else {
                Ok(())
            }
        };
        assert_eq!(
            qualified.identity_with_check(&check),
            Err(ObservationError::ObservationFailed)
        );
        assert_eq!(
            &*trace.borrow(),
            &[true],
            "refused dispatch must not invoke the ABI or produce a snapshot"
        );
    }

    #[test]
    fn sync_leaf_qualifies_location_and_bounded_held_names() {
        let scratch = Scratch::new();
        let path = scratch.0.join("source");
        fs::write(&path, b"source").unwrap();
        let leaf = SyncReadFile::open_no_follow(&path, &|_| Ok(())).unwrap();
        let before = observe(leaf.file()).unwrap();
        assert!(matches!(
            location_with_check(&leaf, &|_| Ok(())).unwrap(),
            NativeLocationClass::LocalFixed | NativeLocationClass::LocalRemovable
        ));
        let guid = normalized_name_with_check(
            leaf.file(),
            NormalizedNameKind::VolumeGuid,
            &|_| Ok(()),
        )
        .unwrap();
        assert!(guid.starts_with("\\\\?\\Volume{"));
        let dos = normalized_name_with_check(leaf.file(), NormalizedNameKind::Dos, &|_| Ok(()))
            .unwrap();
        assert!(dos.starts_with("\\\\?\\"));
        assert_eq!(observe(leaf.file()).unwrap(), before);
    }

    #[test]
    fn location_refusal_after_native_dispatch_returns_no_classification() {
        use std::cell::RefCell;

        let scratch = Scratch::new();
        let path = scratch.0.join("source");
        fs::write(&path, b"source").unwrap();
        let leaf = SyncReadFile::open_no_follow(&path, &|_| Ok(())).unwrap();
        let trace = RefCell::new(Vec::new());
        let check = |before| {
            trace.borrow_mut().push(before);
            if before { Ok(()) } else { Err(ObservationError::ObservationFailed) }
        };
        assert_eq!(
            location_with_check(&leaf, &check),
            Err(ObservationError::ObservationFailed)
        );
        assert_eq!(&*trace.borrow(), &[true, false]);
    }

    #[test]
    fn case_sensitive_query_requires_a_qualified_directory_handle() {
        let scratch = Scratch::new();
        let directory = qualified_directory(&scratch.0);
        assert!(matches!(
            case_sensitive_directory_with_check(&directory, &|_| Ok(())),
            Ok(true) | Ok(false)
        ));
        let file = scratch.create("source", b"source");
        assert_eq!(
            case_sensitive_directory_with_check(&file, &|_| Ok(())),
            Err(ObservationError::UnsupportedHandle)
        );
    }
}
