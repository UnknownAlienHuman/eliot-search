//! Actual native intent/owner observations; no fabricated owner records.

use super::*;
use crate::owner_composition::{
    CatalogMutationIntent, InitializationRequest, initialize_new_request,
};
use crate::root_draining_266::{Scratch, snapshot};
use search_contracts::CanonicalValue;
use search_runtime_owner::DrainReason;

const INITIALIZATION_ID: &str = "2660000000000000000000000000000b";

fn armed() -> (
    Scratch,
    DataRootGuard,
    CatalogMutationIntent,
    CatalogRecoveryRequest,
) {
    let scratch = Scratch::new();
    let root = &scratch.0;
    let initialization = InitializationRequest::parse(INITIALIZATION_ID).unwrap();
    let request = DataRootRequest::from_cli(&[
        "--initialize-data-root".into(),
        root.as_os_str().to_owned(),
        INITIALIZATION_ID.into(),
    ])
    .unwrap();
    initialize_new_request(root, &initialization, &request).unwrap();
    let request = DataRootRequest::from_cli(&[
        "--gc-root".into(),
        root.as_os_str().to_owned(),
        "--apply".into(),
    ])
    .unwrap();
    let guard = DataRootGuard::open_existing_request(root, &request).unwrap();
    let intent = guard.arm_catalog_intent(&request).unwrap();
    let evidence = CatalogIntentEvidence::decode(&fs::read(root.join(MARKER)).unwrap()).unwrap();
    let name = CatalogRecoveryRequest::parse(&evidence.request_id.to_string()).unwrap();
    (scratch, guard, intent, name)
}

fn recovery_request(root: &Path, name: &CatalogRecoveryRequest) -> DataRootRequest {
    DataRootRequest::from_cli(&[
        COMMAND.into(),
        root.as_os_str().to_owned(),
        name.0.to_string().into(),
    ])
    .unwrap()
}

fn discovery_request(root: &Path) -> DataRootRequest {
    DataRootRequest::from_cli(&[
        DISCOVERY_COMMAND.into(),
        root.as_os_str().to_owned(),
    ])
    .unwrap()
}

#[test]
fn discovery_returns_only_the_exact_named_observation_without_writes() {
    let (scratch, guard, intent, name) = armed();
    drop(intent);
    drop(guard);
    let root = &scratch.0;
    let before = snapshot(root);
    let discovered = list_catalog_recovery_request(root, &discovery_request(root)).unwrap();
    let named = inspect_catalog_recovery_request(root, &name, &recovery_request(root, &name))
        .unwrap();
    assert_eq!(discovered, named);
    assert_eq!(discovered.operation_id, name.0);
    assert!(!format!("{discovered:?}").contains(&root.to_string_lossy().to_string()));
    assert_eq!(snapshot(root), before);
}

#[test]
fn discovery_cannot_initialize_absent_or_empty_roots() {
    let scratch = Scratch::new();
    let missing = scratch.0.join("absent");
    let before = snapshot(&scratch.0);
    for root in [&scratch.0, &missing] {
        assert!(list_catalog_recovery_request(root, &discovery_request(root)).is_err());
        assert_eq!(snapshot(&scratch.0), before);
    }
    assert!(!missing.exists());
}

#[test]
fn discovery_context_cancel_and_partial_alias_refusals_preserve_evidence() {
    let (scratch, guard, intent, name) = armed();
    drop(intent);
    drop(guard);
    let root = &scratch.0;
    let before = snapshot(root);
    assert_eq!(
        list_catalog_recovery_request(root, &recovery_request(root, &name))
            .err()
            .as_deref(),
        Some("OWNER_OPERATION_CONFLICT")
    );
    let request = discovery_request(root);
    request.cancel();
    assert_eq!(
        list_catalog_recovery_request(root, &request)
            .err()
            .as_deref(),
        Some("OWNER_CANCELLED_BEFORE_MUTATION")
    );
    assert_eq!(snapshot(root), before);
    fs::remove_file(root.join(STAGING)).unwrap();
    let before = snapshot(root);
    assert_eq!(
        list_catalog_recovery_request(root, &discovery_request(root))
            .err()
            .as_deref(),
        Some("OWNER_GUARD_MISMATCH")
    );
    assert_eq!(snapshot(root), before);
    assert!(root.join(MARKER).exists());
}

#[test]
fn named_active_inspection_preserves_evidence_and_grants_no_ordinary_open() {
    let (scratch, guard, intent, name) = armed();
    drop(intent);
    drop(guard);
    let root = &scratch.0;
    let before = snapshot(root);
    let request = recovery_request(root, &name);
    let observation = inspect_catalog_recovery_request(root, &name, &request).unwrap();
    assert_eq!(observation.operation_id, name.0);
    assert_eq!(observation.kind, CatalogOperationKind::GcApply);
    assert_eq!(observation.state, CatalogRecoveryState::UnresolvedActive);
    assert_eq!(
        (observation.owner_epoch, observation.owner_generation),
        (2, 4)
    );
    assert!(!format!("{observation:?}").contains(&root.to_string_lossy().to_string()));
    assert_eq!(snapshot(root), before);
    let mutation = DataRootRequest::from_cli(&[
        "--gc-root".into(),
        root.as_os_str().to_owned(),
        "--apply".into(),
    ])
    .unwrap();
    assert_eq!(
        DataRootGuard::open_existing_request(root, &mutation)
            .err()
            .as_deref(),
        Some("SERVICE_CATALOG_QUARANTINED")
    );
    assert_eq!(snapshot(root), before);
}

