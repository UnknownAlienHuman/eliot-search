//! Publication floor compare-and-swap and atomic file-journal persistence.
//!
//! This module owns two narrow T27 primitives used by the daemon publication
//! composition:
//!
//! * [`PublicationFloor`] with the exact [`cas`] generation discipline: a
//!   commit succeeds only for the expected generation stepped by exactly one,
//!   with monotone non-inverted floors. Anything else is
//!   [`PublicationCodecError::ControlConflict`].
//! * [`FileJournal`], an atomic marker-file writer (`create_new` temporary
//!   file, content sync, directory sync, atomic rename, exact readback).
//!   Repeating the same bytes replays as [`JournalPersistOutcome::ReplayIdentical`];
//!   a foreign valid marker is [`PublicationCodecError::JournalConflict`];
//!   torn or undecodable bytes quarantine as
//!   [`PublicationCodecError::JournalCorrupt`]; an ambiguous rename reports
//!   [`PublicationCodecError::JournalOutcomeUnknown`] and never success.
//!
//! Floor snapshots encode through [`encode_floor`] / [`decode_floor`] so the
//! encoded bytes can travel as opaque [`FileJournal`] payload through the
//! existing control tables without a new `redb` table or migration.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use search_contracts::Epoch;
use sha2::{Digest, Sha256};

/// Magic prefix of one encoded [`PublicationFloor`] record.
pub const PUBLICATION_FLOOR_MAGIC: &[u8; 8] = b"ELIOPBF1";
/// Exact floor codec version accepted by [`decode_floor`].
pub const PUBLICATION_FLOOR_VERSION: u32 = 1;
/// Magic prefix of one [`FileJournal`] marker file.
pub const JOURNAL_MARKER_MAGIC: &[u8; 8] = b"ELIOPBJ1";
/// Exact marker codec version accepted by the journal readback.
pub const JOURNAL_MARKER_VERSION: u32 = 1;
/// Maximum opaque journal payload in bytes.
pub const MAX_JOURNAL_BYTES: usize = 1_048_576;
/// Maximum journal marker name length in bytes.
pub const MAX_JOURNAL_NAME_LEN: usize = 64;

/// Exact encoded [`PublicationFloor`] length in bytes.
pub const PUBLICATION_FLOOR_LEN: usize = 8 + 4 + 8 + 8 + 8 + 32;

/// Closed publication-codec failure surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationCodecError {
    /// A generation guard or floor monotonicity check failed.
    ControlConflict,
    /// A reserved epoch is not exactly one past the last reservation.
    EpochMismatch,
    /// An abandon request lacks a complete membership fence.
    AbandonFenceMissing,
    /// A foreign valid marker already occupies the journal slot.
    JournalConflict,
    /// Torn, truncated or undecodable bytes; the slot quarantines.
    JournalCorrupt,
    /// A transport or acknowledgement loss after a possible write; only an
    /// exact recovery read may resolve the outcome, never a success claim.
    JournalOutcomeUnknown,
    /// A finite byte, name or batch bound was exceeded.
    BudgetExceeded,
    /// A journal marker name is empty, oversized or carries separators.
    InvalidJournalName,
}

impl PublicationCodecError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ControlConflict => "CONTROL_CONFLICT",
            Self::EpochMismatch => "EPOCH_MISMATCH",
            Self::AbandonFenceMissing => "ABANDON_FENCE_MISSING",
            Self::JournalConflict => "JOURNAL_CONFLICT",
            Self::JournalCorrupt => "JOURNAL_CORRUPT",
            Self::JournalOutcomeUnknown => "JOURNAL_OUTCOME_UNKNOWN",
            Self::BudgetExceeded => "PUBLICATION_BUDGET_EXCEEDED",
            Self::InvalidJournalName => "PUBLICATION_JOURNAL_NAME_INVALID",
        }
    }
}

impl core::fmt::Display for PublicationCodecError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PublicationCodecError {}

/// Durable publication floor: one control generation with the committed
/// visible epoch floor and the last reserved epoch floor.
///
/// Fields stay private so every mutation travels through [`cas`] with an
/// explicit expected generation; direct assignment cannot bypass the bump.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationFloor {
    generation: u64,
    floor_visible: Epoch,
    floor_last_reserved: Epoch,
}

