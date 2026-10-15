//! Component proofs only; no native-owner admission or source-read grant.
#[cfg(windows)]
mod ntfs {
    use super::super::*;
    use std::cell::Cell;
    use std::fs::{self, OpenOptions};
    use std::io::{Read, Seek};
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("eliot-child-membership-{}-{stamp}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self, relative: &str, bytes: &[u8]) -> File {
            let path = self.0.join(relative);
            fs::write(&path, bytes).unwrap();
            File::open(path).unwrap()
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            assert!(self.0.is_absolute());
            assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
            assert!(self.0.file_name().unwrap().to_str().unwrap().starts_with("eliot-child-membership-"));
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn directory(path: &Path) -> File {
        OpenOptions::new().access_mode(0x81) // FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES
            .share_mode(7).custom_flags(0x0220_0000).open(path).unwrap()
    }
    fn verify(parent: &File, child: &File) -> Result<Observation, ObservationError> {
        verify_direct_file_child_with_check(parent, child, &mut ChildMembershipScratch::new(),
            CHILD_MEMBERSHIP_MAX_ENTRIES, &|_| Ok(()), &|_| Ok(()))
    }

    #[test]
    fn child_membership_ntfs_direct_child_continues_and_restarts_same_parent() {
        let scratch = Scratch::new();
        // Enough real entries to require continuation beyond the 64-KiB buffer.
        for index in 0..300 {
            let path = scratch.0.join(format!("padding-{index:04}-{}", "x".repeat(120)));
            fs::write(path, []).unwrap();
        }
        let mut child = scratch.file("actual-source", b"source bytes");
        let parent = directory(&scratch.0);
        let before = observe(&child).expect("real NTFS component lane");
        let position = child.stream_position().unwrap();
        let processed = Cell::new(0_usize);
        let initialized = Cell::new(0);
        let cpu_polls = Cell::new(0);
        let native_admissions = Cell::new(0);
        let native_completions = Cell::new(0);
        let mut buffer = ChildMembershipScratch::new();
        assert_eq!(buffer.bytes.as_ptr().addr() % 8, 0);
        for _ in 0..2 {
            assert_eq!(verify_direct_file_child_with_check(&parent, &child, &mut buffer,
                CHILD_MEMBERSHIP_MAX_ENTRIES, &|before| {
                    if before { native_admissions.set(native_admissions.get() + 1); }
                    else { native_completions.set(native_completions.get() + 1); }
                    Ok(())
                }, &|bytes| {
                    if bytes == CHILD_MEMBERSHIP_BUFFER_BYTES { initialized.set(initialized.get() + 1); }
                    if bytes == 0 { cpu_polls.set(cpu_polls.get() + 1); }
                    processed.set(processed.get() + bytes);
                    Ok(())
                }), Ok(before));
        }
        assert!(initialized.get() >= 6, "two batches and EOF in each independent restart");
        assert!(processed.get() > initialized.get() * CHILD_MEMBERSHIP_BUFFER_BYTES);
        assert!(cpu_polls.get() >= 600, "every real record is polled without native effects");
        assert!(native_admissions.get() < 64, "record polls must not become per-record native checks");
        assert_eq!(native_admissions.get(), native_completions.get(), "every actual ABI keeps both checks");
        assert_eq!(child.stream_position().unwrap(), position);
        let mut body = String::new();
        child.read_to_string(&mut body).unwrap();
        assert_eq!(body, "source bytes");
        assert_eq!(verify_direct_file_child_with_check(&parent, &child, &mut buffer,
            1, &|_| Ok(()), &|_| Ok(())), Err(ObservationError::DirectoryEntryLimit));
        assert!(verify(&parent, &parent).is_err(), "self/dot cannot issue a regular-file membership");
    }

    #[test]
    fn child_membership_ntfs_foreign_equal_bytes_and_wrong_handle_kinds_are_rejected() {
        let scratch = Scratch::new();
        fs::create_dir(scratch.0.join("intended")).unwrap();
        fs::create_dir(scratch.0.join("foreign")).unwrap();
        let local = scratch.file("intended/source", b"same bytes");
        let foreign = scratch.file("foreign/source", b"same bytes");
        let parent = directory(&scratch.0.join("intended"));
        assert_ne!(observe(&local).unwrap().file_index, observe(&foreign).unwrap().file_index);
        assert_eq!(verify(&parent, &foreign), Err(ObservationError::NotDirectChild));
        assert_eq!(verify(&local, &foreign), Err(ObservationError::UnsupportedHandle));
        let foreign_directory = directory(&scratch.0.join("foreign"));
        assert_eq!(verify(&parent, &foreign_directory), Err(ObservationError::UnsupportedHandle));
    }

    #[test]
    fn child_membership_ntfs_directory_chain_and_foreign_directory_binding() {
        let scratch = Scratch::new();
        let nested = scratch.0.join("preparation/objects/ab");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir(scratch.0.join("foreign")).unwrap();
        let handles = [directory(&scratch.0), directory(&scratch.0.join("preparation")),
            directory(&scratch.0.join("preparation/objects")), directory(&nested)];
        let mut buffer = ChildMembershipScratch::new();
        for pair in handles.windows(2) {
            let expected = observe(&pair[1]).unwrap();
            assert_eq!(verify_direct_directory_child_with_check(&pair[0], &pair[1], &mut buffer,
                CHILD_MEMBERSHIP_MAX_ENTRIES, &|_| Ok(()), &|_| Ok(())), Ok(expected));
        }
        let foreign = directory(&scratch.0.join("foreign"));
        assert_eq!(verify_direct_directory_child_with_check(&handles[2], &foreign, &mut buffer,
            CHILD_MEMBERSHIP_MAX_ENTRIES, &|_| Ok(()), &|_| Ok(())), Err(ObservationError::NotDirectChild));
        assert!(verify_direct_directory_child_with_check(&handles[0], &handles[0], &mut buffer,
            CHILD_MEMBERSHIP_MAX_ENTRIES, &|_| Ok(()), &|_| Ok(())).is_err());
        assert_eq!(verify_direct_directory_child_with_check(&handles[2], &handles[1], &mut buffer,
            CHILD_MEMBERSHIP_MAX_ENTRIES, &|_| Ok(()), &|_| Ok(())), Err(ObservationError::NotDirectChild),
            "the native dotdot entry cannot bind an ancestor as a child");
    }

    #[test]
    fn child_membership_ntfs_reparse_child_is_rejected() {
        let scratch = Scratch::new();
        let target = scratch.0.join("target");
        fs::create_dir(&target).unwrap();
        let link = scratch.0.join("junction");
        // A real NTFS junction needs no symlink privilege. Only the unique
        // fixture is changed; neither the helper nor product launches a process.
        let result = std::process::Command::new("cmd.exe")
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .args(["/d", "/c", "mklink", "/J"]).arg(&link).arg(&target).output().unwrap();
        assert!(result.status.success(), "real reparse fixture must be created");
        let parent = directory(&scratch.0);
        let child = OpenOptions::new().access_mode(0x80).custom_flags(0x0220_0000)
            .open(link).unwrap();
        assert_eq!(verify(&parent, &child), Err(ObservationError::ReparsePointDenied));
    }

    #[test]
    fn child_membership_ntfs_original_callback_refusal_never_returns_proof() {
        let scratch = Scratch::new();
        let child = scratch.file("source", b"actual bytes");
        let parent = directory(&scratch.0);
        for after_query in [false, true] {
            let admitted = Cell::new(0);
            let check = |before| {
                if before { admitted.set(admitted.get() + 1); }
                if (!after_query && before && admitted.get() == 7)
                    || (after_query && !before && admitted.get() == 8)
                { return Err(ObservationError::DirectoryEntryLimit); }
                Ok(())
            };
            assert_eq!(verify_direct_file_child_with_check(&parent, &child,
                &mut ChildMembershipScratch::new(), CHILD_MEMBERSHIP_MAX_ENTRIES, &check, &|_| Ok(())),
                Err(ObservationError::DirectoryEntryLimit));
            assert_eq!(admitted.get(), if after_query { 8 } else { 7 });
        }
        assert_eq!(verify_direct_file_child_with_check(&parent, &child,
            &mut ChildMembershipScratch::new(), CHILD_MEMBERSHIP_MAX_ENTRIES, &|_| Ok(()),
            &|_| Err(ObservationError::ObservationFailed)), Err(ObservationError::ObservationFailed));
        let cpu_polls = Cell::new(0);
        let native_admissions = Cell::new(0);
        assert_eq!(verify_direct_file_child_with_check(&parent, &child,
            &mut ChildMembershipScratch::new(), CHILD_MEMBERSHIP_MAX_ENTRIES, &|before| {
                if before { native_admissions.set(native_admissions.get() + 1); }
                Ok(())
            }, &|bytes| {
                if bytes == 0 { cpu_polls.set(cpu_polls.get() + 1); }
                if cpu_polls.get() == 2 { return Err(ObservationError::DirectoryEntryLimit); }
                Ok(())
            }), Err(ObservationError::DirectoryEntryLimit));
        assert_eq!(native_admissions.get(), 8, "CPU parser refusal adds no native effects");
    }

    #[test]
    fn child_membership_ntfs_post_callback_cannot_overwrite_query_exhaustion_error() {
        let scratch = Scratch::new();
        let child = scratch.file("source", b"actual bytes");
        let parent = directory(&scratch.0);
        let expected = observe(&child).unwrap();
        let missing = scratch.0.join("absent-checkpoint-observation");
        let overwritten = Cell::new(0);
        let check = |before: bool| {
            if !before {
                // Model a real checkpoint's OS observation. Its FILE_NOT_FOUND
                // must not replace the preceding directory query's NO_MORE_FILES.
                let error = fs::metadata(&missing).unwrap_err();
                assert_eq!(error.raw_os_error(), Some(2));
                assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(2));
                overwritten.set(overwritten.get() + 1);
            }
            Ok(())
        };
        let mut buffer = ChildMembershipScratch::new();
        for _ in 0..2 {
            assert_eq!(verify_direct_file_child_with_check(&parent, &child, &mut buffer,
                CHILD_MEMBERSHIP_MAX_ENTRIES, &check, &|_| Ok(())), Ok(expected));
        }
        assert!(overwritten.get() > 0);
    }

