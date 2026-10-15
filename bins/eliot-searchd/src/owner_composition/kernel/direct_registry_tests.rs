//! Real Native initializer + sole fixed DB + actual normal command seam. No FullNative claim.
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
impl Drop for Scratch {
    fn drop(&mut self) {
        assert_eq!(self.base.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            self.base
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("search-direct-owner-")
        );
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
