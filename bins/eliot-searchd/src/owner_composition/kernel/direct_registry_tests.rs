//! Real Native initializer + sole fixed DB + actual normal command seam. No FullNative claim.
#[path = "direct_registry_tests/native_denials.rs"]
mod native_denials;
#[path = "direct_registry_tests/native_dek.rs"]
mod native_dek;

use super::*;
use search_runtime_owner::DataRootOpenMode;
use std::fs::OpenOptions;
use std::{
    ffi::OsString,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use std::{fs, path::PathBuf, time::Instant};

struct Scratch {
    base: PathBuf,
    root: PathBuf,
    source: PathBuf,
    other: PathBuf,
    config: PathBuf,
}

// Owned Windows fixtures; execution remains part of integrated Native qualification.
fn retained_v2(revision: u64) -> search_source_provenance::RetainedPolicyV2 {
    use search_source_admission::{self as a, policy_v2::*};
    let (platform, capabilities) = command::admission_environment();
    let mut section = a::compiled_defaults();
    section.max_file_bytes = 1024;
    let compiled = compile_policy_v2(&PolicyInputV2 {
        schema_version: POLICY_SCHEMA_VERSION, revision,
        section: a::validate_section(&section, &platform, &capabilities).unwrap(),
        rules: vec![RuleInputV2 { id: "direct-v2-fixture".into(), precedence: 0,
            source_class: a::SourceClass::Regular, predicate: PredicateV2::Always,
            effect: a::RuleEffect::Allow, unknown_fields: vec![] }],
        unknown_fields: vec![],
    }, DEFAULT_COMPILE_LIMITS, &mut || Ok(())).unwrap();
    search_source_provenance::RetainedPolicyV2::capture(&compiled, &platform, &capabilities, &mut || Ok(())).unwrap()
}

fn run_v2(s: &Scratch, source: &Path, archive: &Path) -> Result<String, String> {
    let args = [OsString::from("--register-source-root-v2"), s.root.as_os_str().to_owned(),
        source.as_os_str().to_owned(), archive.as_os_str().to_owned(),
        OsString::from("--config-file"), s.config.as_os_str().to_owned()];
    let mut output = Vec::new();
    command::run(&args, Instant::now(), &mut output)?;
    Ok(String::from_utf8(output).unwrap())
}

#[test]
fn direct_v2_actual_policy_and_o1_replay_after_o2() {
    let s = Scratch::new(true);
    let policy = retained_v2(7);
    let archive = s.base.join("policy.cbor");
    fs::write(&archive, policy.encoded_inputs()).unwrap();
    let first = run_v2(&s, &s.source, &archive).unwrap();
    assert!(first.contains("\"registered\":true"));
    assert!(first.contains("\"policy_revision\":7"));
    assert!(first.contains(&super::super::super::codec::hex(policy.fingerprint().as_bytes())));
    assert!(run_v2(&s, &s.other, &archive).unwrap().contains("\"registry_revision\":2"));
    let replay = run_v2(&s, &s.source, &archive).unwrap();
    for field in ["\"registered\":false", "\"historical_commit\":true",
        "\"historical_registry_revision\":1", "\"registry_revision\":2",
        "\"root_count\":2", "\"current_qualification\":true", "\"ready\":false",
        "\"source_body_serving\":false", "\"safe_read_security_barrier_revision\":null"] {
        assert!(replay.contains(field), "{field}: {replay}");
    }
    // The same pathname cannot substitute a different valid policy for O1.
    fs::write(&archive, retained_v2(8).encoded_inputs()).unwrap();
    assert_eq!(run_v2(&s, &s.source, &archive).unwrap_err(), "DIRECT_ORIGINAL_OPERATION_CONFLICT");
}

#[test]
fn direct_v2_noncanonical_and_unknown_policy_refuse_before_owner_acquisition() {
    use search_contracts::{CanonicalKey, CanonicalValue as V, parse_canonical_cbor, to_canonical_cbor};
    let s = Scratch::new(true);
    let policy = retained_v2(7);
    let V::Object(mut fields) = parse_canonical_cbor(policy.encoded_inputs()).unwrap() else { panic!("archive map"); };
    fields.insert(CanonicalKey::new("unexpected").unwrap(), V::U64(1)).unwrap();
    let unknown = to_canonical_cbor(&V::Object(fields)).unwrap().as_slice().to_vec();
    let archive = s.base.join("invalid.cbor");
    for bytes in [vec![0xff], unknown] {
        fs::write(&archive, bytes).unwrap();
        let slots = s.slots();
        let journal = fs::read(s.root.join("control/control.redb")).unwrap();
        assert!(run_v2(&s, &s.source, &archive).is_err());
        assert_eq!(s.slots(), slots);
        assert_eq!(fs::read(s.root.join("control/control.redb")).unwrap(), journal);
    }
}

#[test]
fn native_source_v2_root_fd_policy_identity_redb_and_original_replay() {
    let s = Scratch::new(true);
    let policy = retained_v2(7);
    let archive = s.base.join("native-policy.cbor");
    fs::write(&archive, policy.encoded_inputs()).unwrap();
    fs::write(s.source.join("one.rs"), b"pub fn native_source_fixture() {}\n").unwrap();
    run_v2(&s, &s.source, &archive).unwrap();
    let args = [OsString::from("--admit-source-v2"), s.root.as_os_str().to_owned(),
        s.source.as_os_str().to_owned(), OsString::from("one.rs"), archive.as_os_str().to_owned(),
        OsString::from("41414141414141414141414141414141"), OsString::from("new"),
        OsString::from("--config-file"), s.config.as_os_str().to_owned()];
    let mut output = Vec::new();
    command::run(&args, Instant::now(), &mut output).unwrap();
    let first = String::from_utf8(output).unwrap();
    assert!(first.contains("\"admitted\":true"), "{first}");
    assert!(first.contains("\"registry_revision\":2"), "{first}");
    assert!(first.contains("\"ready\":false"));
    let field = |text: &str, name: &str| text.split(&format!("\"{name}\":\""))
        .nth(1).unwrap().split('"').next().unwrap().to_owned();
    let mut second_args = args.clone();
    second_args[3] = "two.rs".into();
    second_args[5] = "42424242424242424242424242424242".into();
    fs::write(s.source.join("two.rs"), b"pub fn second_native_source() {}\n").unwrap();
    let mut second = Vec::new();
    command::run(&second_args, Instant::now(), &mut second).unwrap();
    let second = String::from_utf8(second).unwrap();
    assert!(second.contains("\"registry_revision\":3"), "{second}");
    assert_eq!(field(&first, "namespace"), field(&second, "namespace"));
    assert_ne!(field(&first, "source"), field(&second, "source"));
    fs::remove_file(s.source.join("one.rs")).unwrap();
    let body_attempts = super::super::read_child::source_body_attempts_for_test();
    let mut replay = Vec::new();
    command::run(&args, Instant::now(), &mut replay).unwrap();
    let replay = String::from_utf8(replay).unwrap();
    assert!(replay.contains("\"admitted\":true"), "{replay}");
    assert!(replay.contains("\"historical_commit\":true"), "{replay}");
    assert!(replay.contains("\"registry_revision\":2"), "{replay}");
    assert_eq!(field(&first, "source"), field(&replay, "source"));
    assert_eq!(super::super::read_child::source_body_attempts_for_test(), body_attempts);
    let mut conflicting = args.clone();
    conflicting[3] = "two.rs".into();
    assert_eq!(command::run(&conflicting, Instant::now(), &mut Vec::new()).unwrap_err(),
        "NATIVE_SOURCE_OPERATION_CONFLICT");
    assert_eq!(super::super::read_child::source_body_attempts_for_test(), body_attempts);
}

fn owned_fixture_namespace(root: &Path) -> Result<[u8; 32], &'static str> {
    let root = fs::canonicalize(root).map_err(|_| "fixture-root-unavailable")?;
    let temp = fs::canonicalize(std::env::temp_dir()).map_err(|_| "fixture-temp-unavailable")?;
    let base = root.parent().ok_or("fixture-base-missing")?;
    if base.parent() != Some(temp.as_path())
        || !base.file_name().and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("search-direct-owner-"))
        || root.file_name() != Some(std::ffi::OsStr::new("data"))
    {
        return Err("fixture-path-invalid");
    }
    let history = fs::read(root.join(super::super::super::initialization_journal::JOURNAL_FILE))
        .map_err(|_| "fixture-journal-unavailable")?;
    let original = super::super::super::initialization_journal::decode_journal(&history, &mut |_| Ok(()))
        .map_err(|_| "fixture-journal-invalid")?;
    if original.subject.initialization != [0x36; 16] {
        return Err("fixture-initialization-mismatch");
    }
    Ok(original.subject.namespace)
}

