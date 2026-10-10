//! Actual DIRECT child checkpoints, separate from root-admission qualification.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "../src/catalog_presence.rs"]
mod catalog_presence;
#[path = "../src/directory_manifest/kernel/codec.rs"]
pub mod codec;
#[path = "../src/directory_manifest/kernel/load.rs"]
pub mod load;
#[path = "../src/maintenance.rs"]
mod maintenance;
#[path = "../src/maintenance_guard.rs"]
mod maintenance_guard;
#[path = "../src/directory_manifest/kernel/model.rs"]
mod model;
#[path = "../src/owner_composition/kernel/operation.rs"]
mod operation;
#[path = "../src/directory_manifest/kernel/paths.rs"]
pub mod paths;
#[path = "../src/directory_manifest/kernel/persist.rs"]
pub mod persist;
#[path = "../src/qualified_entropy.rs"]
mod qualified_entropy;
#[path = "../src/sha256.rs"]
mod sha256;
#[path = "../src/directory_manifest/kernel/spec.rs"]
pub mod spec;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "eliot-children-266-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn request(&self) -> operation::DataRootRequest {
        operation::DataRootRequest::from_cli(&[
            "--fixture-child".into(),
            self.0.as_os_str().to_owned(),
        ])
        .unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let root = fs::canonicalize(&self.0).unwrap();
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        assert_eq!(root.parent(), Some(temporary.as_path()));
        fs::remove_dir_all(root).unwrap();
    }
}

fn gc_fixture(root: &Path) -> [PathBuf; 2] {
    fs::create_dir(root.join("control")).unwrap();
    fs::write(
        root.join("control/namespace.id"),
        format!("{}\n", "11".repeat(32)),
    )
    .unwrap();
    fs::write(
        root.join("control/source-events.log"),
        "ELIOT_SEARCH_SOURCE_EVENTS_V1\n",
    )
    .unwrap();
    let shard = root.join("revisions/aa");
    fs::create_dir_all(&shard).unwrap();
    let files = [
        shard.join(format!("{}.bin", "aa".repeat(32))),
        shard.join(format!("aa{}.bin", "bb".repeat(31))),
    ];
    for path in &files {
        fs::write(path, b"orphan").unwrap();
    }
    files
}

#[test]
fn cancelled_gc_does_not_begin_reads_or_deletions() {
    let root = Scratch::new();
    let files = gc_fixture(&root.0);
    let request = root.request();
    request.cancel();
    assert_eq!(
        maintenance_guard::guarded_collect_orphan_revisions_with_check(&root.0, true, &|| request
            .preflight(),)
        .unwrap_err(),
        "OWNER_CANCELLED_BEFORE_MUTATION"
    );
    for path in files {
        assert_eq!(fs::read(path).unwrap(), b"orphan");
    }
}

#[test]
fn gc_cancellation_after_one_delete_preserves_later_objects() {
    let root = Scratch::new();
    let files = gc_fixture(&root.0);
    let request = root.request();
    let result =
        maintenance_guard::guarded_collect_orphan_revisions_with_check(&root.0, true, &|| {
            if !files[0].exists() {
                request.cancel();
            }
            request.preflight()
        });
    assert_eq!(result.unwrap_err(), "OWNER_CANCELLED_BEFORE_MUTATION");
    assert!(!files[0].exists());
    assert_eq!(fs::read(&files[1]).unwrap(), b"orphan");
}

#[test]
fn live_request_gc_still_previews_and_deletes_only_generated_orphans() {
    let root = Scratch::new();
    let files = gc_fixture(&root.0);
    let request = root.request();
    let preview =
        maintenance_guard::guarded_collect_orphan_revisions_with_check(&root.0, false, &|| {
            request.preflight()
        })
        .unwrap();
    assert_eq!(preview.orphan_objects, 2);
    assert_eq!(preview.deleted_objects, 0);
    let applied =
        maintenance_guard::guarded_collect_orphan_revisions_with_check(&root.0, true, &|| {
            request.preflight()
        })
        .unwrap();
    assert_eq!(applied.deleted_objects, 2);
    for path in files {
        assert!(!path.exists());
    }
}

