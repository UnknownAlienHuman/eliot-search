//! One real Native -> protected Credential Manager -> redb -> restart proof.
use super::*;
use search_os_secrets::RevisionDekBinding;
use search_os_secrets_windows::{self as os, LegacyRevisionDpapiContext, LegacyRevisionDpapiWork};
use std::{cell::{Cell, RefCell}, rc::Rc};

struct CleanupContext { deadline: Instant }
impl CleanupContext { fn new() -> Self { Self { deadline: Instant::now() + Duration::from_secs(30) } } }
impl LegacyRevisionDpapiContext for CleanupContext {
    type Error = &'static str;
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        if Instant::now() < self.deadline { Ok(()) } else { Err("TEST_CLEANUP_DEADLINE") }
    }
    fn charge(&mut self, _: LegacyRevisionDpapiWork) -> Result<(), Self::Error> { self.checkpoint() }
    fn account_native_clear(&mut self, _: usize) -> Result<(), Self::Error> { Ok(()) }
}
impl os::RevisionDekContext for CleanupContext {
    fn deadline(&self) -> Instant { self.deadline }
    fn charge_open(&mut self) -> Result<(), Self::Error> { self.checkpoint() }
}

struct OwnedCandidates {
    targets: Rc<RefCell<Vec<RevisionDekBinding>>>,
    cleaned: Cell<Option<bool>>,
}
impl OwnedCandidates {
    fn new() -> Self { Self { targets: Rc::new(RefCell::new(Vec::new())), cleaned: Cell::new(None) } }
    fn cleanup(&self) -> bool {
        if let Some(outcome) = self.cleaned.get() { return outcome; }
        let mut context = CleanupContext::new();
        let mut outcome = true;
        for binding in self.targets.borrow().iter() {
            outcome &= os::delete_revision_dek_for_test(binding, &mut context).is_ok();
        }
        self.cleaned.set(Some(outcome));
        outcome
    }
}
impl Drop for OwnedCandidates {
    fn drop(&mut self) {
        let outcome = self.cleanup();
        if !outcome {
            let _ = std::io::Write::write_all(&mut std::io::stderr(),
                b"exact candidate credential cleanup unresolved; disk retained\n");
            assert!(std::thread::panicking(), "exact candidate credential cleanup unresolved; disk retained");
        }
    }
}

fn run(s: &Scratch, owned: &OwnedCandidates, operation: &str) -> (Result<(), String>, String) {
    let args = [OsString::from("--prepare-revision-dek-v1"), s.root.as_os_str().to_owned(), operation.into()];
    let deadline = Instant::now() + Duration::from_secs(120);
    let (request, selected) = NativeRootRequest::for_direct_registry(&args,
        DataRootOpenMode::OpenExistingMutating, deadline, Arc::new(AtomicBool::new(false)),
        super::super::super::super::native_request::NativeRootLimits::new(
            [256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256]).unwrap()).unwrap();
    let command = super::super::dek_command::RevisionDekCommand::parse(&selected).unwrap();
    let targets = Rc::clone(&owned.targets);
    let mut work = NativeWork::new();
    work.revision_dek_target_hook = Some(Box::new(move |binding| targets.borrow_mut().push(binding.clone())));
    let owner = DirectRegistryOwner::open_with_work(request, work).unwrap();
    let mut output = Vec::new();
    let result = owner.execute_revision_dek(&command, &mut output);
    (result, String::from_utf8(output).unwrap())
}