#[test]
#[ignore = "requires an explicitly selected retained fixture"]
fn native_fixture_existing_credential_readback_only() {
    let path = PathBuf::from(std::env::var_os("ELIOT_SEARCH_TEST_RECOVERY_ROOT").expect("explicit retained fixture"));
    let namespace = owned_fixture_namespace(&path).expect("exact owned fixture journal");
    let observed = search_os_secrets_windows::load_existing_legacy_revision_root_secret_with_context(
        &namespace, Instant::now() + Duration::from_secs(30), &AtomicBool::new(false));
    match observed {
        Ok(Some(secret)) => { drop(secret); eprintln!("native-existing-readback:present"); }
        Ok(None) => eprintln!("native-existing-readback:absent"),
        Err(error) => panic!("native-existing-readback:error:{}", error.code()),
    }
    if std::env::var_os("ELIOT_SEARCH_TEST_CLEANUP_OWNED_FIXTURE").as_deref() == Some(std::ffi::OsStr::new("1")) {
        search_os_secrets_windows::delete_legacy_revision_root_secret_for_test(&namespace)
            .expect("exact retained fixture credential cleanup");
        eprintln!("native-existing-readback:exact-owned-key-cleanup:verified-absent");
    }
}

impl Scratch {
    fn new(direct: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "search-direct-owner-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let value = Self {
            root: base.join("data"),
            source: base.join("source"),
            other: base.join("other"),
            config: base.join("roots.toml"),
            base,
        };
        for path in [&value.root, &value.source, &value.other] {
            fs::create_dir(path).unwrap();
        }
        if direct { prepare_private_fixture_root(&value); }
        let request = value.request("--initialize-data-root", DataRootOpenMode::InitializeNew);
        let initialized = if direct {
            super::super::super::initialization::initialize_direct(request)
        } else {
            super::super::super::initialization::initialize_new(request)
        }
        .unwrap();
        initialized.emit(&mut Vec::new()).unwrap();
        value.write_config(&value.source, &value.other);
        value
    }
    fn write_config(&self, a: &Path, b: &Path) {
        let escape = |p: &Path| {
            p.to_str()
                .unwrap()
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
        };
        fs::write(&self.config, format!("schema_version = 2\nprofile = \"direct\"\n[source_root_locators]\nentries = [\"{}={}\", \"{}={}\"]\n",
            "31".repeat(16), escape(a), "32".repeat(16), escape(b))).unwrap();
    }
    fn request(&self, command: &str, mode: DataRootOpenMode) -> NativeRootRequest {
        let mut args = vec![OsString::from(command), self.root.as_os_str().to_owned()];
        if mode == DataRootOpenMode::InitializeNew {
            args.push("36363636363636363636363636363636".into());
        }
        NativeRootRequest::new(
            mode,
            &args,
            Instant::now() + Duration::from_secs(120),
            Arc::new(AtomicBool::new(false)),
            super::super::super::native_request::NativeRootLimits::new([
                256,
                256 * 1024,
                512 * 1024,
                1024 * 1024 * 1024,
                16384,
                256,
            ])
            .unwrap(),
        )
        .unwrap()
    }
    fn run(&self, command: &str, source: Option<&Path>) -> Result<String, String> {
        let mut args = vec![OsString::from(command), self.root.as_os_str().to_owned()];
        if let Some(source) = source {
            args.push(source.as_os_str().to_owned());
        }
        args.extend([
            OsString::from("--config-file"),
            self.config.as_os_str().to_owned(),
        ]);
        let mut output = Vec::new();
        command::run(&args, Instant::now(), &mut output)?;
        Ok(String::from_utf8(output).unwrap())
    }
    fn slots(&self) -> (Vec<u8>, Vec<u8>) {
        (
            fs::read(self.root.join(super::super::super::spec::OWNER_SLOT_A)).unwrap(),
            fs::read(self.root.join(super::super::super::spec::OWNER_SLOT_B)).unwrap(),
        )
    }
}

