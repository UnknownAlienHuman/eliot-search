//! Existing-only store access for secure one-shot commands.
//!
//! Root admission owns quarantine and durable owner validation. These helpers
//! borrow its canonical root and never acquire a second owner.
//!
//! Integration requirement: read-only construction and strict status inspection
//! must not resolve revision secrets. The secure reader needs lazy protected
//! reads; status callers need catalog-only verification because the current
//! `DirectStore::verify` reads retained revision content. No plaintext fallback
//! or mutating opener is permitted to fill that seam.

use std::path::Path;

use crate::development::{DataRootGuard, InspectedDataRoot};
use crate::direct_store::{DirectStore, ReadOnlyStore};
use crate::owner_composition::DataRootRequest;
use crate::storage_security::StorageSecurityStatus;

/// Executes an existing-store read under the caller's original request.
///
/// The opener must neither create nor migrate storage. Admission and final
/// identity/barrier checks are performed by the inspection owner. Preflight
/// checks reuse the original absolute deadline and cancellation state.
pub(super) fn with_store_request<T>(
    root: &Path,
    request: &DataRootRequest,
    operation: impl FnOnce(&Path, &ReadOnlyStore<'_>, &StorageSecurityStatus) -> Result<T, String>,
) -> Result<T, String> {
    request.preflight()?;
    DataRootGuard::with_inspection_request(root, request, |cap: &InspectedDataRoot| {
        request.preflight()?;
        let store = DirectStore::open_existing_read_only(cap)?;
        let storage = StorageSecurityStatus::inspect_with_check(cap.canonical_root(), &|| {
            request.preflight()
        })?;
        request.preflight()?;
        let value = operation(cap.canonical_root(), &store, &storage)?;
        request.preflight()?;
        Ok(value)
    })
}

/// Executes one existing-store mutation under the caller's original request.
///
/// The marker is armed before dispatch. Closure errors, including output errors,
/// and failed verification do not clear it or reach clean release. Only exact
/// readback permits clean release followed by clearing. A drain, release or clear error remains
/// an error; dropping the guard is never reported as clean durable release.
/// Cancellation or deadline refusal before marker clearing retains recovery
/// evidence. No helper starts another request or resets the deadline.
pub(super) fn with_store_mut_request<T>(
    root: &Path,
    request: &DataRootRequest,
    operation: impl FnOnce(&Path, &mut DirectStore) -> Result<T, String>,
) -> Result<T, String> {
    request.preflight()?;
    let mut guard = DataRootGuard::open_existing_request(root, request)?;
    request.preflight()?;
    let mut store = DirectStore::open_existing_mutating(&guard)?;
    guard.verify_existing()?;
    request.preflight()?;
    let intent = guard.arm_catalog_intent(request)?;

    // Early errors close the store before the guard and retain recovery evidence.
    // Output belongs to the closure, so a failed acknowledgement cannot clear
    // the marker or produce a successful release through this helper.
    let value = operation(guard.canonical_root(), &mut store)?;
    request.preflight()?;
    store.verify()?;
    guard.verify_existing()?;
    request.preflight()?;
    drop(store);
    guard.begin_drain(search_runtime_owner::DrainReason::Shutdown)?;
    guard.release_catalog_cleanly(intent, request)?;
    Ok(value)
}

#[cfg(all(test, windows))]
mod catalog_intent_tests {
    use super::*;
    use crate::owner_composition::{InitializationRequest, initialize_new_request};
    use crate::root_draining_266::{NativeCrashChild, Scratch, snapshot};
    use search_contracts::{BoundedBytes, CanonicalValue, parse_canonical_cbor};
    use std::ffi::OsString;
    use std::fs;
    use std::io::{self, Read, Write};

    const CHILD_ROOT: &str = "ELIOT_SEARCH_TEST_266_CATALOG_INTENT_ROOT";
    const INITIALIZATION_ID: &str = "26600000000000000000000000000009";
    const READY: &[u8] = b"266_CATALOG_INTENT_DURABLE\n";
    const MARKER: &str = "control/catalog-quarantine.marker";
    const STAGING: &str = "control/catalog-quarantine.tmp";

    fn request(root: &Path, flag: &str, extra: Option<&str>) -> DataRootRequest {
        let mut arguments = vec![OsString::from(flag), root.as_os_str().to_owned()];
        if let Some(extra) = extra {
            arguments.push(extra.into());
        }
        DataRootRequest::from_cli(&arguments).unwrap()
    }

    fn crash_in_child(root: &Path) -> ! {
        let initialization = InitializationRequest::parse(INITIALIZATION_ID).unwrap();
        let initialize = request(root, "--initialize-data-root", Some(INITIALIZATION_ID));
        initialize_new_request(root, &initialization, &initialize).unwrap();
        let mutation = request(root, "--gc-root", Some("--apply"));
        let result: Result<(), String> =
            with_store_mut_request(root, &mutation, |canonical, _store| {
                // The production helper published/read back the intent before this
                // closure; keep its real store/owner/File alive until native kill.
                assert_eq!(
                    fs::read(canonical.join(MARKER)).unwrap(),
                    fs::read(canonical.join(STAGING)).unwrap()
                );
                let mut output = io::stdout().lock();
                output.write_all(b"\n").unwrap();
                output.write_all(READY).unwrap();
                output.flush().unwrap();
                drop(output);
                let mut unexpected = [0_u8; 1];
                let resumed = io::stdin().read(&mut unexpected);
                panic!("catalog-intent child resumed without termination: {resumed:?}");
            });
        panic!("catalog-intent child returned before crash: {result:?}");
    }

    fn exact_test_name() -> &'static str {
        concat!(
            module_path!(),
            "::catalog_intent_crash_retains_exact_original_inputs"
        )
        .split_once("::")
        .unwrap()
        .1
    }

    fn initialized() -> Scratch {
        let scratch = Scratch::new();
        let initialization = InitializationRequest::parse(INITIALIZATION_ID).unwrap();
        let initialize = request(
            &scratch.0,
            "--initialize-data-root",
            Some(INITIALIZATION_ID),
        );
        initialize_new_request(&scratch.0, &initialization, &initialize).unwrap();
        scratch
    }

    fn assert_retained_fences(root: &Path) {
        let marker = fs::read(root.join(MARKER)).unwrap();
        assert_eq!(fs::read(root.join(STAGING)).unwrap(), marker);
        let CanonicalValue::Array(fields) = parse_canonical_cbor(&marker).unwrap() else {
            panic!("closed intent absent after failed dispatch");
        };
        let CanonicalValue::Bytes(owner) = &fields.as_slice()[2] else {
            panic!("original owner absent after failed dispatch");
        };
        assert!(
            std::str::from_utf8(owner.as_slice())
                .unwrap()
                .lines()
                .any(|line| line == "lifecycle=ACTIVE")
        );
        assert!(
            [
                ".eliot-search-owner-state-a.v1",
                ".eliot-search-owner-state-b.v1"
            ]
            .into_iter()
            .any(|name| {
                let text = fs::read_to_string(root.join(name)).unwrap();
                text.lines().any(|line| line == "epoch=2")
                    && text.lines().any(|line| line == "generation=4")
                    && text.lines().any(|line| line == "lifecycle=ACTIVE")
            }),
            "failed dispatch unexpectedly advanced the durable owner"
        );
        let retained = snapshot(root);
        let mutation = request(root, "--gc-root", Some("--apply"));
        assert_eq!(
            DataRootGuard::open_existing_request(root, &mutation)
                .err()
                .as_deref(),
            Some("SERVICE_CATALOG_QUARANTINED")
        );
        assert_eq!(snapshot(root), retained);
        let inspection = request(root, "--health-data-root", None);
        let result: Result<(), String> =
            DataRootGuard::with_inspection_request(root, &inspection, |_| {
                panic!("failed dispatch admitted ordinary inspection");
            });
        assert_eq!(result.unwrap_err(), "SERVICE_CATALOG_QUARANTINED");
        assert_eq!(snapshot(root), retained);
    }

    #[test]
    fn closure_error_after_arm_retains_exact_inputs_and_fences() {
        let scratch = initialized();
        let root = &scratch.0;
        let mutation = request(root, "--gc-root", Some("--apply"));
        let result: Result<(), String> =
            with_store_mut_request(root, &mutation, |canonical, _store| {
                assert_eq!(
                    fs::read(canonical.join(MARKER)).unwrap(),
                    fs::read(canonical.join(STAGING)).unwrap()
                );
                Err("FIXTURE_CLOSURE_FAILED".to_owned())
            });
        assert_eq!(result.unwrap_err(), "FIXTURE_CLOSURE_FAILED");
        // The helper has dropped its real store/owner before the full snapshot.
        assert_retained_fences(root);
    }

    #[test]
    fn cancellation_after_arm_is_detected_by_actual_helper_and_retains_inputs() {
        let scratch = initialized();
        let root = &scratch.0;
        let mutation = request(root, "--gc-root", Some("--apply"));
        let result: Result<(), String> =
            with_store_mut_request(root, &mutation, |canonical, _store| {
                assert_eq!(
                    fs::read(canonical.join(MARKER)).unwrap(),
                    fs::read(canonical.join(STAGING)).unwrap()
                );
                mutation.cancel();
                // The real helper's post-closure preflight must detect cancellation.
                Ok(())
            });
        assert_eq!(result.unwrap_err(), "OWNER_CANCELLED_BEFORE_MUTATION");
        assert_retained_fences(root);
    }

    #[test]
    fn catalog_intent_crash_retains_exact_original_inputs() {
        use std::os::windows::ffi::OsStrExt;
        if let Some(root) = std::env::var_os(CHILD_ROOT) {
            crash_in_child(Path::new(&root));
        }
        let scratch = Scratch::new();
        let root = &scratch.0;
        let mut child = NativeCrashChild::start(root, CHILD_ROOT, exact_test_name(), READY);
        child.crash();
        drop(child);

        let marker = fs::read(root.join(MARKER)).unwrap();
        assert_eq!(fs::read(root.join(STAGING)).unwrap(), marker);
        let CanonicalValue::Array(fields) = parse_canonical_cbor(&marker).unwrap() else {
            panic!("closed catalog intent absent");
        };
        assert_eq!(fields.len(), 4);
        assert_eq!(fields.as_slice()[1], CanonicalValue::U64(2));
        let CanonicalValue::Bytes(record) = &fields.as_slice()[2] else {
            panic!("actual owner record absent");
        };
        let record = std::str::from_utf8(record.as_slice()).unwrap();
        assert!(record.lines().any(|line| line == "epoch=2"));
        assert!(record.lines().any(|line| line == "generation=4"));
        assert!(record.lines().any(|line| line == "lifecycle=ACTIVE"));
        let CanonicalValue::Array(input) = &fields.as_slice()[3] else {
            panic!("original command context absent");
        };
        assert_eq!(input.len(), 4);
        let CanonicalValue::Array(arguments) = &input.as_slice()[3] else {
            panic!("original native arguments absent");
        };
        let expected = [
            OsString::from("--gc-root"),
            root.as_os_str().to_owned(),
            OsString::from("--apply"),
        ];
        let expected = expected
            .iter()
            .map(|argument| {
                CanonicalValue::Bytes(
                    BoundedBytes::new(
                        argument
                            .encode_wide()
                            .flat_map(u16::to_le_bytes)
                            .collect::<Vec<_>>(),
                    )
                    .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(arguments.as_slice(), expected);

        let retained = snapshot(root);
        let CanonicalValue::Bytes(invocation) = &input.as_slice()[1] else {
            panic!("actual invocation identity absent");
        };
        let invocation: [u8; 16] = invocation.as_slice().try_into().unwrap();
        let invocation = search_contracts::RequestId::from_bytes(invocation).to_string();
        let name = crate::owner_composition::CatalogRecoveryRequest::parse(&invocation).unwrap();
        let named = DataRootRequest::from_cli(&[
            "--inspect-catalog-recovery".into(),
            root.as_os_str().to_owned(),
            invocation.into(),
        ])
        .unwrap();
        let observation =
            crate::owner_composition::inspect_catalog_recovery_request(root, &name, &named)
                .unwrap();
        assert_eq!(observation.state.as_str(), "unresolved_active");
        assert_eq!(
            (observation.owner_epoch, observation.owner_generation),
            (2, 4)
        );
        assert_eq!(snapshot(root), retained);
        let mutation = request(root, "--gc-root", Some("--apply"));
        assert_eq!(
            DataRootGuard::open_existing_request(root, &mutation)
                .err()
                .as_deref(),
            Some("SERVICE_CATALOG_QUARANTINED")
        );
        assert_eq!(snapshot(root), retained);
        let inspection = request(root, "--health-data-root", None);
        let result: Result<(), String> =
            DataRootGuard::with_inspection_request(root, &inspection, |_| {
                panic!("abandoned catalog intent admitted inspection");
            });
        assert_eq!(result.unwrap_err(), "SERVICE_CATALOG_QUARANTINED");
        assert_eq!(snapshot(root), retained);
    }
}
