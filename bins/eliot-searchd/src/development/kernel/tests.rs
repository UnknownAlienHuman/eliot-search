use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch {
    base: PathBuf,
    data: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-owner-guard-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let data = base.join("data");
        fs::create_dir_all(&data).unwrap();
        Self { base, data }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn registration_reopens_under_the_same_exclusive_owner_lock() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!(
        "eliot-owner-{}-{stamp}",
        std::process::id()
    ));
    let data = base.join("data");
    let source = base.join("source");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir(&source).unwrap();
    {
        let mut owner = DataRootGuard::acquire(&data).unwrap();
        owner.source_roots_mut().add(&source).unwrap();
        assert!(matches!(
            DataRootGuard::acquire(&data),
            Err(error) if error == "DATA_ROOT_ALREADY_OWNED"
        ));
    }
    {
        let owner = DataRootGuard::acquire(&data).unwrap();
        assert_eq!(owner.source_roots().configured_count(), 1);
        assert_eq!(owner.source_roots().available_count(), 1);
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn single_guard_binds_epoch_and_stable_identities_across_succession() {
    let scratch = Scratch::new();
    let (incarnation, root, _) = {
        let guard = DataRootGuard::acquire(&scratch.data).unwrap();
        assert_eq!(guard.epoch(), 1);
        assert!(!guard.recovered_previous_active());
        guard.journal_owner_inputs()
    };
    {
        let guard = DataRootGuard::acquire(&scratch.data).unwrap();
        assert_eq!(guard.epoch(), 2);
        assert!(guard.recovered_previous_active());
        let (incarnation_next, root_next, epoch_next) = guard.journal_owner_inputs();
        assert_eq!(incarnation, incarnation_next);
        assert_eq!(root, root_next);
        assert_eq!(epoch_next.get(), 2);
    }
}

#[test]
fn guard_stays_live_until_released_then_successor_advances() {
    let scratch = Scratch::new();
    let first = DataRootGuard::acquire(&scratch.data).unwrap();
    assert!(matches!(
        DataRootGuard::acquire(&scratch.data),
        Err(error) if error == "DATA_ROOT_ALREADY_OWNED"
    ));
    drop(first);
    let second = DataRootGuard::acquire(&scratch.data).unwrap();
    assert_eq!(second.epoch(), 2);
}

#[test]
fn release_requires_prior_drain_and_persists_one_tombstone() {
    use search_runtime_owner::OwnerError;

    let scratch = Scratch::new();
    let guard = DataRootGuard::acquire(&scratch.data).unwrap();
    assert_eq!(
        guard.release_cleanly().map(|_| ()),
        Err(OwnerError::OwnerDrainRequired.code().to_owned())
    );
    let mut guard = DataRootGuard::acquire(&scratch.data).unwrap();
    // The refused release above wrote nothing: the next incarnation
    // still succeeds the dropped (unreleased) guard at epoch two.
    assert_eq!(guard.epoch(), 2);
    guard
        .begin_drain(search_runtime_owner::DrainReason::Shutdown)
        .unwrap();
    let receipt = guard.release_cleanly().unwrap();
    assert_eq!(receipt.epoch.get(), 2);
    let next = DataRootGuard::acquire(&scratch.data).unwrap();
    assert_eq!(next.epoch(), 3);
    assert!(!next.recovered_previous_active());
}

#[test]
fn relocated_copy_is_denied_while_original_advances() {
    let scratch = Scratch::new();
    drop(DataRootGuard::acquire(&scratch.data).unwrap());
    let moved = scratch.base.join("moved");
    fs::create_dir(&moved).unwrap();
    for name in [
        ".eliot-search-installation.v1",
        ".eliot-search-owner-state-a.v1",
    ] {
        let bytes = fs::read(scratch.data.join(name)).unwrap();
        fs::write(moved.join(name), &bytes).unwrap();
    }
    assert!(matches!(
        DataRootGuard::acquire(&moved),
        Err(error) if error == "OWNER_GUARD_MISMATCH"
    ));
    // The denied copy wrote no successor slot of its own.
    assert!(!moved.join(".eliot-search-owner-state-b.v1").exists());
    let guard = DataRootGuard::acquire(&scratch.data).unwrap();
    assert_eq!(guard.epoch(), 2);
}

#[test]
fn corrupt_owner_state_quarantines_without_touching_catalogs() {
    let scratch = Scratch::new();
    drop(DataRootGuard::acquire(&scratch.data).unwrap());
    for name in [
        ".eliot-search-owner-state-a.v1",
        ".eliot-search-owner-state-b.v1",
    ] {
        fs::write(scratch.data.join(name), b"corrupt-and-preserved").unwrap();
    }
    assert!(matches!(
        DataRootGuard::acquire(&scratch.data),
        Err(error) if error == "OWNER_RECOVERY_QUARANTINED"
    ));
    assert_eq!(
        fs::read(scratch.data.join(".eliot-search-owner-state-a.v1")).unwrap(),
        b"corrupt-and-preserved"
    );
}

#[test]
fn lock_profiles_name_the_exact_owner_files_without_new_effects() {
    use search_domain::MutationOutcomeClass;
    use search_runtime_owner::OwnerError;

    assert_eq!(
        OwnerLockProfile::Direct.file_name(),
        ".eliot-search-owner.lock"
    );
    assert_eq!(
        OwnerLockProfile::MigrationOutput.file_name(),
        OwnerLockProfile::DIRECT_LOCK_FILE
    );
    assert_eq!(
        OwnerLockProfile::Sealed.file_name(),
        OwnerLockProfile::SEALED_LOCK_FILE
    );
    assert_eq!(
        OwnerLockProfile::Sealed.file_name(),
        ".eliot-search-sealed-owner.lock"
    );
    assert_eq!(
        live_owner_denial(None),
        OwnerError::DataRootAlreadyOwned.code()
    );
    assert_eq!(live_owner_denial(None), "DATA_ROOT_ALREADY_OWNED");
    assert_eq!(
        classify_ambiguous_owner_write(),
        MutationOutcomeClass::Unknown
    );
}

#[test]
fn lock_path_joins_the_named_file_under_a_canonical_root() {
    let root = Path::new("data-root");
    assert_eq!(
        OwnerLockProfile::Direct.lock_path(root),
        root.join(".eliot-search-owner.lock")
    );
    assert_eq!(
        OwnerLockProfile::MigrationOutput.lock_path(root),
        OwnerLockProfile::Direct.lock_path(root)
    );
}

/// Counts newline bytes with an explicit loop.
fn count_newlines(bytes: &[u8]) -> usize {
    let mut count = 0_usize;
    for byte in bytes {
        if *byte == b'\n' {
            count += 1;
        }
    }
    count
}

#[test]
fn shared_matcher_preserves_legacy_coordinates_and_ascii_only_folding() {
    for text in [
        "",
        "aaaaa",
        "aAéAa",
        "ΑαA\0a",
        "\n\nx\r\nX",
        "a\rb",
        "𐀀a𐀀",
        "a\r\nβ\nz",
    ] {
        for query in [
            "a", "aa", "aaa", "é", "α", "𐀀", "\n", "\r\n", "\0", "\nX", "absent",
        ] {
            for insensitive in [false, true] {
                // Independent bounded oracle; never called by runtime code.
                let expected = text
                    .as_bytes()
                    .windows(query.len())
                    .enumerate()
                    .filter(|(start, bytes)| {
                        text.is_char_boundary(*start)
                            && text.is_char_boundary(start + query.len())
                            && if insensitive {
                                bytes.eq_ignore_ascii_case(query.as_bytes())
                            } else {
                                *bytes == query.as_bytes()
                            }
                    })
                    .map(|(start, _)| {
                        let prefix = &text.as_bytes()[..start];
                        let line_start = prefix
                            .iter()
                            .rposition(|byte| *byte == b'\n')
                            .map_or(0, |index| index + 1);
                        ScanMatch {
                            byte_start: start,
                            byte_end: start + query.len(),
                            line: count_newlines(prefix),
                            column_bytes: start - line_start,
                        }
                    })
                    .collect::<Vec<_>>();
                let actual = scan_text(text, query, insensitive).unwrap();
                assert_eq!(
                    actual.matches, expected,
                    "text={text:?} query={query:?} insensitive={insensitive}"
                );
                assert_eq!(
                    actual.coverage,
                    ScanCoverage {
                        input_bytes: text.len(),
                        complete: true,
                        match_limit_reached: false,
                    }
                );
            }
        }
    }
}

#[test]
fn output_ceiling_is_incomplete_only_when_an_additional_match_exists() {
    for extra in [0, 1] {
        let text = "a".repeat(MAX_SCAN_MATCHES + extra);
        let actual = scan_text(&text, "a", false).unwrap();
        assert_eq!(actual.matches.len(), MAX_SCAN_MATCHES);
        assert_eq!(
            actual.matches.last().unwrap().byte_start,
            MAX_SCAN_MATCHES - 1
        );
        assert_eq!(actual.coverage.complete, extra == 0);
        assert_eq!(actual.coverage.match_limit_reached, extra != 0);
    }
}

#[test]
fn repeated_long_prefix_uses_the_shared_linear_matcher() {
    let count = 1024 * 1024;
    let text = format!("{}b", "a".repeat(count));
    let query = format!("{}b", "a".repeat(8192));
    let actual = scan_text(&text, &query, false).unwrap();
    assert_eq!(
        actual.matches,
        vec![ScanMatch {
            byte_start: count - 8192,
            byte_end: count + 1,
            line: 0,
            column_bytes: count - 8192,
        }]
    );
    assert!(actual.coverage.complete);
}

#[test]
fn caller_errors_still_use_the_existing_scan_namespace() {
    assert_eq!(
        scan_text("", "", false),
        Err("SCAN_QUERY_EMPTY".to_owned())
    );
    assert_eq!(
        scan_text("", &"a".repeat(MAX_SCAN_QUERY_BYTES + 1), false),
        Err("SCAN_QUERY_TOO_LARGE".to_owned())
    );
    assert_eq!(
        scan_text(&"a".repeat(MAX_SCAN_INPUT_BYTES + 1), "a", false),
        Err("SCAN_INPUT_TOO_LARGE".to_owned())
    );
}