// Test preparation is deliberately outside the product initializer. This host's
// Temp inherits foreign trustees; only our exact new empty root is changed.
#[allow(unsafe_code, reason = "Test-only ACL setup on the exact newly created empty Scratch root")]
fn prepare_private_fixture_root(scratch: &Scratch) {
    use core::ffi::c_void;
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    assert!(scratch.base.is_absolute());
    assert_eq!(scratch.root.parent(), Some(scratch.base.as_path()));
    assert!(scratch.base.file_name().unwrap().to_str().unwrap()
        .starts_with(&format!("search-direct-owner-{}-", std::process::id())));
    assert!(fs::read_dir(&scratch.root).unwrap().next().is_none());
    let file = OpenOptions::new().access_mode(0x0006_0080).custom_flags(0x0220_0000)
        .share_mode(7).open(&scratch.root).unwrap();
    eliot_searchd::native_file::observe(&file).unwrap();
    // Owner Rights applies only to the actual object owner; SYSTEM/Admin remain trusted.
    let sids: [&[u8]; 3] = [
        &[1,1,0,0,0,0,0,3,4,0,0,0], &[1,1,0,0,0,0,0,5,18,0,0,0],
        &[1,2,0,0,0,0,0,5,32,0,0,0,32,2,0,0],
    ];
    let length = 8 + sids.iter().map(|sid| 8 + sid.len()).sum::<usize>();
    let mut bytes = vec![2,0];
    bytes.extend_from_slice(&u16::try_from(length).unwrap().to_le_bytes());
    bytes.extend_from_slice(&[3,0,0,0]);
    for sid in sids {
        bytes.extend_from_slice(&[0,3]);
        bytes.extend_from_slice(&u16::try_from(8 + sid.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&0x001f_01ff_u32.to_le_bytes());
        bytes.extend_from_slice(sid);
    }
    let mut words: Vec<u32> = bytes.chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap())).collect();
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn SetSecurityInfo(handle: *mut c_void, kind: u32, info: u32,
            owner: *mut c_void, group: *mut c_void, dacl: *mut c_void, sacl: *mut c_void) -> u32;
    }
    assert_eq!(unsafe { SetSecurityInfo(file.as_raw_handle(), 1, 4 | 0x8000_0000,
        std::ptr::null_mut(), std::ptr::null_mut(), words.as_mut_ptr().cast(), std::ptr::null_mut()) }, 0);
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let panicking = std::thread::panicking();
        let cleanup = owned_fixture_namespace(&self.root).and_then(|namespace| {
            search_os_secrets_windows::delete_legacy_revision_root_secret_for_test(&namespace)
                .map_err(|error| error.code())
        });
        if let Err(reason) = cleanup {
            use std::io::Write;
            let _ = writeln!(std::io::stderr().lock(), "native fixture credential cleanup failed: {reason}; disk retained");
            if !panicking {
                panic!("exact fixture credential cleanup failed: {reason}");
            }
            return;
        }
        if panicking {
            use std::io::Write;
            let _ = writeln!(std::io::stderr().lock(), "native fixture retained after failure; exact owned credential absence verified");
            return;
        }
        fs::remove_dir_all(&self.base).unwrap();
    }
}

