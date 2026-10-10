//! Alternating-slot read, selection, publication and transition readback.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use search_runtime_owner::OwnerError;

use super::codec::sync_directory;
use super::read_existing::read_existing_bytes;
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

pub(super) fn read_slot(canonical_root: &Path, slot: Slot) -> Option<Box<DurableOwnerRecord>> {
    match read_slot_raw(canonical_root, slot) {
        SlotRead::Valid(record) => Some(record),
        SlotRead::Missing | SlotRead::Unreadable => None,
    }
}

fn read_slot_raw(canonical_root: &Path, slot: Slot) -> SlotRead {
    let bytes = match read_existing_bytes(&canonical_root.join(slot.file_name()), MAX_STATE_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return SlotRead::Missing,
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
        (SlotRead::Unreadable, _) | (_, SlotRead::Unreadable) => {
            Err(OwnerError::OwnerRecoveryQuarantined)
        }
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
    let mut options = OpenOptions::new();
    options.create(cfg!(test)).write(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000).share_mode(0x3);
    }
    let mut file = options
        .open(&path)
        .map_err(|_| OwnerError::DataRootInvalid)?;
    super::read_existing::verify_existing_locator(&file, &path)?;
    #[cfg(not(test))]
    super::installation::verify_native_installation(canonical_root)?;
    file.set_len(0)
        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
    file.write_all(&expected)
        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
    file.sync_all()
        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
    drop(file);
    sync_directory(canonical_root);
    match read_existing_bytes(&path, MAX_STATE_BYTES) {
        Ok(Some(actual)) if actual == expected => Ok(()),
        Ok(Some(_)) => Err(OwnerError::OwnerRecordDigestMismatch),
        Ok(None) | Err(_) => Err(OwnerError::OwnerAcquireOutcomeUnknown),
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