#[test]
fn original_shutdown_lifecycles_are_observations_without_cleanup() {
    for released in [false, true] {
        let (scratch, mut guard, intent, name) = armed();
        guard.begin_drain(DrainReason::Shutdown).unwrap();
        if released {
            guard.release_cleanly().unwrap();
        } else {
            drop(guard);
        }
        drop(intent);
        let root = &scratch.0;
        let before = snapshot(root);
        let request = recovery_request(root, &name);
        let observation = inspect_catalog_recovery_request(root, &name, &request).unwrap();
        assert_eq!(
            observation.state,
            if released {
                CatalogRecoveryState::ReleasedAwaitingCleanup
            } else {
                CatalogRecoveryState::UnresolvedDraining
            }
        );
        assert_eq!(observation.owner_generation, if released { 6 } else { 5 });
        assert_eq!(snapshot(root), before);
        assert!(root.join(MARKER).exists());
        assert!(root.join(STAGING).exists());
    }
}

#[test]
fn foreign_name_mismatched_context_and_cancellation_are_preserving_refusals() {
    let (scratch, guard, intent, name) = armed();
    drop(intent);
    drop(guard);
    let root = &scratch.0;
    let before = snapshot(root);
    let mut foreign_bytes = *name.0.as_bytes();
    foreign_bytes[0] ^= 1;
    let foreign =
        CatalogRecoveryRequest::parse(&RequestId::from_bytes(foreign_bytes).to_string()).unwrap();
    let request = recovery_request(root, &foreign);
    assert_eq!(
        inspect_catalog_recovery_request(root, &foreign, &request)
            .err()
            .as_deref(),
        Some("OWNER_OPERATION_CONFLICT")
    );
    assert_eq!(snapshot(root), before);
    assert_eq!(
        inspect_catalog_recovery_request(root, &name, &request)
            .err()
            .as_deref(),
        Some("OWNER_OPERATION_CONFLICT")
    );
    assert_eq!(snapshot(root), before);
    let request = recovery_request(root, &name);
    request.cancel();
    assert_eq!(
        inspect_catalog_recovery_request(root, &name, &request)
            .err()
            .as_deref(),
        Some("OWNER_CANCELLED_BEFORE_MUTATION")
    );
    assert_eq!(snapshot(root), before);
}

#[test]
fn identical_bytes_in_another_staging_object_cannot_satisfy_native_binding() {
    let (scratch, guard, intent, name) = armed();
    drop(intent);
    drop(guard);
    let root = &scratch.0;
    let bytes = fs::read(root.join(MARKER)).unwrap();
    fs::remove_file(root.join(STAGING)).unwrap();
    fs::write(root.join(STAGING), &bytes).unwrap();
    let before = snapshot(root);
    let request = recovery_request(root, &name);
    assert_eq!(
        inspect_catalog_recovery_request(root, &name, &request)
            .err()
            .as_deref(),
        Some("OWNER_GUARD_MISMATCH")
    );
    assert_eq!(snapshot(root), before);
}

#[test]
fn changed_original_digest_and_legacy_marker_cannot_name_recovery() {
    for legacy in [false, true] {
        let (scratch, guard, intent, name) = armed();
        drop(intent);
        drop(guard);
        let root = &scratch.0;
        let bytes = if legacy {
            b"ELIOT_SEARCH_CATALOG_QUARANTINE_V1\nreason=SERVICE_MUTATION_OUTCOME_UNKNOWN\n"
                .to_vec()
        } else {
            let mut bytes = fs::read(root.join(MARKER)).unwrap();
            let CanonicalValue::Array(fields) = parse_canonical_cbor(&bytes).unwrap() else {
                panic!("actual intent absent");
            };
            let CanonicalValue::Array(input) = &fields.as_slice()[3] else {
                panic!("actual input absent");
            };
            let CanonicalValue::Bytes(digest) = &input.as_slice()[2] else {
                panic!("actual original digest absent");
            };
            let positions = bytes
                .windows(digest.len())
                .enumerate()
                .filter_map(|(position, value)| (value == digest.as_slice()).then_some(position))
                .collect::<Vec<_>>();
            assert_eq!(positions.len(), 1);
            bytes[positions[0]] ^= 1;
            bytes
        };
        // Corrupt only actual fixture-produced evidence, never owner identities.
        fs::write(root.join(MARKER), bytes).unwrap();
        let before = snapshot(root);
        let request = recovery_request(root, &name);
        assert_eq!(
            inspect_catalog_recovery_request(root, &name, &request)
                .err()
                .as_deref(),
            Some("OWNER_RECOVERY_QUARANTINED")
        );
        assert_eq!(snapshot(root), before);
    }
}