#[test]
fn direct_owner_init_restart_register_restart_o2_then_o1_original_replay() {
    let s = Scratch::new(true);
    assert!(
        s.run("--source-roots", None)
            .unwrap()
            .contains("\"root_count\":0")
    );
    let first = s.run("--register-source-root", Some(&s.source)).unwrap();
    assert!(first.contains("\"registered\":true"));
    let observed = s.run("--source-roots", None).unwrap();
    assert!(observed.contains("\"registry_revision\":1"));
    assert!(
        s.run("--register-source-root", Some(&s.other))
            .unwrap()
            .contains("\"registry_revision\":2")
    );
    let replay = s.run("--register-source-root", Some(&s.source)).unwrap();
    assert!(replay.contains("\"historical_commit\":true"));
    assert!(replay.contains("\"historical_registry_revision\":1"));
    assert!(replay.contains("\"registered\":false"));
    assert!(replay.contains("\"root_count\":2"));
    assert!(replay.contains("\"registry_revision\":2"));
    assert!(replay.contains("\"ready\":false"));
    assert!(
        s.run("--source-roots", None)
            .unwrap()
            .contains(&"32".repeat(16))
    );
}

#[test]
fn direct_owner_missing_db_and_legacy_root_never_create_or_fallback() {
    let s = Scratch::new(false);
    let before = s.slots();
    assert!(s.run("--register-source-root", Some(&s.source)).is_err());
    assert!(!s.root.join("control/control.redb").exists());
    assert_eq!(before, s.slots());
}