    #[test]
    fn child_membership_ntfs_containment_does_not_supply_data_read_access() {
        let scratch = Scratch::new();
        let original = scratch.file("source", b"actual bytes");
        let parent = directory(&scratch.0);
        let mut attributes_only = OpenOptions::new().access_mode(0x80)
            .share_mode(7).open(scratch.0.join("source")).unwrap();
        assert_eq!(verify(&parent, &attributes_only), Ok(observe(&original).unwrap()));
        let error = attributes_only.read(&mut [0_u8; 1]).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(error.raw_os_error(), Some(5));
    }

    #[test]
    fn child_membership_checked_parser_rejects_malformed_frames_and_dot_matches() {
        let scratch = Scratch::new();
        let child = scratch.file("source", b"actual bytes");
        let observed = observe(&child).unwrap();
        let frame = |buffer: &mut ChildMembershipScratch| {
            buffer.bytes.fill(0);
            for (at, value) in [(8, observed.creation_time), (24, observed.last_write_time),
                (40, observed.length), (96, observed.file_index)] {
                buffer.bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
            }
            buffer.bytes[56..60].copy_from_slice(&observed.attributes.to_le_bytes());
            buffer.bytes[60..64].copy_from_slice(&8_u32.to_le_bytes());
            buffer.bytes[104..112].copy_from_slice(&[108, 0, 101, 0, 97, 0, 102, 0]);
        };
        for (at, value) in [(0, 107_u32), (0, 16), (0, 65_536), (60, 3), (60, 0), (60, u32::MAX)] {
            let mut buffer = ChildMembershipScratch::new();
            frame(&mut buffer);
            buffer.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert_eq!(directory_batch_matches(&buffer.bytes, observed, &mut 0, 8, &|_| Ok(())),
                Err(ObservationError::InvalidDirectoryInformation));
        }
        let mut buffer = ChildMembershipScratch::new();
        frame(&mut buffer);
        buffer.bytes[68] = 25;
        assert!(directory_batch_matches(&buffer.bytes, observed, &mut 0, 8, &|_| Ok(())).is_err());
        frame(&mut buffer);
        assert!(directory_batch_matches(&buffer.bytes[..103], observed, &mut 0, 8, &|_| Ok(())).is_err());
        frame(&mut buffer);
        assert!(directory_batch_matches(&buffer.bytes[1..], observed, &mut 0, 8, &|_| Ok(())).is_err());
        for name in [&[46, 0][..], &[46, 0, 46, 0][..]] {
            frame(&mut buffer);
            buffer.bytes[60..64].copy_from_slice(&u32::try_from(name.len()).unwrap().to_le_bytes());
            buffer.bytes[104..104 + name.len()].copy_from_slice(name);
            assert_eq!(directory_batch_matches(&buffer.bytes, observed, &mut 0, 8, &|_| Ok(())), Ok(false));
        }
    }
}