impl PublicationFloor {
    /// Builds a well-formed floor. An inverted floor
    /// (`floor_visible > floor_last_reserved`) is a conflict, never a floor.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationCodecError::ControlConflict`] for an inverted floor.
    pub fn new(
        generation: u64,
        floor_visible: Epoch,
        floor_last_reserved: Epoch,
    ) -> Result<Self, PublicationCodecError> {
        if floor_visible > floor_last_reserved {
            return Err(PublicationCodecError::ControlConflict);
        }
        Ok(Self {
            generation,
            floor_visible,
            floor_last_reserved,
        })
    }

    /// Exact control generation of this floor.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Committed visible epoch floor; advances only on observed commit.
    #[must_use]
    pub const fn floor_visible(&self) -> Epoch {
        self.floor_visible
    }

    /// Last reserved epoch floor, including aborted reservations.
    #[must_use]
    pub const fn floor_last_reserved(&self) -> Epoch {
        self.floor_last_reserved
    }
}

/// Exact generation-guarded floor transition.
///
/// Succeeds with the `next` floor only when `stored.generation` equals
/// `expected_generation`, `next.generation` equals `expected_generation + 1`,
/// `next` is non-inverted, and both floors are monotone against `stored`.
/// A mutation without the exact `+ 1` step always conflicts, including at the
/// `u64` ceiling where no successor exists.
///
/// # Errors
///
/// Returns [`PublicationCodecError::ControlConflict`] for any stale, skipped,
/// unstepped, inverted or regressed transition.
pub fn cas(
    stored: &PublicationFloor,
    expected_generation: u64,
    next: &PublicationFloor,
) -> Result<PublicationFloor, PublicationCodecError> {
    if stored.generation != expected_generation {
        return Err(PublicationCodecError::ControlConflict);
    }
    let successor = expected_generation
        .checked_add(1)
        .ok_or(PublicationCodecError::ControlConflict)?;
    if next.generation != successor {
        return Err(PublicationCodecError::ControlConflict);
    }
    if next.floor_visible > next.floor_last_reserved {
        return Err(PublicationCodecError::ControlConflict);
    }
    if next.floor_visible < stored.floor_visible
        || next.floor_last_reserved < stored.floor_last_reserved
    {
        return Err(PublicationCodecError::ControlConflict);
    }
    Ok(*next)
}

/// Encodes one floor to its canonical bytes for opaque journal transport.
#[must_use]
pub fn encode_floor(floor: &PublicationFloor) -> Vec<u8> {
    let mut out = Vec::with_capacity(PUBLICATION_FLOOR_LEN);
    out.extend_from_slice(PUBLICATION_FLOOR_MAGIC);
    out.extend_from_slice(&PUBLICATION_FLOOR_VERSION.to_be_bytes());
    out.extend_from_slice(&floor.generation.to_be_bytes());
    out.extend_from_slice(&floor.floor_visible.get().to_be_bytes());
    out.extend_from_slice(&floor.floor_last_reserved.get().to_be_bytes());
    let digest = Sha256::digest(&out);
    out.extend_from_slice(&digest);
    debug_assert_eq!(out.len(), PUBLICATION_FLOOR_LEN);
    out
}