#[test]
fn native_dek_candidate_persists_restarts_conflicts_and_cleans_exact_target() {
    let s = Scratch::new(true);
    // Declared after Scratch so candidate cleanup runs before its disk teardown.
    let owned = OwnedCandidates::new();
    let (result, first) = run(&s, &owned, "51515151515151515151515151515151");
    assert!(result.is_ok(), "{first}: {result:?}");
    for field in ["\"prepared\":true", "\"historical_commit\":false", "\"role\":\"candidate_nonserving\"",
        "\"current_selected\":false", "\"key_use\":false", "\"ready\":false"] {
        assert!(first.contains(field), "{first}");
    }
    assert_eq!(owned.targets.borrow().len(), 1, "one possible publication path");
    let binding = owned.targets.borrow()[0].clone();
    let first_read = os::read_revision_dek(&binding, &mut CleanupContext::new()).unwrap().unwrap();
    let (result, replay) = run(&s, &owned, "51515151515151515151515151515151");
    assert!(result.is_ok(), "{replay}: {result:?}");
    assert!(replay.contains("\"historical_commit\":true"), "{replay}");
    assert_eq!(owned.targets.borrow().len(), 1, "replay must not enter a publication path");
    let after = os::read_revision_dek(&binding, &mut CleanupContext::new()).unwrap().unwrap();
    assert!(first_read.protected_bytes() == after.protected_bytes(), "protected key changed on replay");
    let (result, conflict) = run(&s, &owned, "52525252525252525252525252525252");
    assert!(result.is_err());
    assert!(conflict.contains("REVISION_DEK_OPERATION_CONFLICT"), "{conflict}");
    assert_eq!(owned.targets.borrow().len(), 1, "conflict must not enter publication");
    assert!(owned.cleanup(), "exact candidate cleanup");
    assert!(os::read_revision_dek(&binding, &mut CleanupContext::new()).unwrap().is_none(), "candidate still present");
}

fn select(s: &Scratch, operation: &str, expected: &[u8; 32], config_file: &Path) -> (Result<(), String>, String) {
    let args = [OsString::from("--select-revision-dek-v1"), s.root.as_os_str().to_owned(), operation.into(),
        super::super::super::super::codec::hex(expected).into(), "--config-file".into(), config_file.as_os_str().to_owned()];
    let deadline = Instant::now() + Duration::from_secs(120);
    let (request, selected) = NativeRootRequest::for_direct_registry(&args,
        DataRootOpenMode::OpenExistingMutating, deadline, Arc::new(AtomicBool::new(false)),
        super::super::super::super::native_request::NativeRootLimits::new(
            [256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256]).unwrap()).unwrap();
    let (_, cli) = crate::config_composition::strip_config_args_os(&args).unwrap();
    let command = super::super::dek_selection::SelectionCommand::parse(&selected, cli).unwrap();
    let owner = DirectRegistryOwner::open_with_work(request, NativeWork::new()).unwrap();
    let mut output = Vec::new();
    let result = owner.execute_revision_dek_selection(&command, &mut output);
    (result, String::from_utf8(output).unwrap())
}

