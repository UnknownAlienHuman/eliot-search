//! Owning fixtures authored for deferred integrated/native qualification only.

use super::*;
use std::ffi::OsString;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

fn request(root: &Path, source: &Path, command: &str, flag: Arc<AtomicBool>) -> NativeRootRequest {
    NativeRootRequest::for_source_registration(
        &[
            OsString::from(command),
            root.as_os_str().to_owned(),
            source.as_os_str().to_owned(),
        ],
        Instant::now() + Duration::from_secs(120),
        flag,
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
    .0
}

#[test]
fn registration_selector_keeps_original_native_config_arguments_and_context() {
    let flag = Arc::new(AtomicBool::new(false));
    let args = [
        OsString::from("--config-file"),
        OsString::from("private-config"),
        OsString::from("--register-source-root"),
        OsString::from("private-root"),
        OsString::from("private-source"),
    ];
    let deadline = Instant::now() + Duration::from_secs(120);
    let (request, selected) = NativeRootRequest::for_source_registration(
        &args,
        deadline,
        Arc::clone(&flag),
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
    .unwrap();
    assert_eq!(
        request.mode,
        search_runtime_owner::DataRootOpenMode::OpenExistingMutating
    );
    assert_eq!(request.root(), Path::new("private-root"));
    assert_eq!(request.arguments().len(), 5);
    assert_eq!(selected.len(), 3);
    assert_eq!(request.credential_context().0, deadline);
    assert_eq!(format!("{request:?}"), "NativeRootRequest(<opaque>)");
    flag.store(true, Ordering::Release);
    assert_eq!(
        request.check(),
        Err(OwnerError::OwnerCancelledBeforeMutation)
    );
}

#[cfg(windows)]
#[test]
fn semantic_preflight_refuses_without_active_catalog_or_owner_slot_writes() {
    super::super::native_tests::with_released_registration_fixture(|root, _source| {
        let before: Vec<_> = [
            super::super::super::spec::OWNER_SLOT_A,
            super::super::super::spec::OWNER_SLOT_B,
        ]
        .map(|name| fs::read(root.join(name)).unwrap())
        .into();
        let mut output = Vec::new();
        let mut error = Vec::new();
        assert_eq!(
            run_source_registration(
                request(
                    root,
                    root,
                    "--register-source-root",
                    Arc::new(AtomicBool::new(false))
                ),
                SourceRootAction::Register,
                root,
                &mut output,
                &mut error
            ),
            Err("SOURCE_ROOT_DATA_ROOT_OVERLAP".to_owned())
        );
        assert!(output.is_empty());
        assert_eq!(error, b"{\"error\":\"SOURCE_ROOT_DATA_ROOT_OVERLAP\"}\n");
        for (name, before) in [
            super::super::super::spec::OWNER_SLOT_A,
            super::super::super::spec::OWNER_SLOT_B,
        ]
        .into_iter()
        .zip(before)
        {
            assert_eq!(fs::read(root.join(name)).unwrap(), before);
        }
        for name in [
            CatalogName::Primary,
            CatalogName::Temporary,
            CatalogName::Backup,
        ] {
            assert!(!root.join("control").join(name.text()).exists());
        }
    });
}

#[cfg(windows)]
#[test]
fn compatible_register_duplicate_and_missing_root_removal_keep_exact_catalog_bytes() {
    super::super::native_tests::with_released_registration_fixture(|root, source| {
        let invoke = |command, action| {
            let mut output = Vec::new();
            let mut error = Vec::new();
            run_source_registration(
                request(root, source, command, Arc::new(AtomicBool::new(false))),
                action,
                source,
                &mut output,
                &mut error,
            )
            .unwrap();
            assert!(error.is_empty());
            assert!(
                String::from_utf8(output)
                    .unwrap()
                    .contains("\"current_workspace_proven\":false")
            );
        };
        invoke("--register-source-root", SourceRootAction::Register);
        let catalog = root.join("control/source-roots.v1");
        let bytes = fs::read(&catalog).unwrap();
        let file = File::open(&catalog).unwrap();
        let identity = native_identity(&file).unwrap();
        let modified = file.metadata().unwrap().modified().unwrap();
        drop(file);
        invoke("--register-source-root", SourceRootAction::Register);
        let file = File::open(&catalog).unwrap();
        assert_eq!(native_identity(&file).unwrap(), identity);
        assert_eq!(file.metadata().unwrap().modified().unwrap(), modified);
        assert_eq!(fs::read(&catalog).unwrap(), bytes);
        drop(file);
        fs::remove_dir(source).unwrap();
        invoke("--unregister-source-root", SourceRootAction::Unregister);
        assert_eq!(
            fs::read(&catalog).unwrap(),
            b"# ELIOT Search source roots v1\n"
        );
        assert!(!root.join("control/source-roots.tmp").exists());
        assert!(!root.join("control/source-roots.bak").exists());
    });
}

#[cfg(windows)]
#[test]
fn original_context_loss_after_active_never_creates_catalog_or_releases_owner() {
    super::super::native_tests::with_released_registration_fixture(|root, source| {
        let flag = Arc::new(AtomicBool::new(false));
        let changed = Arc::clone(&flag);
        let mut work = NativeWork::new();
        work.phase_hook = Some(Box::new(move |phase| {
            if phase == "registration-before-create" {
                changed.store(true, Ordering::Release);
            }
        }));
        let mut output = Vec::new();
        let mut error = Vec::new();
        assert_eq!(
            run_with_work(
                request(root, source, "--register-source-root", Arc::clone(&flag)),
                work,
                SourceRootAction::Register,
                source,
                &mut output,
                &mut error
            ),
            Err(OwnerError::OwnerAcquireOutcomeUnknown.code().to_owned())
        );
        assert!(output.is_empty());
        assert!(error.is_empty());
        assert!(!root.join("control/source-roots.v1").exists());
        assert!(!root.join("control/source-roots.tmp").exists());
        assert!(
            flag.load(Ordering::Acquire),
            "before-create cancellation hook did not fire"
        );
        flag.store(false, Ordering::Release);
        let inspection = NativeRootRequest::new(
            search_runtime_owner::DataRootOpenMode::InspectExistingReadOnly,
            &[
                OsString::from("inspection-fixture"),
                root.as_os_str().to_owned(),
            ],
            Instant::now() + Duration::from_secs(120),
            flag,
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
        .unwrap();
        assert!(matches!(
            super::super::admit_existing(inspection).unwrap(),
            super::super::NativeRootAdmission::Unavailable(
                search_runtime_owner::RootOpenState::OutcomeUnknown,
                _
            )
        ));
    });
}
#[cfg(windows)]
#[test]
fn exhausted_original_meter_never_opens_a_second_allowance_or_mutates_slots() {
    super::super::native_tests::with_released_registration_fixture(|root, source| {
        let before = [
            super::super::super::spec::OWNER_SLOT_A,
            super::super::super::spec::OWNER_SLOT_B,
        ]
        .map(|name| fs::read(root.join(name)).unwrap());
        let request = request(
            root,
            source,
            "--register-source-root",
            Arc::new(AtomicBool::new(false)),
        );
        let mut work = NativeWork::new();
        work.read(&request, 1024 * 1024 * 1024).unwrap();
        let mut output = Vec::new();
        let mut error = Vec::new();
        assert_eq!(
            run_with_work(
                request,
                work,
                SourceRootAction::Register,
                source,
                &mut output,
                &mut error
            ),
            Err(OwnerError::ContractExhausted.code().to_owned())
        );
        assert!(output.is_empty());
        assert!(error.is_empty());
        for (name, bytes) in [
            super::super::super::spec::OWNER_SLOT_A,
            super::super::super::spec::OWNER_SLOT_B,
        ]
        .into_iter()
        .zip(before)
        {
            assert_eq!(fs::read(root.join(name)).unwrap(), bytes);
        }
        assert!(!root.join("control/source-roots.v1").exists());
        assert!(!root.join("control/source-roots.tmp").exists());
    });
}
