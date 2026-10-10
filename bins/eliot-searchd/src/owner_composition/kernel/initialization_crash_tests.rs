//! Native initialization crash/recovery proofs over actual owner phases.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use super::{
    DataRootRequest, INTENT, InitializationRequest, PRIMARY, SEALED, acquire_new,
    initialize_new_request, publish_new_layout, recover_initialization_request,
};
use crate::development::DataRootGuard;
use crate::root_draining_266::{NativeCrashChild, Scratch, snapshot};

// Only this fixture reads the child locator; no product fault switch exists.
const CHILD_ROOT: &str = "ELIOT_SEARCH_TEST_266_PARTIAL_INITIALIZATION_ROOT";
const INITIALIZATION_ID: &str = "26600000000000000000000000000005";
const READY: &[u8] = b"266_INITIALIZATION_INTENT_DURABLE\n";

fn request(root: &Path, flag: &str, id: Option<&str>) -> DataRootRequest {
    let mut arguments = vec![OsString::from(flag), root.as_os_str().to_owned()];
    if let Some(id) = id {
        arguments.push(OsString::from(id));
    }
    DataRootRequest::from_cli(&arguments).unwrap()
}

const COMPLETE_CHILD_ROOT: &str = "ELIOT_SEARCH_TEST_266_COMPLETE_INITIALIZATION_ROOT";
const COMPLETE_INITIALIZATION_ID: &str = "26600000000000000000000000000006";
const FOREIGN_INITIALIZATION_ID: &str = "26600000000000000000000000000007";
const COMPLETE_READY: &[u8] = b"266_INITIALIZATION_LAYOUT_PUBLISHED\n";

fn complete_in_child(root: &Path) -> ! {
    let initialization = InitializationRequest::parse(COMPLETE_INITIALIZATION_ID).unwrap();
    let operation = request(
        root,
        "--initialize-data-root",
        Some(COMPLETE_INITIALIZATION_ID),
    );
    let cap = acquire_new(root, &initialization, &operation).unwrap();
    let record = publish_new_layout(&cap).unwrap();
    cap.verify().unwrap();
    assert_eq!(record.epoch, 1);
    assert_eq!(record.generation, 1);
    assert_eq!(record.lifecycle, super::LifecycleState::Active);
    let mut output = io::stdout().lock();
    output.write_all(b"\n").unwrap();
    output.write_all(COMPLETE_READY).unwrap();
    output.flush().unwrap();
    drop(output);
    let mut unexpected = [0_u8; 1];
    let result = io::stdin().read(&mut unexpected);
    let retained_namespace = cap.namespace_id();
    panic!("complete initialization child resumed: {result:?}, namespace={retained_namespace:?}");
}

fn complete_exact_test_name() -> &'static str {
    concat!(
        module_path!(),
        "::complete_initialization_crash_replays_published_layout"
    )
    .split_once("::")
    .unwrap()
    .1
}

#[test]
fn complete_initialization_crash_replays_published_layout() {
    if let Some(root) = std::env::var_os(COMPLETE_CHILD_ROOT) {
        complete_in_child(Path::new(&root));
    }
    let scratch = Scratch::new();
    let root = &scratch.0;
    let mut child = NativeCrashChild::start(
        root,
        COMPLETE_CHILD_ROOT,
        complete_exact_test_name(),
        COMPLETE_READY,
    );
    child.crash();
    drop(child);
    let retained = snapshot(root);
    for name in [
        "control",
        "revisions",
        "control/namespace.id",
        "control/source-events.log",
        super::INSTALLATION_FILE,
        super::OWNER_SLOT_A,
        super::OWNER_SLOT_B,
        INTENT,
    ] {
        assert!(
            retained.contains_key(Path::new(name)),
            "missing published object {name}"
        );
    }
    let namespace = retained
        .get(Path::new("control/namespace.id"))
        .unwrap()
        .1
        .clone();

    let foreign = InitializationRequest::parse(FOREIGN_INITIALIZATION_ID).unwrap();
    let foreign_operation = request(
        root,
        "--recover-initialization",
        Some(FOREIGN_INITIALIZATION_ID),
    );
    match recover_initialization_request(root, &foreign, &foreign_operation) {
        Err(error) => assert_eq!(error, "OWNER_OPERATION_CONFLICT"),
        Ok(_) => panic!("foreign initialization identity recovered the retained layout"),
    }
    assert_eq!(
        snapshot(root),
        retained,
        "foreign recovery changed retained crash state"
    );

    let initialization = InitializationRequest::parse(COMPLETE_INITIALIZATION_ID).unwrap();
    let recovery = request(
        root,
        "--recover-initialization",
        Some(COMPLETE_INITIALIZATION_ID),
    );
    let receipt = recover_initialization_request(root, &initialization, &recovery).unwrap();
    assert!(receipt.replayed);
    assert_eq!(
        crate::sha256::decode_digest(std::str::from_utf8(&namespace).unwrap().trim_end()).unwrap(),
        receipt.namespace,
    );
    assert!(!root.join(INTENT).exists());

    let released = super::newest_valid(root).unwrap().1.unwrap();
    assert_eq!(released.epoch, 1);
    assert_eq!(released.generation, 3);
    assert_eq!(released.lifecycle, super::LifecycleState::Released);

    // Finalization changes lifecycle slots and removes the exact intent. Every
    // other retained layout object keeps its original bytes and timestamps.
    let preserve_layout = |mut tree: crate::root_draining_266::TreeSnapshot| {
        for name in ["", INTENT, super::OWNER_SLOT_A, super::OWNER_SLOT_B] {
            tree.remove(Path::new(name));
        }
        tree
    };
    let expected_layout = preserve_layout(retained);
    assert_eq!(preserve_layout(snapshot(root)), expected_layout);
    let inspection = request(root, "--health-data-root", None);
    DataRootGuard::with_inspection_request(root, &inspection, |_| Ok(())).unwrap();
    assert_eq!(preserve_layout(snapshot(root)), expected_layout);

    let mutation = request(root, "--serve-data-root", None);
    let mut owner = DataRootGuard::open_existing_request(root, &mutation).unwrap();
    assert_eq!(owner.epoch(), 2);
    owner
        .begin_drain(search_runtime_owner::DrainReason::Shutdown)
        .unwrap();
    owner.release_cleanly().unwrap();
    assert_eq!(preserve_layout(snapshot(root)), expected_layout);
}

