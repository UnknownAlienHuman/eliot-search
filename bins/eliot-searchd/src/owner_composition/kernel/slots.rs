//! Alternating-slot read, selection, publication and transition readback.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use search_runtime_owner::OwnerError;

use super::codec::sync_directory;
use super::record::DurableOwnerRecord;
use super::spec::{MAX_STATE_BYTES, Slot};

/// Raw per-slot read outcome, keeping absence distinct from corruption.
///
/// The valid payload is boxed: slots are usually missing, and the record is
/// hundreds of bytes next to fieldless outcomes.
enum SlotRead {
    Missing,
    Valid(Box<DurableOwnerRecord>),
    Unreadable,
}

pub(super) fn read_slot(
    canonical_root: &Path,
    slot: Slot,
) -> Option<Box<DurableOwnerRecord>> {
    match read_slot_raw(canonical_root, slot) {
        SlotRead::Valid(record) => Some(record),
        SlotRead::Missing | SlotRead::Unreadable => None,
    }
}

fn read_slot_raw(canonical_root: &Path, slot: Slot) -> SlotRead {
    let bytes = match fs::read(canonical_root.join(slot.file_name())) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return SlotRead::Missing;
        }
        Err(_) => return SlotRead::Unreadable,
    };
    DurableOwnerRecord::decode(&bytes).map_or(SlotRead::Unreadable, |record| {
        SlotRead::Valid(Box::new(record))
    })
}

/// Selects the write target and the validated predecessor.
///
/// Any unreadable slot or same-generation conflict quarantines; only two
/// missing slots mean a fresh root.
pub(super) fn newest_valid(
    canonical_root: &Path,
) -> Result<(Slot, Option<Box<DurableOwnerRecord>>), OwnerError> {
    let first = read_slot_raw(canonical_root, Slot::A);
    let second = read_slot_raw(canonical_root, Slot::B);
    match (first, second) {
        (SlotRead::Valid(a), SlotRead::Valid(b)) => {
            if a.generation == b.generation && a != b {
                return Err(OwnerError::OwnerRecoveryQuarantined);
            }
            if a.generation >= b.generation {
                Ok((Slot::B, Some(a)))
            } else {
                Ok((Slot::A, Some(b)))
            }
        }
        (SlotRead::Valid(record), _) => Ok((Slot::B, Some(record))),
        (_, SlotRead::Valid(record)) => Ok((Slot::A, Some(record))),
        (SlotRead::Missing, SlotRead::Missing) => Ok((Slot::A, None)),
        _ => Err(OwnerError::OwnerRecoveryQuarantined),
    }
}

/// Publishes one slot with exact byte readback.
///
/// Pre-publication storage failures report the root unusable; a missing or
/// contradictory readback reports the unknown outcome or digest mismatch.
pub(super) fn write_slot(
    canonical_root: &Path,
    slot: Slot,
    record: &DurableOwnerRecord,
) -> Result<(), OwnerError> {
    let expected = record.encode();
    if expected.len() > MAX_STATE_BYTES {
        return Err(OwnerError::DataRootInvalid);
    }
    let path = canonical_root.join(slot.file_name());
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|_| OwnerError::DataRootInvalid)?;
    file.write_all(&expected)
        .map_err(|_| OwnerError::DataRootInvalid)?;
    file.sync_all().map_err(|_| OwnerError::DataRootInvalid)?;
    drop(file);
    sync_directory(canonical_root);
    match fs::read(&path) {
        Ok(actual) if actual == expected => Ok(()),
        Ok(_) => Err(OwnerError::OwnerRecordDigestMismatch),
        Err(_) => Err(OwnerError::OwnerAcquireOutcomeUnknown),
    }
}

/// Publishes a drain/release transition over a verified live record.
///
/// A predecessor that no longer matches the live guard quarantines instead
/// of overwriting foreign state.
pub(super) fn publish_transition(
    canonical_root: &Path,
    current: &DurableOwnerRecord,
    next: &DurableOwnerRecord,
) -> Result<(), OwnerError> {
    let (target, prior) = newest_valid(canonical_root)?;
    let Some(previous) = prior else {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    };
    if previous.as_ref() != current {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    write_slot(canonical_root, target, next)?;
    let reloaded =
        read_slot(canonical_root, target).ok_or(OwnerError::OwnerReleaseOutcomeUnknown)?;
    if *reloaded != *next {
        return Err(OwnerError::OwnerRecordDigestMismatch);
    }
    Ok(())
}