fn manifest_fixture(root: &Path) -> model::DirectoryManifest {
    let entries = std::iter::once(model::DirectoryEntry {
        source_id: "33".repeat(32),
        path_digest: "44".repeat(32),
        revision_id: "55".repeat(32),
    })
    .map(|entry| (entry.source_id.clone(), entry))
    .collect::<BTreeMap<_, _>>();
    let manifest = codec::build_manifest("11".repeat(32), "22".repeat(32), 1, entries).unwrap();
    fs::write(
        codec::manifest_path(root, &manifest),
        codec::encode_manifest(&manifest).unwrap(),
    )
    .unwrap();
    manifest
}

#[test]
fn manifest_read_and_publish_refuse_cancelled_request_without_writes() {
    let root = Scratch::new();
    let manifest = manifest_fixture(&root.0);
    let path = codec::manifest_path(&root.0, &manifest);
    let before = fs::read(&path).unwrap();
    let request = root.request();
    request.cancel();
    assert_eq!(
        load::load_manifest_file_with_check(&path, &|| request.preflight()).unwrap_err(),
        "OWNER_CANCELLED_BEFORE_MUTATION"
    );
    let next = codec::build_manifest(
        manifest.namespace_id,
        manifest.directory_digest,
        2,
        manifest.entries,
    )
    .unwrap();
    assert_eq!(
        persist::persist_manifest(&root.0, &next, &|| request.preflight()).unwrap_err(),
        "OWNER_CANCELLED_BEFORE_MUTATION"
    );
    assert_eq!(fs::read(path).unwrap(), before);
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
}

#[test]
fn manifest_readback_uses_actual_codec_and_original_context() {
    let root = Scratch::new();
    let manifest = manifest_fixture(&root.0);
    let request = root.request();
    assert_eq!(
        load::load_manifest_file_with_check(&codec::manifest_path(&root.0, &manifest), &|| request
            .preflight())
        .unwrap(),
        manifest
    );
    request.retain().cancel();
    assert_eq!(
        load::load_latest_manifest(
            &root.0,
            &manifest.namespace_id,
            &manifest.directory_digest,
            &|| request.preflight()
        )
        .unwrap_err(),
        "OWNER_CANCELLED_BEFORE_MUTATION"
    );
}

#[test]
fn cancellation_after_manifest_creation_retains_pending_attempt() {
    let root = Scratch::new();
    let manifest = manifest_fixture(&root.0);
    let next = codec::build_manifest(
        manifest.namespace_id,
        manifest.directory_digest,
        2,
        manifest.entries,
    )
    .unwrap();
    let request = root.request();
    let pending = || {
        fs::read_dir(&root.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().is_some_and(|extension| extension == "tmp"))
    };
    let result = persist::persist_manifest(&root.0, &next, &|| {
        if pending().is_some() {
            request.cancel();
        }
        request.preflight()
    });
    assert_eq!(result.unwrap_err(), "OWNER_CANCELLED_BEFORE_MUTATION");
    assert_eq!(fs::read(pending().unwrap()).unwrap(), b"");
    assert!(!codec::manifest_path(&root.0, &next).exists());
}

#[test]
fn failed_manifest_rename_retains_exact_attempted_bytes() {
    let root = Scratch::new();
    let manifest = manifest_fixture(&root.0);
    let next = codec::build_manifest(
        manifest.namespace_id,
        manifest.directory_digest,
        2,
        manifest.entries,
    )
    .unwrap();
    let final_path = codec::manifest_path(&root.0, &next);
    let request = root.request();
    let pending = || {
        fs::read_dir(&root.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().is_some_and(|extension| extension == "tmp"))
    };
    let result = persist::persist_manifest(&root.0, &next, &|| {
        if pending().is_some_and(|path| fs::metadata(path).unwrap().len() > 0)
            && !final_path.exists()
        {
            // Force an actual filesystem rename failure after the synced write.
            fs::create_dir(&final_path).unwrap();
        }
        request.preflight()
    });
    assert_eq!(result.unwrap_err(), "DIRECT_MANIFEST_FILE_INVALID");
    assert_eq!(
        fs::read(pending().unwrap()).unwrap(),
        codec::encode_manifest(&next).unwrap().as_bytes()
    );
    assert!(final_path.is_dir());
}
