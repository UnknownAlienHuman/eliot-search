//! Native initializer cases authored for Q215; NOT_COMPILED/NOT_RUN.
//! Qualification owns retained temporary roots and credentials; no blind key deletion.

use super::super::native_admission::{NativeRootAdmission, admit_existing};
use super::super::native_request::NativeRootLimits;
use super::*;
use std::cell::RefCell;
use std::ffi::OsString;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type Trace = Rc<RefCell<Vec<(&'static str, [usize; 4], usize)>>>;
type Credentials = Rc<RefCell<Vec<(bool, Instant, usize)>>>;

fn scratch() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("search-initializer-{}-{stamp}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::canonicalize(root).unwrap()
}

fn request(
    root: &std::path::Path,
    id: [u8; 16],
    flag: Arc<AtomicBool>,
    deadline: Instant,
    io: usize,
) -> NativeRootRequest {
    NativeRootRequest::new(
        DataRootOpenMode::InitializeNew,
        &[
            OsString::from("--initialize-data-root"),
            root.as_os_str().to_owned(),
            OsString::from(super::super::codec::hex(&id)),
        ],
        deadline,
        flag,
        NativeRootLimits::new([256, 256 * 1024, 512 * 1024, io, 16384, 256]).unwrap(),
    )
    .unwrap()
}

fn observed_work(trace: &Trace, credentials: &Credentials) -> NativeWork {
    let mut work = NativeWork::new();
    let trace = Rc::clone(trace);
    work.usage_hook = Some(Box::new(move |phase, used, requested| {
        trace.borrow_mut().push((phase, used, requested))
    }));
    let credentials = Rc::clone(credentials);
    work.credential_hook = Some(Box::new(move |create, deadline, flag| {
        credentials
            .borrow_mut()
            .push((create, deadline, std::ptr::from_ref(flag) as usize));
    }));
    work
}

fn run(
    root: &std::path::Path,
    id: [u8; 16],
    io: usize,
) -> (Result<(), OwnerError>, Trace, Credentials, Vec<u8>) {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let credentials = Rc::new(RefCell::new(Vec::new()));
    let mut output = Vec::new();
    let req = request(
        root,
        id,
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(120),
        io,
    );
    let result = initialize_with_work(req, observed_work(&trace, &credentials))
        .and_then(|result| result.emit(&mut output));
    (result, trace, credentials, output)
}

fn terminal_io_ceiling(trace: &Trace) -> usize {
    trace
        .borrow()
        .iter()
        .skip_while(|row| row.0 != "initialization-terminal")
        .find(|row| row.0 == "write-reservation")
        .unwrap()
        .1[0]
}

fn inventory(root: &std::path::Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut result = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            for child in fs::read_dir(&path).unwrap() {
                let child = child.unwrap().path();
                result.push((
                    child.strip_prefix(root).unwrap().to_owned(),
                    fs::read(&child).unwrap(),
                ));
            }
        } else {
            result.push((
                path.strip_prefix(root).unwrap().to_owned(),
                fs::read(&path).unwrap(),
            ));
        }
    }
    result.sort_by(|left, right| left.0.cmp(&right.0));
    result
}

#[test]
fn complete_secure_initialization_and_terminal_use_one_cumulative_meter() {
    let root = scratch();
    let (result, trace, credentials, output) = run(&root, [1; 16], 1024 * 1024 * 1024);
    assert_eq!(result, Ok(()));
    assert_eq!(output, b"{\"initialized\":true,\"replayed\":false}\n");
    assert_eq!(credentials.borrow().iter().filter(|row| row.0).count(), 1);
    let trace = trace.borrow();
    let final_usage = trace
        .iter()
        .find(|row| row.0 == "after-initialization-release")
        .unwrap()
        .1;
    assert!(final_usage[2] > 0 && final_usage[2] <= 256);
    assert!(trace.windows(2).all(|pair| {
        pair[0]
            .1
            .iter()
            .zip(pair[1].1)
            .all(|(old, new)| *old <= new)
    }));
    let bytes = fs::read(root.join(JOURNAL_FILE)).unwrap();
    let journal = decode_journal(&bytes, &mut |_| Ok(())).unwrap();
    let seal = journal.completion.unwrap();
    assert_ne!(seal.selected_key_binding, [0; 32]);
    for (index, expected) in seal.native_objects.iter().enumerate() {
        let path = if index == 10 {
            root.join(JOURNAL_FILE)
        } else {
            root.join(OBJECTS[index].0)
        };
        let req = request(
            &root,
            [1; 16],
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(120),
            1024 * 1024 * 1024,
        );
        let file = open_existing(
            &req,
            &mut NativeWork::new(),
            &path,
            matches!(index, 0 | 3 | 4),
            false,
        )
        .unwrap();
        assert_eq!(native_identity(&file).unwrap().as_slice(), expected);
    }
}

