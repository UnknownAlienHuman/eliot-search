//! Deferred native source fixtures; authored, not compiled or executed.

use super::*;
use std::ffi::OsString;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn limits() -> super::super::native_request::NativeRootLimits {
    super::super::native_request::NativeRootLimits::new([
        256,
        256 * 1024,
        512 * 1024,
        1024 * 1024 * 1024,
        16384,
        256,
    ])
    .unwrap()
}

fn request(
    root: &std::path::Path,
    mode: DataRootOpenMode,
    flag: Arc<AtomicBool>,
) -> NativeRootRequest {
    NativeRootRequest::new(
        mode,
        &[
            OsString::from("native-fixture"),
            root.as_os_str().to_owned(),
        ],
        Instant::now() + Duration::from_secs(120),
        flag,
        limits(),
    )
    .unwrap()
}

struct Scratch(PathBuf);

impl Scratch {
    fn empty() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("search-native-root-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(&path).unwrap())
    }

    #[cfg(windows)]
    fn released() -> Self {
        use super::super::codec::hex;
        let scratch = Self::empty();
        for name in ["control", "revisions"] {
            fs::create_dir(scratch.0.join(name)).unwrap();
        }
        for name in [
            ".eliot-search-owner.lock",
            ".eliot-search-sealed-owner.lock",
            "control/namespace.id",
            "control/source-events.log",
        ] {
            fs::write(scratch.0.join(name), b"").unwrap();
        }
        // Setup alone uses the retained legacy producer; admission never does.
        let mut owner = super::super::succession::establish(&scratch.0).unwrap();
        owner.begin_drain(DrainReason::Shutdown).unwrap();
        owner.release_cleanly().unwrap();
        let binding = super::super::installation::load_or_create_installation(&scratch.0).unwrap();
        let req = request(
            &scratch.0,
            DataRootOpenMode::InspectExistingReadOnly,
            Arc::new(AtomicBool::new(false)),
        );
        let mut work = NativeWork::new();
        let canonical = fs::canonicalize(&scratch.0).unwrap();
        let pins = NativePins::acquire(&req, &mut work, &canonical, false).unwrap();
        let digest = pins.digest(&req, &mut work).unwrap();
        drop(pins);
        let text = format!(
            "ELIOT-SEARCH-INSTALLATION-V1\nformat_version=2\ninstallation_id={}\ninstallation_incarnation_id={}\ninitialization_id={}\nlayout_profile=legacy-direct-root-v2\nnative_objects_digest={}\n",
            hex(&binding.installation_id),
            hex(&binding.installation_incarnation_id),
            hex(&[2; 16]),
            hex(&digest)
        );
        fs::write(scratch.0.join(INSTALLATION_FILE), text).unwrap();
        scratch
    }

    #[cfg(windows)]
    fn durable_inventory(&self) -> Vec<(Vec<u8>, std::time::SystemTime)> {
        super::super::native_objects::OBJECTS
            .iter()
            .filter(|(_, directory)| !directory)
            .map(|(name, _)| {
                let path = self.0.join(name);
                (
                    fs::read(&path).unwrap(),
                    fs::metadata(path).unwrap().modified().unwrap(),
                )
            })
            .collect()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn original_request_survives_argv_drop_and_terminal_cancellation_is_shared() {
    let flag = Arc::new(AtomicBool::new(false));
    let mut args = vec![
        OsString::from("native-fixture"),
        OsString::from("private-root"),
    ];
    let deadline = Instant::now() + Duration::from_secs(120);
    let req = NativeRootRequest::new(
        DataRootOpenMode::InspectExistingReadOnly,
        &args,
        deadline,
        Arc::clone(&flag),
        limits(),
    )
    .unwrap();
    args[1] = OsString::from("different-root");
    drop(args);
    assert_eq!(req.root(), std::path::Path::new("private-root"));
    assert_eq!(format!("{req:?}"), "NativeRootRequest(<opaque>)");
    struct CancellingWriter {
        flag: Arc<AtomicBool>,
        writes: usize,
        flushes: usize,
    }
    impl Write for CancellingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.writes += 1;
            self.flag.store(true, Ordering::Release);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }
    let mut writer = CancellingWriter {
        flag,
        writes: 0,
        flushes: 0,
    };
    assert_eq!(
        req.emit(&mut writer, "bounded"),
        Err(OwnerError::OwnerAcquireOutcomeUnknown)
    );
    assert_eq!((writer.writes, writer.flushes), (1, 0));
}

#[test]
fn native_argument_and_cumulative_call_lower_limits_refuse_before_effect() {
    use super::super::native_request::{NativeRootLimits, native_argument};
    let value = OsString::from("abcd");
    let bytes = native_argument(&value, 8).unwrap();
    assert!(native_argument(&value, bytes.len() - 1).is_err());
    let lower = NativeRootLimits::new([2, 1024, 2048, 1024, 2, 1]).unwrap();
    let req = NativeRootRequest::new(
        DataRootOpenMode::InspectExistingReadOnly,
        &[OsString::from("fixture"), OsString::from("root")],
        Instant::now() + Duration::from_secs(120),
        Arc::new(AtomicBool::new(false)),
        lower,
    )
    .unwrap();
    let mut work = NativeWork::new();
    work.call(&req, || Ok(())).unwrap();
    let mut effect = false;
    assert_eq!(
        work.call(&req, || {
            effect = true;
            Ok(())
        }),
        Err(OwnerError::ContractExhausted)
    );
    assert!(!effect);
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        let surrogate = OsString::from_wide(&[0xD800, 0x78]);
        assert_eq!(
            native_argument(&surrogate, 4).unwrap(),
            vec![0, 0xD8, 0x78, 0]
        );
    }
}