fn acquire_in_child(root: &Path) -> ! {
    let initialization = InitializationRequest::parse(INITIALIZATION_ID).unwrap();
    let operation = request(root, "--initialize-data-root", Some(INITIALIZATION_ID));
    let cap = acquire_new(root, &initialization, &operation).unwrap();
    cap.verify().unwrap();
    // This is the actual production phase immediately before DIRECT creation.
    assert!(!root.join("control").exists());
    assert!(!root.join("revisions").exists());
    let mut output = io::stdout().lock();
    output.write_all(b"\n").unwrap();
    output.write_all(READY).unwrap();
    output.flush().unwrap();
    drop(output);
    let mut unexpected = [0_u8; 1];
    let result = io::stdin().read(&mut unexpected);
    // Keep all actual native exclusions alive until the parent kills us.
    let retained_namespace = cap.namespace_id();
    panic!("initialization child resumed: {result:?}, namespace={retained_namespace:?}");
}

fn exact_test_name() -> &'static str {
    concat!(
        module_path!(),
        "::partial_initialization_crash_preserves_fence"
    )
    .split_once("::")
    .unwrap()
    .1
}

#[test]
fn partial_initialization_crash_preserves_fence() {
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        acquire_in_child(Path::new(&root));
    }

    let scratch = Scratch::new();
    let root = &scratch.0;
    let mut child = NativeCrashChild::start(root, CHILD_ROOT, exact_test_name(), READY);
    child.crash();
    drop(child);

    let names = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        names,
        [INTENT, PRIMARY, SEALED]
            .map(OsString::from)
            .into_iter()
            .collect(),
        "the child must die before layout/installation/owner publication"
    );
    let intent = fs::read_to_string(root.join(INTENT)).unwrap();
    assert!(
        intent
            .lines()
            .any(|line| line == format!("operation_id={INITIALIZATION_ID}"))
    );
    assert!(intent.lines().any(|line| line.starts_with("owner_pid=")));
    let retained = snapshot(root);

    let initialization = InitializationRequest::parse(INITIALIZATION_ID).unwrap();
    let recovery = request(root, "--recover-initialization", Some(INITIALIZATION_ID));
    match recover_initialization_request(root, &initialization, &recovery) {
        Err(error) => assert_eq!(error, "OWNER_RECOVERY_EVIDENCE_MISSING"),
        Ok(_) => panic!("partial layout was recreated by named recovery"),
    }
    assert_eq!(
        snapshot(root),
        retained,
        "recovery changed retained crash state"
    );

    let initialize = request(root, "--initialize-data-root", Some(INITIALIZATION_ID));
    match initialize_new_request(root, &initialization, &initialize) {
        Err(error) => assert_eq!(error, "DATA_ROOT_RECOVERY_REQUIRED"),
        Ok(_) => panic!("partial initialization was replayed as a new create"),
    }
    assert_eq!(
        snapshot(root),
        retained,
        "initialization retry changed crash state"
    );

    let mutation = request(root, "--serve-data-root", None);
    match DataRootGuard::open_existing_request(root, &mutation) {
        Err(error) => assert_eq!(error, "DATA_ROOT_RECOVERY_REQUIRED"),
        Ok(_) => panic!("partial initialization admitted ordinary mutation"),
    }
    assert_eq!(
        snapshot(root),
        retained,
        "mutation changed retained crash state"
    );

    let inspection = request(root, "--health-data-root", None);
    let result: Result<(), String> =
        DataRootGuard::with_inspection_request(root, &inspection, |_| {
            panic!("partial initialization admitted inspection")
        });
    assert_eq!(result.unwrap_err(), "DATA_ROOT_RECOVERY_REQUIRED");
    assert_eq!(
        snapshot(root),
        retained,
        "inspection changed retained crash state"
    );
}