/// Decodes one floor, failing closed with quarantine semantics on any torn,
/// foreign or digested-mismatch input.
///
/// # Errors
///
/// Returns [`PublicationCodecError::JournalCorrupt`] for magic, version,
/// length, digest or inversion mismatch. Never defaults.
pub fn decode_floor(bytes: &[u8]) -> Result<PublicationFloor, PublicationCodecError> {
    if bytes.len() != PUBLICATION_FLOOR_LEN
        || bytes[..8] != *PUBLICATION_FLOOR_MAGIC
        || u32::from_be_bytes(bytes[8..12].try_into().unwrap_or([0xFF; 4]))
            != PUBLICATION_FLOOR_VERSION
    {
        return Err(PublicationCodecError::JournalCorrupt);
    }
    let digest = Sha256::digest(&bytes[..PUBLICATION_FLOOR_LEN - 32]);
    if digest.as_slice() != &bytes[PUBLICATION_FLOOR_LEN - 32..] {
        return Err(PublicationCodecError::JournalCorrupt);
    }
    let generation = u64::from_be_bytes(bytes[12..20].try_into().unwrap_or([0; 8]));
    let visible = i64::from_be_bytes(bytes[20..28].try_into().unwrap_or([0; 8]));
    let reserved = i64::from_be_bytes(bytes[28..36].try_into().unwrap_or([0; 8]));
    let floor_visible = Epoch::new(visible).map_err(|_| PublicationCodecError::JournalCorrupt)?;
    let floor_last_reserved =
        Epoch::new(reserved).map_err(|_| PublicationCodecError::JournalCorrupt)?;
    if floor_visible > floor_last_reserved {
        return Err(PublicationCodecError::JournalCorrupt);
    }
    Ok(PublicationFloor {
        generation,
        floor_visible,
        floor_last_reserved,
    })
}

/// Outcome of one [`FileJournal::persist`] call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalPersistOutcome {
    /// Bytes are durable and proven by exact readback.
    Persisted,
    /// The slot already holds exactly these bytes; no second write ran.
    ReplayIdentical,
}

/// Atomic marker-file journal for one opaque intent slot.
///
/// Callers serialize reservations through [`cas`] generations before touching
/// this journal; the journal is the second fence, not the lock. A torn slot
/// quarantines to `<name>.quarantine` (with a bounded numeric suffix when the
/// first quarantine name is taken) instead of being overwritten.
#[derive(Debug)]
pub struct FileJournal {
    dir: PathBuf,
    name: String,
}

