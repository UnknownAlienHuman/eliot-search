use super::*;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use search_runtime_owner::{DrainReason, OwnerError};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-owner-composition-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(fs::canonicalize(&root).unwrap())
    }

    fn establish(&self) -> LiveOwner {
        establish(&self.0).unwrap()
    }

    fn slot_bytes(&self, slot: Slot) -> Vec<u8> {
        fs::read(self.0.join(slot.file_name())).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn fresh_acquisition_starts_at_epoch_one_with_zero_predecessor() {
    let scratch = Scratch::new();
    let owner = scratch.establish();
    assert_eq!(owner.epoch().get(), 1);
    assert!(!owner.recovered_previous_active());
    assert_eq!(owner.record.previous_epoch, 0);
    assert_eq!(owner.record.previous_record_digest, [0; 32]);
    assert_eq!(owner.record.lifecycle, LifecycleState::Active);
    assert_eq!(owner.record.generation, 1);
    let stored = DurableOwnerRecord::decode(&scratch.slot_bytes(Slot::A)).unwrap();
    assert_eq!(stored, owner.record);
}

#[test]
fn record_encoding_round_trips_and_has_exact_shape() {
    let scratch = Scratch::new();
    let owner = scratch.establish();
    let encoded = owner.record.encode();
    assert!(encoded.len() <= MAX_STATE_BYTES);
    let text = String::from_utf8(encoded.clone()).unwrap();
    assert!(text.starts_with("ELIOT-SEARCH-OWNER-STATE-V1\nformat_version=1\n"));
    assert_eq!(text.lines().count(), 17);
    assert!(text.ends_with('\n'));
    assert_eq!(DurableOwnerRecord::decode(&encoded).unwrap(), owner.record);
}

#[test]
fn strict_decode_rejects_non_canonical_records() {
    let scratch = Scratch::new();
    let owner = scratch.establish();
    let canonical = String::from_utf8(owner.record.encode()).unwrap();
    let mut cases: Vec<String> = Vec::new();
    cases.push(canonical.trim_end().to_owned());
    cases.push(canonical.replacen(
        "ELIOT-SEARCH-OWNER-STATE-V1",
        "ELIOT-SEARCH-OWNER",
        1,
    ));
    cases.push(format!("{canonical}epoch=1\n"));
    cases.push(canonical.replacen("generation=", "generation_x=", 1));
    let digest_line = canonical
        .lines()
        .find(|line| line.starts_with("record_digest="))
        .unwrap();
    cases.push(canonical.replace(digest_line, &digest_line.to_uppercase()));
    cases.push(canonical.replacen("epoch=1\n", "epoch=01\n", 1));
    cases.push(canonical.replacen("epoch=1\n", "epoch=0\n", 1));
    let dropped = canonical
        .lines()
        .filter(|line| !line.starts_with("owner_pid="))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    cases.push(dropped);
    let mut tampered = owner.record.encode();
    let position = tampered.iter().position(|byte| *byte == b'=').unwrap() + 1;
    tampered[position] = if tampered[position] == b'0' {
        b'1'
    } else {
        b'0'
    };
    cases.push(String::from_utf8(tampered).unwrap());
    for (index, case) in cases.iter().enumerate() {
        assert!(
            DurableOwnerRecord::decode(case.as_bytes()).is_err(),
            "case {index} must be rejected"
        );
    }
}

#[test]
fn successor_advances_epoch_and_links_previous_digest() {
    let scratch = Scratch::new();
    let first = scratch.establish();
    let first_digest = first.record.record_digest;
    drop(first);
    let second = scratch.establish();
    assert_eq!(second.epoch().get(), 2);
    assert!(second.recovered_previous_active());
    assert_eq!(second.record.previous_epoch, 1);
    assert_eq!(second.record.previous_record_digest, first_digest);
    assert_ne!(second.record.owner_token, [0; 16]);
    let from_a = DurableOwnerRecord::decode(&scratch.slot_bytes(Slot::A)).unwrap();
    let from_b = DurableOwnerRecord::decode(&scratch.slot_bytes(Slot::B)).unwrap();
    assert_eq!(from_a.epoch, 1);
    assert_eq!(from_b.epoch, 2);
    assert_eq!(from_b.previous_record_digest, from_a.record_digest);
}

#[test]
fn installation_identity_is_stable_across_succession() {
    let scratch = Scratch::new();
    let first = scratch.establish();
    let (incarnation, root, _) = first.journal_owner_inputs();
    drop(first);
    let second = scratch.establish();
    let (incarnation_next, root_next, epoch_next) = second.journal_owner_inputs();
    assert_eq!(incarnation, incarnation_next);
    assert_eq!(root, root_next);
    assert_eq!(epoch_next.get(), 2);
}

#[test]
fn journal_inputs_survive_drain_release_for_cutover_replay() {
    let scratch = Scratch::new();
    let mut owner = scratch.establish();
    let (incarnation, root, epoch) = owner.journal_owner_inputs();
    owner.begin_drain(DrainReason::Shutdown).unwrap();
    owner.release_cleanly().unwrap();
    drop(owner);
    let next = scratch.establish();
    let (incarnation_next, root_next, epoch_next) = next.journal_owner_inputs();
    assert_eq!(incarnation, incarnation_next);
    assert_eq!(root, root_next);
    assert_eq!(epoch_next.get(), epoch.get() + 1);
}

#[test]
fn foreign_installation_denies_succession() {
    let first = Scratch::new();
    let _ = first.establish();
    let second = Scratch::new();
    let _ = second.establish();
    fs::copy(
        second.0.join(INSTALLATION_FILE),
        first.0.join(INSTALLATION_FILE),
    )
    .unwrap();
    assert!(matches!(
        establish(&first.0),
        Err(OwnerError::OwnerGuardMismatch)
    ));
}

#[test]
fn copied_state_files_deny_on_a_relocated_root() {
    let first = Scratch::new();
    let _ = first.establish();
    let second = Scratch::new();
    for name in [INSTALLATION_FILE, Slot::A.file_name()] {
        let bytes = fs::read(first.0.join(name)).unwrap();
        fs::write(second.0.join(name), &bytes).unwrap();
    }
    assert!(matches!(
        establish(&second.0),
        Err(OwnerError::OwnerGuardMismatch)
    ));
    assert!(!second.0.join(Slot::B.file_name()).exists());
}

#[test]
fn corrupt_slots_quarantine_without_repair() {
    let scratch = Scratch::new();
    let _ = scratch.establish();
    for slot in [Slot::A, Slot::B] {
        fs::write(
            scratch.0.join(slot.file_name()),
            b"corrupt-and-preserved",
        )
        .unwrap();
    }
    assert!(matches!(
        establish(&scratch.0),
        Err(OwnerError::OwnerRecoveryQuarantined)
    ));
    assert_eq!(
        fs::read(scratch.0.join(Slot::A.file_name())).unwrap(),
        b"corrupt-and-preserved"
    );
}

#[test]
fn torn_non_authority_slot_heals_by_guarded_succession() {
    let scratch = Scratch::new();
    let _ = scratch.establish();
    fs::write(scratch.0.join(Slot::B.file_name()), b"torn-write").unwrap();
    let next = scratch.establish();
    assert_eq!(next.epoch().get(), 2);
    assert!(next.recovered_previous_active());
    let healed = DurableOwnerRecord::decode(&scratch.slot_bytes(Slot::B)).unwrap();
    assert_eq!(healed.epoch, 2);
    assert_eq!(healed, next.record);
    let authority = DurableOwnerRecord::decode(&scratch.slot_bytes(Slot::A)).unwrap();
    assert_eq!(authority.epoch, 1);
}

#[test]
fn drain_release_lifecycle_is_guarded_and_idempotent() {
    let scratch = Scratch::new();
    let mut owner = scratch.establish();
    assert_eq!(owner.release_cleanly(), Err(OwnerError::OwnerDrainRequired));
    owner.begin_drain(DrainReason::Shutdown).unwrap();
    let generation = owner.record.generation;
    owner.begin_drain(DrainReason::Shutdown).unwrap();
    assert_eq!(owner.record.generation, generation);
    let receipt = owner.release_cleanly().unwrap();
    assert_eq!(receipt.epoch.get(), 1);
    assert_eq!(receipt.generation, generation + 1);
    assert_eq!(receipt.record_digest, owner.record.record_digest);
    assert_eq!(owner.release_cleanly(), Err(OwnerError::OwnerDrainRequired));
    assert_eq!(
        owner.begin_drain(DrainReason::Shutdown),
        Err(OwnerError::OwnerInvalidTransition)
    );
    let stored = [Slot::A, Slot::B]
        .into_iter()
        .filter_map(|slot| DurableOwnerRecord::decode(&scratch.slot_bytes(slot)).ok())
        .max_by_key(|record| record.generation)
        .unwrap();
    assert_eq!(stored.lifecycle, LifecycleState::Released);
    assert_eq!(stored, owner.record);
}

#[test]
fn clean_tombstone_clears_the_recovery_flag_for_the_successor() {
    let scratch = Scratch::new();
    let mut owner = scratch.establish();
    owner.begin_drain(DrainReason::Restart).unwrap();
    owner.release_cleanly().unwrap();
    drop(owner);
    let next = scratch.establish();
    assert_eq!(next.epoch().get(), 2);
    assert!(!next.recovered_previous_active());
}

#[test]
fn poisoned_guard_reports_unknown_instead_of_mutating() {
    let scratch = Scratch::new();
    let mut owner = scratch.establish();
    owner.poisoned = true;
    assert_eq!(
        owner.begin_drain(DrainReason::Shutdown),
        Err(OwnerError::OwnerAcquireOutcomeUnknown)
    );
    assert_eq!(
        owner.release_cleanly(),
        Err(OwnerError::OwnerReleaseOutcomeUnknown)
    );
}

#[test]
fn journal_inputs_construct_a_valid_redb_identity_without_relabelling() {
    use search_contracts::Blake3Digest32;
    use search_control_redb::JournalIdentity;

    let scratch = Scratch::new();
    let owner = scratch.establish();
    let (installation_incarnation_id, data_root_id, owner_epoch) =
        owner.journal_owner_inputs();
    let identity = JournalIdentity {
        installation_incarnation_id,
        data_root_id,
        owner_epoch,
        path_identity_digest: Blake3Digest32::from_bytes([1; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([2; 32]),
        schema_version: 1,
    };
    assert_eq!(identity.validate().unwrap(), identity);
    assert_eq!(identity.owner_epoch.get(), 1);
}

#[test]
fn root_derivation_is_stable_and_path_sensitive() {
    let first = Scratch::new();
    let second = Scratch::new();
    let left = observe_physical_root(&first.0).unwrap();
    let again = observe_physical_root(&first.0).unwrap();
    assert_eq!(left.data_root_id, again.data_root_id);
    assert_eq!(left.canonical_path_digest, again.canonical_path_digest);
    let right = observe_physical_root(&second.0).unwrap();
    assert_ne!(left.data_root_id, right.data_root_id);
    assert_ne!(left.canonical_path_digest, right.canonical_path_digest);
}

#[test]
fn executable_binding_is_stable_and_sized() {
    let first = observe_executable().unwrap();
    let second = observe_executable().unwrap();
    assert_eq!(first, second);
    assert_eq!(first.len(), 32);
}

#[test]
fn absent_sealed_mirror_passes_agreement() {
    let scratch = Scratch::new();
    assert!(verify_sealed_head_agrees(&scratch.0).is_ok());
}

#[test]
fn owner_debug_redacts_the_creation_token() {
    let scratch = Scratch::new();
    let owner = scratch.establish();
    let debug = format!("{owner:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains(&hex(&owner.record.owner_token)));
    let record_debug = format!("{:?}", owner.record);
    assert!(record_debug.contains("<redacted>"));
}