#[test]
fn native_dek_first_current_reopens_replays_without_config_and_cleans_exact_target() {
    let s = Scratch::new(true);
    let owned = OwnedCandidates::new();
    let (result, candidate_output) = run(&s, &owned, "61616161616161616161616161616161");
    assert!(result.is_ok(), "{candidate_output}: {result:?}");
    assert_eq!(owned.targets.borrow().len(), 1);
    let binding = owned.targets.borrow()[0].clone();
    let before = os::read_revision_dek(&binding, &mut CleanupContext::new()).unwrap().unwrap();
    let config_file = s.base.join("selection-backend.toml");
    let config_bytes = b"schema_version=2\nprofile=\"direct\"\n[secrets]\nbackend=\"windows_user_bound\"\nlease_ttl_ms=20000\nrotation_grace_ms=0\n";
    fs::write(&config_file, config_bytes).unwrap();
    let document = crate::config_composition::capture_file_document(config_bytes, "config-file").unwrap();
    let effective = crate::config_composition::build_effective(Some(document), None, None, "direct", "direct").unwrap();
    let expected = *effective.fingerprint().as_bytes();
    let operation = "62626262626262626262626262626262";
    let (result, first) = select(&s, operation, &expected, &config_file);
    assert!(result.is_ok(), "{first}: {result:?}");
    for field in ["\"durable_current_committed\":true", "\"current_backend_validated\":true",
        "\"historical_commit\":false", "\"whole_daemon_config_applied\":false",
        "\"ttl_enforced\":false", "\"rotation_enforced\":false", "\"key_use\":false",
        "\"leases\":false", "\"source_body_serving\":false", "\"ready\":false"] {
        assert!(first.contains(field), "{first}");
    }
    fs::write(&config_file, b"invalid selection replay input").unwrap();
    let (result, replay) = select(&s, operation, &expected, &config_file);
    assert!(result.is_ok(), "{replay}: {result:?}");
    assert!(replay.contains("\"historical_commit\":true"), "{replay}");
    fs::remove_file(&config_file).unwrap();
    let (result, replay) = select(&s, operation, &expected, &config_file);
    assert!(result.is_ok(), "{replay}: {result:?}");
    assert!(replay.contains("\"current_backend_validated\":true"), "{replay}");
    let mut changed = expected;
    changed[0] ^= 1;
    let (result, conflict) = select(&s, operation, &changed, &config_file);
    assert!(result.is_err());
    assert!(conflict.contains("REVISION_DEK_SELECTION_OPERATION_CONFLICT"), "{conflict}");
    let (result, conflict) = select(&s, "63636363636363636363636363636363", &expected, &config_file);
    assert!(result.is_err());
    assert!(conflict.contains("REVISION_DEK_SELECTION_OPERATION_CONFLICT"), "{conflict}");
    let after = os::read_revision_dek(&binding, &mut CleanupContext::new()).unwrap().unwrap();
    assert!(before.protected_bytes() == after.protected_bytes(), "selection must not replace a key");
    assert_eq!(owned.targets.borrow().len(), 1);
    assert!(owned.cleanup(), "exact candidate cleanup");
    assert!(os::read_revision_dek(&binding, &mut CleanupContext::new()).unwrap().is_none());
}

fn reopen_deleted_retained_source(arguments: &[OsString], config: &crate::config_composition::EffectiveDaemonConfig) {
    let deadline = Instant::now() + Duration::from_secs(120);
    let (request, selected) = NativeRootRequest::for_direct_registry(arguments,
        DataRootOpenMode::OpenExistingMutating, deadline, Arc::new(AtomicBool::new(false)),
        super::super::super::super::native_request::NativeRootLimits::for_source_retention()).unwrap();
    let command = super::super::retention_command::NativeRetentionCommand::parse(&selected).unwrap();
    let mut work = NativeWork::new();
    let policy = super::super::super::super::direct_policy_v2::capture(
        &request, &mut work, Path::new(&selected[4])).unwrap();
    let hints = crate::config_composition::RootLocatorHints::from_config(config).unwrap();
    let mut owner = DirectRegistryOwner::open_with_work(request, work).unwrap();
    // This is a Native source-owner proof. Provider grant/IPC qualification is
    // separate; the fixture does not expose a new command or mint permission.
    owner.with_retention_source(&hints, Path::new(&selected[2]), &policy, &command,
        |input, journal, publisher| {
            assert!(input.body.is_none(), "deleted source must never be reacquired");
            let profile = input.existing.profile.as_ref().unwrap();
            let expected = search_contracts::SourceRevisionRef {
                source_namespace_id: profile.source_namespace_id, source_id: profile.source_id,
                revision_id: profile.revision_id, content_digest: profile.content_digest,
                byte_length: profile.byte_length,
            };
            let work = |unit| input.native.work(unit);
            let control = super::super::retention_control::NativeRetentionControl::bind(journal, publisher,
                input.source, input.namespace, input.root, input.security, input.deadline, &work)
                .map_err(|error| error.to_string())?;
            let mut wrong = expected;
            wrong.content_digest = search_contracts::Blake3Digest32::from_bytes([0x55; 32]);
            assert!(control.load_retained_revision(&wrong, input.native.context()).is_err());
            let plaintext = super::super::retained_reader::open_retained_revision(input.native,
                &control, &expected, config)?;
            assert_eq!(plaintext.as_bytes(), b"fn retained_native_fixture() {}\n");
            drop(plaintext);
            drop(control);
            super::super::retention::consume_source(input, journal, publisher, config)
        }).unwrap();
    owner.finish().unwrap();
}

