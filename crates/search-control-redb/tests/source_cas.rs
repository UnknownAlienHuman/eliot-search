//! A durable current-source CAS using native acquisition and an actual redb file.

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use search_contracts::Blake3Digest32;
use search_control_redb::current_source::SourceControlError;
use search_control_redb::plain_control::{ControlWriter, initialize_plain_control};
use search_control_redb::publication::PublicationError;
use search_safe_reader::{MAX_SOURCE_FILE_BYTES, acquire_file};
use search_source_registry::{NativeFileKey, ObjectRef};

fn acquired_reference(
    path: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
) -> (NativeFileKey, ObjectRef) {
    let (bytes, key, length) = acquire_file(path, MAX_SOURCE_FILE_BYTES, deadline, cancel)
        .expect("acquire the actual source file")
        .into_parts();
    let digest = Blake3Digest32::from_bytes(*blake3::hash(&bytes).as_bytes());
    (key, ObjectRef::source(digest, length))
}

#[test]
fn newer_source_survives_stale_acquisition_and_stale_publication_capture() {
    let scratch = tempfile::tempdir().expect("isolated native files");
    let control = scratch.path().join("control.redb");
    let selected = scratch.path().join("selected.rs");
    let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    fs::copy(sources.join("current_source.rs"), &selected).expect("select repository source");
    let deadline = Instant::now() + Duration::from_secs(20);
    let cancel = AtomicBool::new(false);
    initialize_plain_control(&control, deadline, &cancel).expect("initialize actual CONTROL");
    let writer = ControlWriter::open(&control, deadline, &cancel).expect("open CONTROL");
    let empty = writer
        .capture_corpus(deadline, &cancel)
        .expect("capture empty desired corpus");

    let capture = writer
        .capture_current_source(&selected, deadline, &cancel)
        .expect("capture the selected file");
    let (first_key, first_ref) = acquired_reference(&selected, deadline, &cancel);
    let first = writer
        .commit_current_source(capture, first_key, first_ref, deadline, &cancel)
        .expect("commit first acquired source");
    assert!(matches!(
        writer.start_build(&empty, deadline, &cancel),
        Err(PublicationError::Stale)
    ));
    assert!(
        writer
            .load_pending(deadline, &cancel)
            .expect("read pending")
            .is_none()
    );

    let before_update = writer
        .capture_current_source(&selected, deadline, &cancel)
        .expect("capture the same registration before update");
    let replacement =
        fs::read(sources.join("current_preparation.rs")).expect("read other real code");
    fs::write(&selected, replacement).expect("replace bytes in the same native file");
    let (second_key, second_ref) = acquired_reference(&selected, deadline, &cancel);
    assert_eq!(first_key, second_key);
    assert_ne!(first_ref, second_ref);
    let second = writer
        .commit_current_source(
            before_update.clone(),
            second_key,
            second_ref,
            deadline,
            &cancel,
        )
        .expect("commit the changed retained input");
    assert_eq!(second.source_id, first.source_id);
    assert_eq!(second.registration_revision, first.registration_revision);
    assert!(second.row_revision > first.row_revision);
    assert!(matches!(
        writer.commit_current_source(before_update, first_key, first_ref, deadline, &cancel),
        Err(SourceControlError::Stale)
    ));
    writer
        .close(deadline, &cancel)
        .expect("close committed CONTROL");

    let reopened = ControlWriter::open(&control, deadline, &cancel).expect("reopen from disk");
    let rows = reopened
        .current_source_page(None, deadline, &cancel)
        .expect("read authoritative source rows");
    assert_eq!(rows, vec![second]);
    reopened
        .close(deadline, &cancel)
        .expect("close reopened CONTROL");
}