#[test]
fn direct_owner_equal_byte_replaced_db_is_not_original() {
    let s = Scratch::new(true);
    let path = s.root.join("control/control.redb");
    let backup = s.base.join("held-original.redb");
    fs::rename(&path, &backup).unwrap();
    fs::copy(&backup, &path).unwrap();
    let before = s.slots();
    assert!(s.run("--source-roots", None).is_err());
    assert_eq!(before, s.slots());
}

#[test]
fn direct_owner_inert_db_cannot_be_adopted_as_fresh() {
    let s = Scratch::new(false);
    let raw = fs::read(
        s.root
            .join(super::super::super::initialization_journal::JOURNAL_FILE),
    )
    .unwrap();
    let subject =
        super::super::super::initialization_journal::decode_journal(&raw, &mut |_| Ok(()))
            .unwrap()
            .subject;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(s.root.join("control/control.redb"))
        .unwrap();
    let observation = eliot_searchd::native_file::observe(&file).unwrap();
    let identity = crate::control_identity::journal_identity_from_observation(
        InstallationIncarnationId::from_bytes(subject.incarnation),
        DataRootId::from_bytes(subject.root_id),
        OwnerEpoch::new(1).unwrap(),
        observation,
    )
    .unwrap();
    let mut journal =
        PersistentControlJournal::create(file, identity, JournalLimits::BASELINE).unwrap();
    let ctx = search_ports::OperationContext::new(
        search_contracts::RequestId::from_bytes([1; 16]),
        120_000,
        search_ports::FakeCancellation::new(false),
        search_contracts::OpaqueRef::new("inert-fixture").unwrap(),
    )
    .unwrap();
    journal
        .initialize_operating_profile(
            search_control_redb::JournalProfileInitialization::MigrationInert,
            MutationId([2; 32]),
            Blake3Digest32::from_bytes([3; 32]),
            &ctx,
        )
        .unwrap();
    drop(journal);
    let before = s.slots();
    assert!(s.run("--source-roots", None).is_err());
    assert_eq!(before, s.slots());
}

#[test]
fn direct_owner_missing_hint_and_partial_policy_refuse_before_owner_effects() {
    let s = Scratch::new(true);
    let before = s.slots();
    fs::write(&s.config, "schema_version = 2\nprofile = \"direct\"\n").unwrap();
    assert!(s.run("--register-source-root", Some(&s.source)).is_err());
    assert_eq!(before, s.slots());
    fs::write(
        &s.config,
        "schema_version = 2\nprofile = \"direct\"\n[source_admission]\nallow_generated = false\n",
    )
    .unwrap();
    assert!(s.run("--register-source-root", Some(&s.source)).is_err());
    assert_eq!(before, s.slots());
}

