//! Ownership regression for production standalone-grant entropy and time.

use std::fs;
use std::path::Path;

#[test]
fn grant_runtime_adapters_share_one_native_entropy_owner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member");
    let daemon = root.join("bins/eliot-searchd/src");
    let entry = read(&daemon.join("entry.rs"));
    let access = read(&daemon.join("access_composition.rs"));
    let grant = read(&daemon.join("access_composition/system_grant.rs"));
    let continuation = read(&daemon.join("continuation/kernel/entropy.rs"));
    let entropy = read(&daemon.join("qualified_entropy.rs"));

    assert!(entry.contains("mod qualified_entropy;"));
    assert!(access.contains("mod system_grant;"));
    assert!(access.contains("pub use system_grant::*;"));
    assert!(grant.contains("pub struct QualifiedGrantEntropy"));
    assert!(grant.contains("pub struct SystemGrantClock"));
    assert!(grant.contains("crate::qualified_entropy::fill_qualified_entropy"));
    assert!(grant.contains("SystemTime::now()"));
    assert!(continuation.contains("crate::qualified_entropy::qualified_entropy_32()"));
    assert!(!continuation.contains("/dev/urandom"));
    assert!(!continuation.contains("BCryptGenRandom"));
    assert!(entropy.contains("/dev/urandom"));
    assert!(entropy.contains("BCryptGenRandom"));

    let mut native_owners = Vec::new();
    collect_native_entropy_owners(root, &daemon, &mut native_owners);
    native_owners.sort();
    assert_eq!(
        native_owners,
        vec!["bins/eliot-searchd/src/qualified_entropy.rs".to_owned()]
    );
}

fn collect_native_entropy_owners(
    root: &Path,
    path: &Path,
    owners: &mut Vec<String>,
) {
    for entry in fs::read_dir(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    {
        let entry = entry.expect("daemon source entry must be readable");
        let current = entry.path();
        let metadata = fs::symlink_metadata(&current)
            .unwrap_or_else(|error| panic!("{}: {error}", current.display()));
        assert!(
            !metadata.file_type().is_symlink(),
            "daemon source symlink is not admitted: {}",
            current.display()
        );
        if metadata.is_dir() {
            collect_native_entropy_owners(root, &current, owners);
            continue;
        }
        if !metadata.is_file()
            || current.extension().and_then(|extension| extension.to_str())
                != Some("rs")
        {
            continue;
        }
        let text = read(&current);
        if text.contains("/dev/urandom") || text.contains("BCryptGenRandom") {
            let relative = current
                .strip_prefix(root)
                .expect("daemon file must remain inside repository")
                .to_string_lossy()
                .replace('\\', "/");
            owners.push(relative);
        }
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