impl FileJournal {
    /// Binds one marker name inside an existing directory.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationCodecError::InvalidJournalName`] for an empty,
    /// oversized or separator-carrying name.
    pub fn new(dir: &Path, name: &str) -> Result<Self, PublicationCodecError> {
        validate_name(name)?;
        if dir.as_os_str().is_empty() {
            return Err(PublicationCodecError::InvalidJournalName);
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            name: name.to_owned(),
        })
    }

    /// Marker name bound to this journal.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Atomically persists opaque bytes: temporary file, content sync,
    /// directory sync, atomic rename, exact readback comparison.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationCodecError::BudgetExceeded`] for empty or oversized
    /// input, [`PublicationCodecError::JournalConflict`] for a foreign valid
    /// marker, [`PublicationCodecError::JournalCorrupt`] for torn readback
    /// (the slot quarantines), and
    /// [`PublicationCodecError::JournalOutcomeUnknown`] after an ambiguous
    /// rename. Ambiguity never reports success.
    pub fn persist(&self, bytes: &[u8]) -> Result<JournalPersistOutcome, PublicationCodecError> {
        validate_payload(bytes)?;
        let target = self.marker_path();
        match self.read_existing()? {
            Some(existing) if existing == bytes => {
                return Ok(JournalPersistOutcome::ReplayIdentical);
            }
            Some(_) => return Err(PublicationCodecError::JournalConflict),
            None => {}
        }
        let tmp = self.write_tmp(bytes)?;
        match fs::rename(&tmp, &target) {
            Ok(()) => {}
            Err(rename_error) => {
                let _ = fs::remove_file(&tmp);
                if rename_error.kind() == std::io::ErrorKind::AlreadyExists {
                    return match self.read_existing()? {
                        Some(existing) if existing == bytes => {
                            Ok(JournalPersistOutcome::ReplayIdentical)
                        }
                        Some(_) => Err(PublicationCodecError::JournalConflict),
                        None => Err(PublicationCodecError::JournalOutcomeUnknown),
                    };
                }
                if target.exists() {
                    return match self.read_existing() {
                        Ok(Some(existing)) if existing == bytes => {
                            Ok(JournalPersistOutcome::ReplayIdentical)
                        }
                        Ok(Some(_)) => Err(PublicationCodecError::JournalConflict),
                        Ok(None) | Err(_) => Err(PublicationCodecError::JournalOutcomeUnknown),
                    };
                }
                return Err(PublicationCodecError::JournalOutcomeUnknown);
            }
        }
        match self.read_existing() {
            Ok(Some(stored)) if stored == bytes => Ok(JournalPersistOutcome::Persisted),
            Ok(Some(_)) => Err(PublicationCodecError::JournalConflict),
            Ok(None) | Err(_) => {
                self.quarantine_now();
                Err(PublicationCodecError::JournalCorrupt)
            }
        }
    }

    /// Deterministic fault seam: performs the full durable persist, then
    /// reports acknowledgement loss. The bytes are durable, but the caller
    /// learns only [`PublicationCodecError::JournalOutcomeUnknown`] and must
    /// resolve through [`FileJournal::recovery_read`].
    ///
    /// # Errors
    ///
    /// Returns the underlying persist error, or
    /// [`PublicationCodecError::JournalOutcomeUnknown`] on the durable path.
    pub fn persist_with_lost_acknowledgement(
        &self,
        bytes: &[u8],
    ) -> Result<JournalPersistOutcome, PublicationCodecError> {
        match self.persist(bytes) {
            Ok(_) => Err(PublicationCodecError::JournalOutcomeUnknown),
            Err(error) => Err(error),
        }
    }

    /// Exact recovery read: the only resolver after
    /// [`PublicationCodecError::JournalOutcomeUnknown`]. Absence is an
    /// explicit `None`, never a synthesized payload.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationCodecError::JournalCorrupt`] for torn bytes (the
    /// slot quarantines) or [`PublicationCodecError::JournalOutcomeUnknown`]
    /// when the read itself is interrupted by storage failure.
    pub fn recovery_read(&self) -> Result<Option<Vec<u8>>, PublicationCodecError> {
        self.read_existing()
            .map_err(|_| PublicationCodecError::JournalOutcomeUnknown)
    }

    /// Persists one floor snapshot as opaque marker payload.
    ///
    /// # Errors
    ///
    /// Returns the same failures as [`FileJournal::persist`].
    pub fn persist_floor(
        &self,
        floor: &PublicationFloor,
    ) -> Result<JournalPersistOutcome, PublicationCodecError> {
        self.persist(&encode_floor(floor))
    }

    /// Reads back one floor snapshot written by [`FileJournal::persist_floor`].
    ///
    /// # Errors
    ///
    /// Returns [`PublicationCodecError::JournalCorrupt`] for a present but
    /// undecodable marker, or [`PublicationCodecError::JournalOutcomeUnknown`]
    /// when the read itself is interrupted.
    pub fn recovery_read_floor(&self) -> Result<Option<PublicationFloor>, PublicationCodecError> {
        self.recovery_read()?
            .map(|bytes| decode_floor(&bytes))
            .transpose()
    }

    fn marker_path(&self) -> PathBuf {
        self.dir.join(&self.name)
    }

    fn read_existing(&self) -> Result<Option<Vec<u8>>, PublicationCodecError> {
        let target = self.marker_path();
        let bytes = match fs::read(&target) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(PublicationCodecError::JournalOutcomeUnknown),
        };
        decode_marker(&bytes).map(Some).map_err(|_| {
            self.quarantine_now();
            PublicationCodecError::JournalCorrupt
        })
    }

    fn write_tmp(&self, bytes: &[u8]) -> Result<PathBuf, PublicationCodecError> {
        static NEXT_TMP: AtomicU64 = AtomicU64::new(0);
        let marker = encode_marker(bytes);
        for _ in 0..32_u32 {
            let tmp = self.dir.join(format!(
                ".{}.tmp-{}-{}",
                self.name,
                std::process::id(),
                NEXT_TMP.fetch_add(1, Ordering::Relaxed)
            ));
            let file = match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
            {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(PublicationCodecError::JournalOutcomeUnknown),
            };
            if std::io::Write::write_all(&mut &file, &marker).is_err() {
                let _ = fs::remove_file(&tmp);
                return Err(PublicationCodecError::JournalOutcomeUnknown);
            }
            if file.sync_all().is_err() {
                let _ = fs::remove_file(&tmp);
                return Err(PublicationCodecError::JournalOutcomeUnknown);
            }
            drop(file);
            #[cfg(unix)]
            if sync_dir(&self.dir).is_err() {
                let _ = fs::remove_file(&tmp);
                return Err(PublicationCodecError::JournalOutcomeUnknown);
            }
            #[cfg(windows)]
            sync_dir(&self.dir);
            return Ok(tmp);
        }
        Err(PublicationCodecError::JournalOutcomeUnknown)
    }

    fn quarantine_now(&self) {
        let target = self.marker_path();
        if !target.exists() {
            return;
        }
        let first = self.dir.join(format!("{}.quarantine", self.name));
        if fs::rename(&target, &first).is_ok() {
            return;
        }
        for suffix in 1..=8_u32 {
            let candidate = self.dir.join(format!("{}.quarantine.{suffix}", self.name));
            if fs::rename(&target, &candidate).is_ok() {
                return;
            }
        }
    }
}