#[test]
fn direct_owner_cancel_and_live_db_pin_preserve_custody() {
    let s = Scratch::new(true);
    let before = s.slots();
    let request = s.request("--source-roots", DataRootOpenMode::InspectExistingReadOnly);
    request
        .original_cancellation()
        .store(true, Ordering::Release);
    assert!(DirectRegistryOwner::open(request).is_err());
    assert_eq!(before, s.slots());
    let owner = DirectRegistryOwner::open(
        s.request("--source-roots", DataRootOpenMode::InspectExistingReadOnly),
    )
    .unwrap();
    assert!(
        fs::rename(
            s.root.join("control/control.redb"),
            s.base.join("moved.redb")
        )
        .is_err()
    );
    owner.finish().unwrap();
}

#[test]
fn direct_owner_late_flush_unknown_never_releases_or_blindly_replays() {
    struct FlushFailure;
    impl Write for FlushFailure {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("late flush fixture"))
        }
    }
    let s = Scratch::new(true);
    let mut args = vec![
        OsString::from("--register-source-root"),
        s.root.as_os_str().to_owned(),
        s.source.as_os_str().to_owned(),
    ];
    args.extend([
        OsString::from("--config-file"),
        s.config.as_os_str().to_owned(),
    ]);
    assert!(command::run(&args, Instant::now(), &mut FlushFailure).is_err());
    let before = s.slots();
    assert!(s.run("--register-source-root", Some(&s.source)).is_err());
    assert_eq!(before, s.slots());
    assert!(s.root.join("control/control.redb").exists());
}

#[test]
fn direct_owner_locator_contract_and_lossy_native_input_refuse_before_effects() {
    use std::os::windows::ffi::OsStringExt;
    let s = Scratch::new(true);
    let before = s.slots();
    for entries in [
        "[\"BAD=relative\"]".to_owned(),
        format!("[\"{}=relative\"]", "31".repeat(16)),
        format!(
            "[\"{}=C:\\\\one\", \"{}=C:\\\\two\"]",
            "31".repeat(16),
            "31".repeat(16)
        ),
        format!(
            "[{}]",
            std::iter::repeat_n(format!("\"{}=C:\\\\one\"", "31".repeat(16)), 65)
                .collect::<Vec<_>>()
                .join(",")
        ),
    ] {
        fs::write(&s.config, format!("schema_version = 2\nprofile = \"direct\"\n[source_root_locators]\nentries = {entries}\n")).unwrap();
        assert!(s.run("--register-source-root", Some(&s.source)).is_err());
        assert_eq!(before, s.slots());
    }
    s.write_config(&s.source, &s.other);
    let invalid = OsString::from_wide(&[b'C' as u16, b':' as u16, b'\\' as u16, 0xd800]);
    let args = vec![
        OsString::from("--register-source-root"),
        s.root.as_os_str().to_owned(),
        invalid,
        OsString::from("--config-file"),
        s.config.as_os_str().to_owned(),
    ];
    assert!(command::run(&args, Instant::now(), &mut Vec::new()).is_err());
    assert_eq!(before, s.slots());
}