#[test]
fn historical_replay_keeps_appended_catalog_and_later_released_epoch() {
    use search_source_registry::{
        LegacyDirectDigest, LegacyDirectIdentityStrength, LegacyDirectRecordDraft,
        LegacyDirectRegistryState, LegacyDirectSourceState,
    };
    struct Digest;
    impl LegacyDirectDigest for Digest {
        fn digest(bytes: &[u8]) -> [u8; 32] {
            crate::sha256::digest(bytes)
        }
        fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
            crate::sha256::digest_parts(domain, parts)
        }
    }
    let root = scratch();
    assert_eq!(run(&root, [2; 16], 1024 * 1024 * 1024).0, Ok(()));
    let journal_before = fs::read(root.join(JOURNAL_FILE)).unwrap();
    let prior = decode_journal(&journal_before, &mut |_| Ok(()))
        .unwrap()
        .subject
        .original_request;
    let source_id = "02".repeat(32);
    let revision_id = crate::sha256::hex(&Digest::digest_parts(
        b"eliot-search/direct-revision-id/v1",
        &[source_id.as_bytes(), &[4; 32], &1_u64.to_be_bytes()],
    ));
    search_source_registry::verify_legacy_direct_revision_identity::<Digest>(
        &source_id,
        &revision_id,
        &"04".repeat(32),
        1,
    )
    .unwrap();
    let draft = LegacyDirectRecordDraft {
        operation_id: "01".repeat(32),
        state: LegacyDirectSourceState::Active,
        source_id,
        revision_id,
        content_digest: "04".repeat(32),
        byte_length: 1,
        file_identity_digest: "05".repeat(32),
        path_digest: "06".repeat(32),
        identity_strength: LegacyDirectIdentityStrength::Native,
    };
    let append = LegacyDirectRegistryState::default()
        .plan_append::<Digest>(vec![draft], 1024, 4096)
        .unwrap();
    assert!(!append.encoded.is_empty());
    let mut log = OpenOptions::new()
        .append(true)
        .open(root.join("control/source-events.log"))
        .unwrap();
    log.write_all(append.encoded.as_bytes()).unwrap();
    log.sync_all().unwrap();
    drop(log);
    let req = NativeRootRequest::new(
        DataRootOpenMode::OpenExistingMutating,
        &[
            OsString::from("--check-existing-root-owner"),
            root.as_os_str().to_owned(),
        ],
        Instant::now() + Duration::from_secs(120),
        Arc::new(AtomicBool::new(false)),
        NativeRootLimits::new([256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256])
            .unwrap(),
    )
    .unwrap();
    let NativeRootAdmission::Mutation(mut owner) = admit_existing(req).unwrap() else {
        panic!("mutation");
    };
    owner
        .begin_drain(search_runtime_owner::DrainReason::Shutdown)
        .unwrap();
    let later = owner.release_cleanly().unwrap();
    assert!(later.epoch.get() > 1);
    owner.emit(&mut Vec::new(), "released").unwrap();
    drop(owner);
    let before = inventory(&root);
    let req = request(
        &root,
        [2; 16],
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(120),
        1024 * 1024 * 1024,
    );
    assert_ne!(*req.binding().0.as_bytes(), prior);
    let trace = Rc::new(RefCell::new(Vec::new()));
    let credentials = Rc::new(RefCell::new(Vec::new()));
    let mut output = Vec::new();
    initialize_with_work(req, observed_work(&trace, &credentials))
        .unwrap()
        .emit(&mut output)
        .unwrap();
    assert_eq!(output, b"{\"initialized\":true,\"replayed\":true}\n");
    assert!(credentials.borrow().iter().all(|row| !row.0));
    assert_eq!(inventory(&root), before);
    assert_eq!(fs::read(root.join(JOURNAL_FILE)).unwrap(), journal_before);
}