#[cfg(windows)]
#[test]
fn missing_and_empty_inspection_create_nothing() {
    let root = Scratch::empty();
    let missing = root.0.join("missing");
    for path in [&missing, &root.0] {
        let req = request(
            path,
            DataRootOpenMode::InspectExistingReadOnly,
            Arc::new(AtomicBool::new(false)),
        );
        assert!(matches!(
            admit_existing(req).unwrap(),
            NativeRootAdmission::Unavailable(RootOpenState::Absent, _)
        ));
    }
    assert!(!missing.exists());
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn valid_native_inspection_is_write_neutral_and_coholds_both_exclusions() {
    let root = Scratch::released();
    let before = root.durable_inventory();
    let req = request(
        &root.0,
        DataRootOpenMode::InspectExistingReadOnly,
        Arc::new(AtomicBool::new(false)),
    );
    let NativeRootAdmission::Inspection(mut cap) = admit_existing(req).unwrap() else {
        panic!("inspection")
    };
    let metadata = cap.inspect().unwrap();
    assert_eq!(metadata.owner_epoch.get(), 1);
    let other = request(
        &root.0,
        DataRootOpenMode::OpenExistingMutating,
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        admit_existing(other),
        Err(OwnerError::DataRootAlreadyOwned)
    ));
    assert_eq!(
        cap.begin_drain(DrainReason::Shutdown),
        Err(OwnerError::OwnerInvalidTransition)
    );
    assert_eq!(format!("{cap:?}"), "NativeRootCapability(<opaque>)");
    cap.emit(&mut Vec::new(), "bounded").unwrap();
    drop(cap);
    assert_eq!(root.durable_inventory(), before);
}

#[cfg(windows)]
#[test]
fn corrupt_peer_and_quarantine_or_redb_refuse_without_repair() {
    use super::super::spec::OWNER_SLOT_B;
    for barrier in ["control/catalog-quarantine.marker", "control/control.redb"] {
        let root = Scratch::released();
        fs::write(root.0.join(barrier), b"retained").unwrap();
        let before = root.durable_inventory();
        let req = request(
            &root.0,
            DataRootOpenMode::InspectExistingReadOnly,
            Arc::new(AtomicBool::new(false)),
        );
        assert!(matches!(
            admit_existing(req).unwrap(),
            NativeRootAdmission::Unavailable(_, _)
        ));
        assert_eq!(fs::read(root.0.join(barrier)).unwrap(), b"retained");
        assert_eq!(root.durable_inventory(), before);
    }
    let root = Scratch::released();
    let req = request(
        &root.0,
        DataRootOpenMode::OpenExistingMutating,
        Arc::new(AtomicBool::new(false)),
    );
    let NativeRootAdmission::Mutation(cap) = admit_existing(req).unwrap() else {
        panic!("mutation")
    };
    drop(cap); // A remains older RELEASED; B is the newer unresolved ACTIVE.
    fs::write(root.0.join(OWNER_SLOT_B), b"corrupt-newer").unwrap();
    let before = root.durable_inventory();
    let req = request(
        &root.0,
        DataRootOpenMode::InspectExistingReadOnly,
        Arc::new(AtomicBool::new(false)),
    );
    assert!(admit_existing(req).is_err());
    assert_eq!(root.durable_inventory(), before);
}

