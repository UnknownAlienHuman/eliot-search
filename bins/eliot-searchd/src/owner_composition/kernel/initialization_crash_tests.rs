//! Kill the actual initialization owner after intent/exclusion acquisition.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use super::{
    DataRootRequest, INTENT, InitializationRequest, PRIMARY, SEALED, acquire_new,
    initialize_new_request, recover_initialization_request,
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
