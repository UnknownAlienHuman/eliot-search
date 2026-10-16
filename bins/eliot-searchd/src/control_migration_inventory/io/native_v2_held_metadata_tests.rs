//! Actual Main-existing Native admission around the held-metadata wrapper.
//! No canonical migration profile, private NativeV2 fixture or renewed context.

use super::*;
use crate::direct_store::migration_context::{MigrationContext, NativeMigrationInput};
use crate::owner_composition::{
    NativeRootAdmission, NativeRootLimits, NativeRootRequest, admit_existing, initialize_new,
};
use crate::revision_protection::{RevisionProtector, TestCredentialGuard};
use search_runtime_owner::DataRootOpenMode;
use search_runtime_owner::OwnerError;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

const PREFIX: &str = "search-main-held-metadata-";
const TARGET: &str = "10000000-0000-0000-0000-000000000268";

fn limits() -> NativeRootLimits {
    NativeRootLimits::new([256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256]).unwrap()
}

struct MainFixture {
    base: PathBuf,
    root: PathBuf,
    output: PathBuf,
    namespace: Option<[u8; 32]>,
    credentials: Option<TestCredentialGuard>,
}

impl MainFixture {
    fn new(deadline: Instant) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        let base = temp.join(format!(
            "{PREFIX}{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let root = base.join("data");
        let output = base.join("output");
        let sources = base.join("sources");
        let mut fixture = Self {
            base,
            root,
            output,
            namespace: None,
            credentials: None,
        };
        for path in [&fixture.root, &fixture.output, &sources] {
            fs::create_dir(path).unwrap();
        }
        let request = NativeRootRequest::new(
            DataRootOpenMode::InitializeNew,
            &[
                OsString::from("--initialize-data-root"),
                fixture.root.as_os_str().to_owned(),
                OsString::from("02680268026802680268026802680268"),
            ],
            deadline,
            Arc::new(AtomicBool::new(false)),
            limits(),
        )
        .unwrap();
        let mut terminal = Vec::new();
        fixture.credentials = Some(TestCredentialGuard::for_native_data_root(&fixture.root));
        let initialized = initialize_new(request).unwrap();
        fixture.namespace = Some(
            sha256::decode_digest(
                fs::read_to_string(fixture.root.join("control/namespace.id"))
                    .unwrap()
                    .trim(),
            )
            .unwrap(),
        );
        assert_eq!(fixture.credentials.as_ref().unwrap().capture_native_namespace().unwrap(), fixture.namespace);
        initialized.emit(&mut terminal).unwrap();
        assert_eq!(terminal, b"{\"initialized\":true,\"replayed\":false}\n");
        let registration =
            search_source_registry::encode_legacy_source_root_catalog([sources.to_str().unwrap()])
                .unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(fixture.root.join("control/source-roots.v1"))
            .unwrap();
        std::io::Write::write_all(&mut file, &registration).unwrap();
        file.sync_all().unwrap();
        drop(file);
        fixture
    }

    fn evidence(&self) -> [Vec<u8>; 9] {
        [
            ".eliot-search-installation.v1",
            ".eliot-search-initialization-journal.v2",
            ".eliot-search-owner-state-a.v1",
            ".eliot-search-owner-state-b.v1",
            ".eliot-search-owner.lock",
            ".eliot-search-sealed-owner.lock",
            "control/namespace.id",
            "control/source-events.log",
            "control/source-roots.v1",
        ]
        .map(|relative| fs::read(self.root.join(relative)).unwrap())
    }
}

impl Drop for MainFixture {
    fn drop(&mut self) {
        // Always clean the captured credential; retain failed filesystem evidence.
        let cleaned = self
            .credentials
            .as_ref()
            .is_none_or(TestCredentialGuard::cleanup_captured);
        if std::thread::panicking() {
            return;
        }
        assert!(cleaned, "exact fixture credential cleanup failed; artifacts retained");
        let current = fs::canonicalize(&self.base).unwrap();
        assert_eq!(current, self.base);
        assert_eq!(
            current.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        assert!(
            current
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(PREFIX)
        );
        fs::remove_dir_all(&current).unwrap();
    }
}

pub(super) fn with_main_held_metadata_fixture(
    phase_hook: Box<dyn FnMut(&'static str)>,
    inspect: impl FnOnce(&MigrationSource<'_, '_, '_>) -> Result<(), String>,
) -> Result<(), String> {
    let original_deadline = Instant::now() + Duration::from_secs(120);
    let fixture = MainFixture::new(original_deadline);
    let before = fixture.evidence();
    let cancellation = Arc::new(AtomicBool::new(false));
    let arguments = [
        OsString::from("--stage-source-control-migration"),
        fixture.root.as_os_str().to_owned(),
        OsString::from(TARGET),
        fixture.output.as_os_str().to_owned(),
    ];
    let (request, selected) = NativeRootRequest::for_source_control_migration(
        &arguments,
        original_deadline,
        Arc::clone(&cancellation),
        limits(),
    )
    .unwrap();
    assert_eq!(selected, arguments);
    let NativeRootAdmission::Inspection(owner) = admit_existing(request).unwrap() else {
        panic!("actual Main-existing source-migration inspection");
    };
    let mut terminal = Vec::new();
    let outcome = owner.run_registration_one_shot(|operation| {
        assert_eq!(operation.migration_deadline(), original_deadline);
        let selected_binding = operation
            .selected_binding()
            .expect("actual initialized key binding");
        let namespace = operation
            .namespace_bytes()
            .map_err(|error| error.code().to_owned())?;
        let log = operation
            .catalog_bytes()
            .map_err(|error| error.code().to_owned())?;
        let root = operation.root().to_owned();
        let store = crate::plaintext_direct_store::DirectStore::from_existing_bytes_checked(
            &root,
            &namespace,
            &log,
            || {
                operation
                    .migration_checkpoint()
                    .map_err(|error| error.code().to_owned())
            },
        )?;
        let registration = operation.registration_bytes()?;
        assert_eq!(registration.as_deref(), Some(before[8].as_slice()));
        let namespace_id = fixture.namespace.expect("actual namespace bytes");
        let protector = operation
            .credential_call(|deadline, cancellation| {
                RevisionProtector::for_existing_credential(
                    namespace_id,
                    Some(selected_binding),
                    deadline,
                    cancellation,
                )
            })
            .map_err(|error| error.code().to_owned())?;
        let inventory_owner = operation
            .migration_inventory_owner()
            .map_err(|error| error.code().to_owned())?;
        operation.chain_migration_phase_hook_for_test(phase_hook);
        let result = {
            let context = MigrationContext::new(operation);
            let input = NativeMigrationInput {
                namespace: &namespace,
                log: &log,
                protector: &protector,
                context: &context,
                inventory_owner,
                registration: registration.as_deref(),
            };
            let source = MigrationSource::new(&store, original_deadline, Some(&input));
            assert!(source.native().is_some());
            inspect(&source)
        };
        drop(protector);
        if let Err(refusal) = operation.verify() {
            assert_eq!(
                operation.emit_registration(&mut terminal, b"must not escape\n"),
                Err(refusal.code().to_owned())
            );
        }
        assert!(!operation.output_attempted());
        // Even a successful inspector cannot override original sticky refusal.
        result
    });
    assert!(terminal.is_empty());
    assert_eq!(fixture.evidence(), before);
    assert_eq!(fs::read_dir(&fixture.output).unwrap().count(), 0);
    assert_eq!(
        fs::read_dir(fixture.root.join("revisions"))
            .unwrap()
            .count(),
        0
    );
    assert!(!fixture.root.join("control/source-roots.tmp").exists());
    assert!(!cancellation.load(Ordering::Acquire));
    outcome
}

#[test]
fn native_v2_pending_during_held_metadata_refuses_proof_and_stays_terminal() {
    let dispatches = Rc::new(Cell::new(0));
    let pending = Rc::new(RefCell::new(None::<PathBuf>));
    let injected = Rc::new(Cell::new(false));
    let hook_dispatches = Rc::clone(&dispatches);
    let hook_pending = Rc::clone(&pending);
    let hook_injected = Rc::clone(&injected);
    let outcome = with_main_held_metadata_fixture(
        Box::new(move |phase| {
            if phase != "migration-held-observer-dispatch" {
                return;
            }
            let count = hook_dispatches.get() + 1;
            hook_dispatches.set(count);
            if count == 2 {
                // First actual GetFileType completed; the adversary precedes
                // GetVolumeInformationByHandleW outside Native accounting.
                let path = hook_pending.borrow();
                let path = path.as_ref().expect("armed only around the actual wrapper");
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .unwrap();
                std::io::Write::write_all(&mut file, b"adversarial pending catalog update")
                    .unwrap();
                file.sync_all().unwrap();
                hook_injected.set(true);
            }
        }),
        |source| {
            let path = source.inventory_root().join("control/source-roots.tmp");
            assert!(!path.exists());
            let file =
                File::open(source.inventory_root().join("control/source-events.log")).unwrap();
            *pending.borrow_mut() = Some(path.clone());
            let observed = revision_pin_observation(source, &file);
            assert!(
                injected.get(),
                "pending must be injected inside the observer"
            );
            assert_eq!(dispatches.get(), 3, "all three actual ABI attempts ran");
            assert_eq!(
                observed,
                Err(OwnerError::OwnerRecoveryQuarantined.code().to_owned()),
                "outer post-check returns neither proof nor a soft inventory status"
            );
            fs::remove_file(&path).expect("remove only this test's pending sidecar");
            assert_eq!(
                source.poll(),
                Err(OwnerError::OwnerRecoveryQuarantined.code().to_owned())
            );
            assert_eq!(
                source.checkpoint(),
                Err(OwnerError::OwnerRecoveryQuarantined.code().to_owned())
            );
            Ok(())
        },
    );
    assert_eq!(
        outcome,
        Err(OwnerError::OwnerRecoveryQuarantined.code().to_owned())
    );
}