#[test]
fn missing_component_and_conflicting_request_do_not_recreate_layout() {
    let root = scratch();
    assert_eq!(run(&root, [3; 16], 1024 * 1024 * 1024).0, Ok(()));
    let before = inventory(&root);
    let (result, _, credentials, output) = run(&root, [4; 16], 1024 * 1024 * 1024);
    assert_eq!(result, Err(OwnerError::OwnerOperationConflict));
    assert!(credentials.borrow().is_empty() && output.is_empty());
    assert_eq!(inventory(&root), before);
    fs::remove_file(root.join("control/namespace.id")).unwrap();
    let before = inventory(&root);
    let (result, _, credentials, output) = run(&root, [3; 16], 1024 * 1024 * 1024);
    assert!(result.is_err());
    assert!(credentials.borrow().is_empty() && output.is_empty());
    assert_eq!(inventory(&root), before);
    assert!(!root.join("control/namespace.id").exists());
}

#[test]
fn retained_native_handles_deny_writes_rename_and_journal_deletion() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = scratch();
    let req = request(
        &root,
        [5; 16],
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(120),
        1024 * 1024 * 1024,
    );
    let result = initialize_new(req).unwrap();
    for path in [&root, &root.join("control"), &root.join("revisions")] {
        let error = OpenOptions::new()
            .read(true)
            .access_mode(0x0001_0080)
            .custom_flags(0x0220_0000)
            .open(path)
            .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(32));
    }
    let error = OpenOptions::new()
        .write(true)
        .open(root.join(JOURNAL_FILE))
        .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(32));
    assert_eq!(
        fs::rename(root.join("control"), root.join("changed-control"))
            .unwrap_err()
            .raw_os_error(),
        Some(32)
    );
    assert_eq!(
        fs::remove_file(root.join(JOURNAL_FILE))
            .unwrap_err()
            .raw_os_error(),
        Some(32)
    );
    result.emit(&mut Vec::new()).unwrap();
    for path in [
        &root,
        &root.join("control"),
        &root.join("revisions"),
        &root.join(JOURNAL_FILE),
    ] {
        drop(
            OpenOptions::new()
                .read(true)
                .access_mode(0x0001_0080)
                .custom_flags(0x0220_0000)
                .open(path)
                .unwrap(),
        );
    }
    drop(
        OpenOptions::new()
            .write(true)
            .open(root.join(JOURNAL_FILE))
            .unwrap(),
    );
    fs::rename(root.join("control"), root.join("changed-control")).unwrap();
    fs::rename(root.join("changed-control"), root.join("control")).unwrap();
}

#[test]
fn foreign_entry_before_journal_reservation_refuses_without_effects() {
    let root = scratch();
    let path = root.join("foreign");
    let injection = path.clone();
    let witnessed = Rc::new(RefCell::new(false));
    let marker = Rc::clone(&witnessed);
    let mut work = NativeWork::new();
    work.phase_hook = Some(Box::new(move |phase| {
        if phase == "before-create-file" && !*marker.borrow() {
            fs::write(&injection, b"foreign").unwrap();
            *marker.borrow_mut() = true;
        }
    }));
    let req = request(
        &root,
        [6; 16],
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(120),
        1024 * 1024 * 1024,
    );
    assert!(matches!(
        initialize_with_work(req, work),
        Err(OwnerError::OwnerGuardMismatch)
    ));
    assert!(*witnessed.borrow());
    assert!(!root.join(JOURNAL_FILE).exists());
    assert_eq!(fs::read(path).unwrap(), b"foreign");
}

#[test]
fn terminal_drop_and_failure_release_children_before_both_exclusions() {
    for kind in 0..3 {
        let root = scratch();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let witness = Rc::clone(&observed);
        let path = root.clone();
        let flag = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::clone(&flag);
        let mut work = NativeWork::new();
        work.phase_hook = Some(Box::new(move |phase| {
            if kind == 2 && phase == "after-credential" {
                cancelled.store(true, Ordering::Release);
            }
            if matches!(
                phase,
                "initialization-children-released" | "initialization-exclusions-released"
            ) {
                let available = [1, 2].map(|index| {
                    let Ok(file) = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(path.join(OBJECTS[index].0))
                    else {
                        return false;
                    };
                    file.try_lock().is_ok()
                });
                witness.borrow_mut().push((phase, available));
            }
        }));
        let req = request(
            &root,
            [10 + kind; 16],
            flag,
            Instant::now() + Duration::from_secs(120),
            1024 * 1024 * 1024,
        );
        let result = initialize_with_work(req, work);
        if kind == 0 {
            result.unwrap().emit(&mut Vec::new()).unwrap();
        } else if kind == 1 {
            drop(result.unwrap());
        } else {
            assert!(matches!(
                result,
                Err(OwnerError::OwnerAcquireOutcomeUnknown)
            ));
        }
        assert_eq!(
            observed.borrow().as_slice(),
            &[
                ("initialization-children-released", [false, false]),
                ("initialization-exclusions-released", [true, true]),
            ]
        );
    }
}