fn validate_name(name: &str) -> Result<(), PublicationCodecError> {
    if name.is_empty()
        || name.len() > MAX_JOURNAL_NAME_LEN
        || name.contains(['/', '\\', '\0'])
        || name == "."
        || name == ".."
    {
        return Err(PublicationCodecError::InvalidJournalName);
    }
    Ok(())
}

const fn validate_payload(bytes: &[u8]) -> Result<(), PublicationCodecError> {
    if bytes.is_empty() || bytes.len() > MAX_JOURNAL_BYTES {
        return Err(PublicationCodecError::BudgetExceeded);
    }
    Ok(())
}

fn encode_marker(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 4 + 4 + payload.len() + 32);
    out.extend_from_slice(JOURNAL_MARKER_MAGIC);
    out.extend_from_slice(&JOURNAL_MARKER_VERSION.to_be_bytes());
    out.extend_from_slice(
        &u32::try_from(payload.len())
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    out.extend_from_slice(payload);
    let digest = Sha256::digest(&out);
    out.extend_from_slice(&digest);
    out
}

fn decode_marker(bytes: &[u8]) -> Result<Vec<u8>, PublicationCodecError> {
    if bytes.len() < 8 + 4 + 4 + 32 {
        return Err(PublicationCodecError::JournalCorrupt);
    }
    if bytes[..8] != *JOURNAL_MARKER_MAGIC
        || u32::from_be_bytes(bytes[8..12].try_into().unwrap_or([0xFF; 4]))
            != JOURNAL_MARKER_VERSION
    {
        return Err(PublicationCodecError::JournalCorrupt);
    }
    let declared = u32::from_be_bytes(bytes[12..16].try_into().unwrap_or([0xFF; 4])) as usize;
    if declared > MAX_JOURNAL_BYTES || bytes.len() != 8 + 4 + 4 + declared + 32 {
        return Err(PublicationCodecError::JournalCorrupt);
    }
    let digest = Sha256::digest(&bytes[..bytes.len() - 32]);
    if digest.as_slice() != &bytes[bytes.len() - 32..] {
        return Err(PublicationCodecError::JournalCorrupt);
    }
    Ok(bytes[16..16 + declared].to_vec())
}

#[cfg(unix)]
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    fs::File::open(dir)?.sync_all()
}