#[test]
fn direct_owner_historical_root_commit_cannot_reactivate_unbound_root() {
    use search_source_registry::{
        DEFAULT_REGISTRY_LIMITS, api::unbind_root, error::RegistryControlPort,
    };
    let s = Scratch::new(true);
    s.run("--register-source-root", Some(&s.source)).unwrap();
    let mut owner = DirectRegistryOwner::open(s.request(
        "--register-source-root",
        DataRootOpenMode::OpenExistingMutating,
    ))
    .unwrap();
    {
        let deadline = owner.owner.request.credential_context().0;
        let mut operation = super::super::read_child::direct_operation(&mut owner.owner).unwrap();
        let native = operation.fresh_root_context().unwrap();
        let callback = |attempt| native.work(attempt);
        let journal = owner.control.journal.as_mut().unwrap();
        let identity = journal.identity();
        let mut port = crate::registry_control::RegistryJournal::bind_original(
            journal,
            &mut owner.control.publisher,
            identity,
            DEFAULT_REGISTRY_LIMITS,
            deadline,
            &callback,
        )
        .unwrap();
        let mut registry = port.restore(native.context()).unwrap().into_registry();
        let root = registry.roots().values().next().unwrap().clone();
        unbind_root(
            &mut registry,
            root.root_binding_id,
            root.record_revision,
            &search_contracts::OpaqueId::new("actual-fixture-unbind").unwrap(),
            Blake3Digest32::from_bytes([41; 32]),
            &search_contracts::ReceiptRef::new("actual-unbind-readback").unwrap(),
            &mut port,
            native.context(),
        )
        .unwrap();
        assert!(matches!(
            port.load_batch(
                &search_contracts::OpaqueId::new("actual-fixture-unbind").unwrap(),
                native.context()
            )
            .unwrap(),
            search_source_registry::recovery::RegistryBatchLookup::Committed(_)
        ));
        drop(port);
        journal
            .publish_direct_with_original_context(&mut owner.control.publisher, deadline, &callback)
            .unwrap();
    }
    owner.finish().unwrap();
    let args = vec![
        OsString::from("--register-source-root"),
        s.root.as_os_str().to_owned(),
        s.source.as_os_str().to_owned(),
        OsString::from("--config-file"),
        s.config.as_os_str().to_owned(),
    ];
    let mut output = Vec::new();
    assert!(command::run(&args, Instant::now(), &mut output).is_err());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("\"historical_commit\":true"));
    assert!(output.contains("\"current_qualification\":false"));
    assert!(output.contains("\"registry_revision\":2"));
}

#[test]
fn direct_owner_parent_bound_creation_refuses_foreign_locator_before_effect() {
    let a = Scratch::new(false);
    let b = Scratch::new(false);
    let before_a = a.slots();
    let before_b = b.slots();
    let admitted = super::super::admit_existing(
        a.request("--source-roots", DataRootOpenMode::InspectExistingReadOnly),
    )
    .unwrap();
    let super::super::NativeRootAdmission::Inspection(mut owner) = admitted else {
        panic!("actual Native custody");
    };
    let binding = journal_owner(
        owner.installation.installation_incarnation_id,
        owner.record.data_root_id,
        owner.record.epoch,
    )
    .unwrap();
    let result = DirectJournal::open(
        &owner.request,
        &mut owner.work,
        &b.root,
        &owner.pins.files[3],
        binding,
        true,
    );
    assert!(result.is_err());
    assert!(!a.root.join("control/control.redb").exists());
    assert!(!b.root.join("control/control.redb").exists());
    drop(owner);
    assert_eq!(before_a, a.slots());
    assert_eq!(before_b, b.slots());
}

#[test]
fn direct_owner_handoff_never_promotes_genesis_or_root_revision_to_live_read_fence() {
    let s = Scratch::new(true);
    let genesis = s.run("--source-roots", None).unwrap();
    assert!(genesis.contains("\"journal_generation\":1"));
    assert!(genesis.contains("\"registry_revision\":0"));
    assert!(genesis.contains("\"safe_read_security_barrier_revision\":null"));
    let registered = s.run("--register-source-root", Some(&s.source)).unwrap();
    assert!(registered.contains("\"policy_revision\":1"));
    assert!(registered.contains("\"record_revision\":1"));
    assert!(registered.contains("\"canonical_root_digest\":"));
    assert!(registered.contains("\"qualification_scope\":\"root_metadata_only\""));
    assert!(registered.contains("\"safe_read_security_barrier_revision\":null"));
    assert!(registered.contains("DIRECT_LIVE_SECURITY_OWNER_UNAVAILABLE"));
    assert!(registered.contains("\"source_body_serving\":false"));
}

#[test]
fn direct_owner_corrupt_db_is_refused_without_legacy_fallback_or_recreation() {
    let s = Scratch::new(true);
    let db = s.root.join("control/control.redb");
    let corrupt = b"invalid schema5 database, never an empty new installation";
    fs::write(&db, corrupt).unwrap();
    assert!(s.run("--source-roots", None).is_err());
    assert_eq!(fs::read(&db).unwrap(), corrupt);
    assert!(!s.root.join("source-roots.v1").exists());
}