#[cfg(windows)]
#[test]
fn phase_cancellation_before_publication_is_neutral_and_after_effect_is_unknown() {
    for phase in ["native-proof", "before-owner-write", "after-owner-write"] {
        let root = Scratch::released();
        let before = root.durable_inventory();
        let flag = Arc::new(AtomicBool::new(false));
        let hook_flag = Arc::clone(&flag);
        let mut work = NativeWork::new();
        work.phase_hook = Some(Box::new(move |arrived| {
            if arrived == phase {
                hook_flag.store(true, Ordering::Release);
            }
        }));
        let req = request(&root.0, DataRootOpenMode::OpenExistingMutating, flag);
        let result = admit_with_work(req, work);
        if phase == "after-owner-write" {
            assert!(matches!(
                result,
                Err(OwnerError::OwnerAcquireOutcomeUnknown)
            ));
            assert_ne!(root.durable_inventory(), before);
        } else {
            assert!(matches!(
                result,
                Err(OwnerError::OwnerCancelledBeforeMutation)
            ));
            assert_eq!(root.durable_inventory(), before);
        }
    }
}

#[cfg(windows)]
#[test]
fn admission_terminal_cancellation_cannot_return_capability() {
    for mode in [
        DataRootOpenMode::InspectExistingReadOnly,
        DataRootOpenMode::OpenExistingMutating,
    ] {
        let root = Scratch::released();
        let before = root.durable_inventory();
        let flag = Arc::new(AtomicBool::new(false));
        let hook_flag = Arc::clone(&flag);
        let mut work = NativeWork::new();
        work.phase_hook = Some(Box::new(move |phase| {
            if phase == "admission-terminal" {
                hook_flag.store(true, Ordering::Release);
            }
        }));
        let result = admit_with_work(request(&root.0, mode, flag), work);
        if mode == DataRootOpenMode::InspectExistingReadOnly {
            assert!(matches!(
                result,
                Err(OwnerError::OwnerCancelledBeforeMutation)
            ));
            assert_eq!(root.durable_inventory(), before);
        } else {
            assert!(matches!(
                result,
                Err(OwnerError::OwnerAcquireOutcomeUnknown)
            ));
            assert_ne!(root.durable_inventory(), before);
        }
    }
}

#[cfg(windows)]
#[test]
fn later_lifecycle_cancellation_retains_prior_effect_and_poisoned_outcome() {
    for (release, phase, expected) in [
        (
            false,
            "before-owner-write",
            OwnerError::OwnerAcquireOutcomeUnknown,
        ),
        (
            true,
            "before-owner-write",
            OwnerError::OwnerAcquireOutcomeUnknown,
        ),
        (
            true,
            "after-owner-write",
            OwnerError::OwnerReleaseOutcomeUnknown,
        ),
    ] {
        let root = Scratch::released();
        let flag = Arc::new(AtomicBool::new(false));
        let req = request(
            &root.0,
            DataRootOpenMode::OpenExistingMutating,
            Arc::clone(&flag),
        );
        let NativeRootAdmission::Mutation(mut cap) = admit_existing(req).unwrap() else {
            panic!("mutation")
        };
        if release {
            cap.begin_drain(DrainReason::Shutdown).unwrap();
        }
        let before = root.durable_inventory();
        let hook_flag = Arc::clone(&flag);
        cap.work.phase_hook = Some(Box::new(move |arrived| {
            if arrived == phase {
                hook_flag.store(true, Ordering::Release);
            }
        }));
        let result = if release {
            cap.release_cleanly().map(|_| ())
        } else {
            cap.begin_drain(DrainReason::Shutdown)
        };
        assert_eq!(result, Err(expected));
        let after = root.durable_inventory();
        if phase == "after-owner-write" {
            assert_ne!(after, before);
        } else {
            assert_eq!(after, before);
        }
        cap.work.phase_hook = None;
        flag.store(false, Ordering::Release);
        assert!(matches!(cap.inspect(), Err(error) if error == expected));
        drop(cap);
        assert_eq!(root.durable_inventory(), after);
    }
}

#[cfg(windows)]
#[test]
fn terminal_phase_refuses_ack_and_drop_never_publishes_release() {
    let root = Scratch::released();
    let flag = Arc::new(AtomicBool::new(false));
    let req = request(
        &root.0,
        DataRootOpenMode::OpenExistingMutating,
        Arc::clone(&flag),
    );
    let NativeRootAdmission::Mutation(mut cap) = admit_existing(req).unwrap() else {
        panic!("mutation")
    };
    let active = root.durable_inventory();
    cap.work.phase_hook = Some(Box::new(move |phase| {
        if phase == "terminal-emission" {
            flag.store(true, Ordering::Release);
        }
    }));
    let mut output = Vec::new();
    assert_eq!(
        cap.emit(&mut output, "bounded"),
        Err(OwnerError::OwnerAcquireOutcomeUnknown)
    );
    assert!(output.is_empty());
    drop(cap);
    assert_eq!(root.durable_inventory(), active);
    let req = request(
        &root.0,
        DataRootOpenMode::InspectExistingReadOnly,
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(
        admit_existing(req).unwrap(),
        NativeRootAdmission::Unavailable(RootOpenState::OutcomeUnknown, _)
    ));
}