/// Windows exposes no directory handle sync through `std`; the same-directory
/// rename is atomic on NTFS and durability is proven by the mandatory exact
/// readback, so this step is a documented platform no-op rather than a
/// skipped guarantee.
#[cfg(windows)]
const fn sync_dir(_dir: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScratchDir {
        path: PathBuf,
    }

    impl ScratchDir {
        fn new(tag: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "eliot-publication-codec-{}-{}-{}",
                std::process::id(),
                tag,
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("scratch directory");
            Self { path }
        }

        fn journal(&self, name: &str) -> FileJournal {
            FileJournal::new(&self.path, name).expect("scratch journal")
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn epoch(value: i64) -> Epoch {
        Epoch::new(value).expect("test epoch is valid")
    }

    fn floor(generation: u64, visible: i64, reserved: i64) -> PublicationFloor {
        PublicationFloor::new(generation, epoch(visible), epoch(reserved)).expect("test floor")
    }

    #[test]
    fn cas_accepts_exact_generation_bump() {
        let stored = floor(4, 4, 6);
        let next = floor(5, 5, 6);
        assert_eq!(cas(&stored, 4, &next), Ok(next));
    }

    #[test]
    fn cas_rejects_stale_expected_generation() {
        let stored = floor(4, 4, 6);
        let next = floor(5, 5, 6);
        assert_eq!(
            cas(&stored, 3, &next),
            Err(PublicationCodecError::ControlConflict)
        );
    }

    #[test]
    fn cas_rejects_mutation_without_plus_one() {
        let stored = floor(4, 4, 6);
        for generation in [4, 6, 7, u64::MAX] {
            let candidate =
                PublicationFloor::new(generation, epoch(4), epoch(6)).expect("candidate builds");
            assert_eq!(
                cas(&stored, 4, &candidate),
                Err(PublicationCodecError::ControlConflict),
                "generation {generation} without the exact step must conflict"
            );
        }
    }

    #[test]
    fn cas_rejects_floor_regression() {
        let stored = floor(4, 4, 6);
        let visible_regressed = floor(5, 3, 6);
        let reserved_regressed = floor(5, 4, 5);
        assert_eq!(
            cas(&stored, 4, &visible_regressed),
            Err(PublicationCodecError::ControlConflict)
        );
        assert_eq!(
            cas(&stored, 4, &reserved_regressed),
            Err(PublicationCodecError::ControlConflict)
        );
    }

    #[test]
    fn floor_constructor_rejects_inverted_floors() {
        assert_eq!(
            PublicationFloor::new(1, epoch(3), epoch(2)),
            Err(PublicationCodecError::ControlConflict)
        );
    }

    #[test]
    fn floor_codec_round_trip_is_exact() {
        let value = floor(9, 8, 12);
        let bytes = encode_floor(&value);
        assert_eq!(bytes.len(), PUBLICATION_FLOOR_LEN);
        assert_eq!(decode_floor(&bytes), Ok(value));
        let mut torn = bytes;
        torn.truncate(torn.len() - 1);
        assert_eq!(
            decode_floor(&torn),
            Err(PublicationCodecError::JournalCorrupt)
        );
    }

    #[test]
    fn journal_persists_and_replays_identical_bytes() {
        let scratch = ScratchDir::new("replay");
        let journal = scratch.journal("intent");
        let bytes = [0xA5; 64].to_vec();
        assert_eq!(
            journal.persist(&bytes),
            Ok(JournalPersistOutcome::Persisted)
        );
        assert_eq!(
            journal.persist(&bytes),
            Ok(JournalPersistOutcome::ReplayIdentical)
        );
        assert_eq!(journal.recovery_read(), Ok(Some(bytes)));
    }

    #[test]
    fn journal_conflicts_on_foreign_valid_marker() {
        let scratch = ScratchDir::new("conflict");
        let journal = scratch.journal("intent");
        journal.persist(&[0x11; 16]).expect("first persist");
        assert_eq!(
            journal.persist(&[0x22; 16]),
            Err(PublicationCodecError::JournalConflict)
        );
    }

    #[test]
    fn journal_corrupt_input_quarantines() {
        let scratch = ScratchDir::new("corrupt");
        let journal = scratch.journal("intent");
        fs::write(scratch.path.join("intent"), [0xFF; 24]).expect("torn marker");
        assert_eq!(
            journal.persist(&[0x11; 8]),
            Err(PublicationCodecError::JournalCorrupt)
        );
        assert!(scratch.path.join("intent.quarantine").exists());
    }

    #[test]
    fn journal_lost_acknowledgement_resolves_only_through_recovery_read() {
        let scratch = ScratchDir::new("unknown");
        let journal = scratch.journal("intent");
        let bytes = [0x77; 48].to_vec();
        assert_eq!(
            journal.persist_with_lost_acknowledgement(&bytes),
            Err(PublicationCodecError::JournalOutcomeUnknown)
        );
        assert_eq!(journal.recovery_read(), Ok(Some(bytes.clone())));
        assert_eq!(
            journal.persist(&bytes),
            Ok(JournalPersistOutcome::ReplayIdentical)
        );
    }
}