#[test]
fn native_retained_source_reopens_deleted_source_and_replays_without_key_use() {
    let s = Scratch::new(true);
    // Capture the exact target before any candidate write; cleanup precedes
    // Scratch teardown on success and every panic/error path.
    let owned = OwnedCandidates::new();
    let policy = retained_v2(7);
    let archive = s.base.join("retention-policy.cbor");
    fs::write(&archive, policy.encoded_inputs()).unwrap();
    let source = s.source.join("retained.rs");
    fs::write(&source, b"fn retained_native_fixture() {}\n").unwrap();
    run_v2(&s, &s.source, &archive).unwrap();

    let (result, candidate) = run(&s, &owned, "71717171717171717171717171717171");
    assert!(result.is_ok(), "{candidate}: {result:?}");
    assert_eq!(owned.targets.borrow().len(), 1);
    let binding = owned.targets.borrow()[0].clone();
    let original_config = fs::read_to_string(&s.config).unwrap();
    let config_bytes = format!("{original_config}[secrets]\nbackend=\"windows_user_bound\"\nlease_ttl_ms=20000\nrotation_grace_ms=0\n");
    fs::write(&s.config, &config_bytes).unwrap();
    let document = crate::config_composition::capture_file_document(config_bytes.as_bytes(), "config-file").unwrap();
    let config = crate::config_composition::build_effective(Some(document), None, None, "direct", "direct").unwrap();
    let (result, selected) = select(&s, "72727272727272727272727272727272", config.fingerprint().as_bytes(), &s.config);
    assert!(result.is_ok(), "{selected}: {result:?}");

    let arguments = [OsString::from("--retain-source-v2"), s.root.as_os_str().to_owned(),
        s.source.as_os_str().to_owned(), OsString::from("retained.rs"), archive.as_os_str().to_owned(),
        OsString::from("73737373737373737373737373737373"), OsString::from("new"), OsString::from("new"),
        OsString::from("--config-file"), s.config.as_os_str().to_owned()];
    let mut output = Vec::new();
    let result = command::run(&arguments, Instant::now(), &mut output);
    let first = String::from_utf8(output).unwrap();
    assert!(result.is_ok(), "{first}: {result:?}");
    for field in ["\"event\":\"native_source_retained\"", "\"historical_commit\":false",
        "\"retained_revision\":true", "\"object_readback_verified\":true",
        "\"source_body_serving\":false", "\"workspace_current\":false", "\"ready\":false"] {
        assert!(first.contains(field), "{first}");
    }
    let identity = |text: &str, field: &str| text.split(&format!("\"{field}\":\""))
        .nth(1).unwrap().split('"').next().unwrap().to_owned();
    for field in ["namespace", "source", "revision"] {
        let id = identity(&first, field);
        assert_eq!(id.len(), 32);
        assert_eq!(&id[12..13], "4", "genuine v4 {field}");
        assert!(matches!(&id[16..17], "8" | "9" | "a" | "b"));
    }

    fs::remove_file(&source).unwrap();
    reopen_deleted_retained_source(&arguments, &config);
    assert!(owned.cleanup(), "exact owned candidate cleanup");
    assert!(os::read_revision_dek(&binding, &mut CleanupContext::new()).unwrap().is_none());
    fs::write(&s.config, config_bytes.replace("lease_ttl_ms=20000", "lease_ttl_ms=20001")).unwrap();
    let mut output = Vec::new();
    let result = command::run(&arguments, Instant::now(), &mut output);
    let replay = String::from_utf8(output).unwrap();
    assert!(result.is_ok(), "{replay}: {result:?}");
    assert!(replay.contains("\"historical_commit\":true"), "{replay}");
    assert!(replay.contains("\"object_readback_verified\":false"), "{replay}");
    assert!(replay.contains("\"source_body_serving\":false"), "{replay}");
    for field in ["namespace", "source", "revision"] {
        assert_eq!(identity(&first, field), identity(&replay, field));
    }
    assert_eq!(owned.targets.borrow().len(), 1, "replay must not publish a key");
}