#[test]
fn credential_dispatch_preserves_original_context_and_is_never_repeated() {
    let root = scratch();
    let deadline = Instant::now() + Duration::from_secs(120);
    let flag = Arc::new(AtomicBool::new(false));
    let trace = Rc::new(RefCell::new(Vec::new()));
    let credentials = Rc::new(RefCell::new(Vec::new()));
    let mut work = observed_work(&trace, &credentials);
    let cancelled = Arc::clone(&flag);
    work.phase_hook = Some(Box::new(move |phase| {
        if phase == "after-credential" {
            cancelled.store(true, Ordering::Release);
        }
    }));
    let req = request(
        &root,
        [7; 16],
        Arc::clone(&flag),
        deadline,
        1024 * 1024 * 1024,
    );
    assert!(matches!(
        initialize_with_work(req, work),
        Err(OwnerError::OwnerAcquireOutcomeUnknown)
    ));
    assert_eq!(
        credentials.borrow().as_slice(),
        &[(true, deadline, Arc::as_ptr(&flag) as usize)]
    );
    let before = inventory(&root);
    flag.store(false, Ordering::Release);
    let (result, _, replay_credentials, output) = run(&root, [7; 16], 1024 * 1024 * 1024);
    assert!(result.is_err());
    assert!(replay_credentials.borrow().is_empty() && output.is_empty());
    assert_eq!(inventory(&root), before);
}

#[test]
fn body_only_torn_seal_and_changed_selected_key_are_refused_without_repair() {
    for kind in 0..3 {
        let root = scratch();
        assert_eq!(run(&root, [8; 16], 1024 * 1024 * 1024).0, Ok(()));
        let path = root.join(JOURNAL_FILE);
        let bytes = fs::read(&path).unwrap();
        let journal = decode_journal(&bytes, &mut |_| Ok(())).unwrap();
        let mut changed = journal.subject.frame().unwrap();
        if kind == 1 {
            changed = bytes[..bytes.len() - 1].to_vec();
        } else if kind == 2 {
            let mut seal = journal.completion.unwrap();
            seal.selected_key_binding[0] ^= 1;
            changed.extend_from_slice(&seal.frame().unwrap());
            assert!(
                decode_journal(&changed, &mut |_| Ok(()))
                    .unwrap()
                    .completion
                    .is_some()
            );
        }
        fs::write(&path, &changed).unwrap();
        let before = inventory(&root);
        let (result, _, credentials, output) = run(&root, [8; 16], 1024 * 1024 * 1024);
        assert!(result.is_err());
        assert!(credentials.borrow().iter().all(|row| !row.0) && output.is_empty());
        assert_eq!(inventory(&root), before);
    }
}

#[test]
fn terminal_reservation_refusal_distinguishes_prior_durable_effect_and_readonly_replay() {
    let control = scratch();
    let (result, trace, _, _) = run(&control, [9; 16], 1024 * 1024 * 1024);
    assert_eq!(result, Ok(()));
    let ceiling = terminal_io_ceiling(&trace);
    let root = scratch();
    let (result, trace, _, output) = run(&root, [9; 16], ceiling);
    assert_eq!(result, Err(OwnerError::OwnerReleaseOutcomeUnknown));
    assert!(output.is_empty());
    assert!(
        trace
            .borrow()
            .iter()
            .any(|row| row.0 == "write-reservation" && row.1[0] == ceiling && row.2 > 0)
    );
    assert!(
        decode_journal(&fs::read(root.join(JOURNAL_FILE)).unwrap(), &mut |_| Ok(()))
            .unwrap()
            .completion
            .is_some()
    );
    let (result, trace, _, _) = run(&control, [9; 16], 1024 * 1024 * 1024);
    assert_eq!(result, Ok(()));
    let ceiling = terminal_io_ceiling(&trace);
    let before = inventory(&control);
    let (result, trace, credentials, output) = run(&control, [9; 16], ceiling);
    assert_eq!(result, Err(OwnerError::ContractExhausted));
    assert!(
        trace
            .borrow()
            .iter()
            .any(|row| row.0 == "write-reservation" && row.1[0] == ceiling && row.2 > 0)
    );
    assert!(credentials.borrow().iter().all(|row| !row.0) && output.is_empty());
    assert_eq!(inventory(&control), before);
}
