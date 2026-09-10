//! Single durable installation and runtime owner protocol for one data root.
//!
//! This module is the daemon-local composition root for ownership. The pure
//! ownership policy lives in `search-runtime-owner` and is reused here
//! through its closed [`OwnerError`] codes and [`DrainReason`] vocabulary;
//! this module contributes only the native adapters the policy cannot own:
//! OS exclusion is held by the caller before entry, while installation,
//! physical-root, executable and process-creation observation plus the
//! monotone-epoch durable record live here.
//!
//! Authority rules:
//!
//! - Exactly one live guard exists per data root. Native exclusion (the OS
//!   file lock owned by `DataRootGuard`, co-held with the sealed lock on
//!   Windows) is acquired before any durable owner state is reopened.
//! - Liveness is the held OS lock alone. PID values are recorded for
//!   post-mortem diagnostics but never trusted; no timeout, lease timestamp
//!   or PID comparison can steal, retain or transfer ownership.
//! - Prior ownership is classified by exact observation only: a missing
//!   record means a fresh installation, a fully bound record means guarded
//!   succession with `epoch + 1`, and anything else (installation, physical
//!   root, chain or digest disagreement, corrupt or unreadable state) fails
//!   closed as [`OwnerError::OwnerGuardMismatch`] or quarantine.
//! - Every acquisition consumes exactly one epoch, whether the predecessor
//!   released cleanly or crashed. Clean shutdown persists a `RELEASED`
//!   tombstone first; a crash simply leaves the last published record.
//!   Rollback of the record files to an older internally valid generation is
//!   not detected at this layer and is recorded as a residual risk for the
//!   journal-backed follow-up.
//! - Shutdown is drain-before-release: [`LiveOwner::begin_drain`] persists
//!   `DRAINING` intent, dependency shutdown happens in reverse startup order
//!   by the caller, and [`LiveOwner::release_cleanly`] persists the
//!   `RELEASED` tombstone. Release without a prior drain is refused.
//! - The sealed epoch chain is never a second live authority for primary
//!   roots. When sealed epoch objects are co-present, their head must bind
//!   the same physical root or acquisition is quarantined; epoch numbers
//!   across the two counters are never compared or relabelled. Persistent
//!   sealed and redb formats are unchanged.
//! - [`LiveOwner::journal_owner_inputs`] returns the exact owner-side
//!   journal identity fields (`InstallationIncarnationId`, `DataRootId`,
//!   `OwnerEpoch`) in their native contract types so the redb follow-up can
//!   construct its identity without digest relabelling.
//!
//! File layout (all at the data-root level, beside the lock files, never in
//! `control/` which the catalog-presence preflight scans):
//!
//! - `.eliot-search-installation.v1`: minted once, never regenerated.
//! - `.eliot-search-owner-state-a.v1` / `-b.v1`: alternating durable epoch
//!   slots. A torn write fails its digest and the other slot still carries
//!   the previous valid generation, so no backup or rename dance is needed.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use search_contracts::{DataRootId, InstallationIncarnationId, OwnerEpoch};
use search_runtime_owner::{DrainReason, OwnerError};

const INSTALLATION_FILE: &str = ".eliot-search-installation.v1";
const OWNER_SLOT_A: &str = ".eliot-search-owner-state-a.v1";
const OWNER_SLOT_B: &str = ".eliot-search-owner-state-b.v1";
const INSTALLATION_MAGIC: &str = "ELIOT-SEARCH-INSTALLATION-V1";
const OWNER_STATE_MAGIC: &str = "ELIOT-SEARCH-OWNER-STATE-V1";
const FORMAT_VERSION_LINE: &str = "format_version=1";
const MAX_STATE_BYTES: usize = 4 * 1024;
const MAX_INSTALLATION_BYTES: usize = 1024;
const MAX_EXECUTABLE_BYTES: u64 = 64 * 1024 * 1024;
const READ_CHUNK_BYTES: usize = 64 * 1024;

/// Durable lifecycle persisted in the owner-state slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleState {
    Active,
    Draining,
    Released,
}

impl LifecycleState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Draining => "DRAINING",
            Self::Released => "RELEASED",
        }
    }

    fn parse(value: &str) -> Result<Self, OwnerError> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "DRAINING" => Ok(Self::Draining),
            "RELEASED" => Ok(Self::Released),
            _ => Err(OwnerError::OwnerRecoveryQuarantined),
        }
    }
}

/// Durable drain reason mirroring the [`DrainReason`] policy vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DrainReasonText {
    None,
    Shutdown,
    Restart,
    ModeOrRootChange,
    Maintenance,
}

impl DrainReasonText {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Shutdown => "SHUTDOWN",
            Self::Restart => "RESTART",
            Self::ModeOrRootChange => "MODE_OR_ROOT_CHANGE",
            Self::Maintenance => "MAINTENANCE",
        }
    }

    const fn from_policy(reason: DrainReason) -> Self {
        match reason {
            DrainReason::Shutdown => Self::Shutdown,
            DrainReason::Restart => Self::Restart,
            DrainReason::ModeOrRootChange => Self::ModeOrRootChange,
            DrainReason::Maintenance => Self::Maintenance,
        }
    }

    fn parse(value: &str) -> Result<Self, OwnerError> {
        match value {
            "NONE" => Ok(Self::None),
            "SHUTDOWN" => Ok(Self::Shutdown),
            "RESTART" => Ok(Self::Restart),
            "MODE_OR_ROOT_CHANGE" => Ok(Self::ModeOrRootChange),
            "MAINTENANCE" => Ok(Self::Maintenance),
            _ => Err(OwnerError::OwnerRecoveryQuarantined),
        }
    }
}

/// Which alternating durable slot carries a record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Slot {
    A,
    B,
}

impl Slot {
    const fn file_name(self) -> &'static str {
        match self {
            Self::A => OWNER_SLOT_A,
            Self::B => OWNER_SLOT_B,
        }
    }
}

/// Exact durable owner record: one monotone epoch bound to one installation,
/// one physical root, one executable and one process-creation token.
#[derive(Clone, Eq, PartialEq)]
struct DurableOwnerRecord {
    installation_id: [u8; 16],
    installation_incarnation_id: [u8; 16],
    data_root_id: [u8; 16],
    epoch: u64,
    previous_epoch: u64,
    previous_record_digest: [u8; 32],
    canonical_path_digest: [u8; 32],
    volume_identity_digest: [u8; 32],
    executable_digest: [u8; 32],
    owner_token: [u8; 16],
    owner_pid: u32,
    lifecycle: LifecycleState,
    drain_reason: DrainReasonText,
    generation: u64,
    record_digest: [u8; 32],
}

impl core::fmt::Debug for DurableOwnerRecord {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DurableOwnerRecord")
            .field("installation_id", &hex(&self.installation_id))
            .field(
                "installation_incarnation_id",
                &hex(&self.installation_incarnation_id),
            )
            .field("data_root_id", &hex(&self.data_root_id))
            .field("epoch", &self.epoch)
            .field("previous_epoch", &self.previous_epoch)
            .field("lifecycle", &self.lifecycle)
            .field("drain_reason", &self.drain_reason)
            .field("generation", &self.generation)
            .field("record_digest", &hex(&self.record_digest))
            .field("owner_token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl DurableOwnerRecord {
    /// Canonical body bytes: magic plus every field line in fixed order.
    fn encode_body(&self) -> Vec<u8> {
        let mut output = String::new();
        push_line(&mut output, OWNER_STATE_MAGIC);
        push_field(&mut output, "format_version", "1");
        push_field(&mut output, "installation_id", &hex(&self.installation_id));
        push_field(
            &mut output,
            "installation_incarnation_id",
            &hex(&self.installation_incarnation_id),
        );
        push_field(&mut output, "data_root_id", &hex(&self.data_root_id));
        push_field(&mut output, "epoch", &self.epoch.to_string());
        push_field(
            &mut output,
            "previous_epoch",
            &self.previous_epoch.to_string(),
        );
        push_field(
            &mut output,
            "previous_record_digest",
            &hex(&self.previous_record_digest),
        );
        push_field(
            &mut output,
            "canonical_path_digest",
            &hex(&self.canonical_path_digest),
        );
        push_field(
            &mut output,
            "volume_identity_digest",
            &hex(&self.volume_identity_digest),
        );
        push_field(
            &mut output,
            "executable_digest",
            &hex(&self.executable_digest),
        );
        push_field(&mut output, "owner_token", &hex(&self.owner_token));
        push_field(&mut output, "owner_pid", &self.owner_pid.to_string());
        push_field(&mut output, "lifecycle", self.lifecycle.as_str());
        push_field(&mut output, "drain_reason", self.drain_reason.as_str());
        push_field(&mut output, "generation", &self.generation.to_string());
        output.into_bytes()
    }

    /// Canonical bytes closed by the digest of all preceding body bytes.
    fn encode(&self) -> Vec<u8> {
        let body = self.encode_body();
        let mut output = body.clone();
        output.extend_from_slice(b"record_digest=");
        output.extend_from_slice(blake3_hex(&body).as_bytes());
        output.push(b'\n');
        output
    }

    /// Strict canonical decode with exact digest readback.
    ///
    /// # Errors
    ///
    /// Any shape, value or digest disagreement quarantines; nothing is
    /// repaired or reinterpreted.
    fn decode(bytes: &[u8]) -> Result<Self, OwnerError> {
        if bytes.len() > MAX_STATE_BYTES {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let text = core::str::from_utf8(bytes).map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        if !text.ends_with('\n') {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let lines: Vec<&str> = text.lines().collect();
        // Magic plus fifteen field lines plus the closing digest line.
        if lines.len() != 17 || lines[0] != OWNER_STATE_MAGIC {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let mut values: [Option<&str>; 15] = [None; 15];
        for line in &lines[1..16] {
            let Some((key, value)) = line.split_once('=') else {
                return Err(OwnerError::OwnerRecoveryQuarantined);
            };
            let slot = match key {
                "format_version" => 0,
                "installation_id" => 1,
                "installation_incarnation_id" => 2,
                "data_root_id" => 3,
                "epoch" => 4,
                "previous_epoch" => 5,
                "previous_record_digest" => 6,
                "canonical_path_digest" => 7,
                "volume_identity_digest" => 8,
                "executable_digest" => 9,
                "owner_token" => 10,
                "owner_pid" => 11,
                "lifecycle" => 12,
                "drain_reason" => 13,
                "generation" => 14,
                // The closing digest key is only valid as the final line.
                _ => return Err(OwnerError::OwnerRecoveryQuarantined),
            };
            if values[slot].is_some() || value.is_empty() {
                return Err(OwnerError::OwnerRecoveryQuarantined);
            }
            values[slot] = Some(value);
        }
        // `record_digest` must close the record; it covers all preceding bytes.
        let digest_line = lines[16];
        let Some(digest_value) = digest_line.strip_prefix("record_digest=") else {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        };
        if values[0] != Some("1") {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        // `lines()` strips terminators: the body is everything before the
        // closing digest line plus its own newline.
        let body_end = text
            .len()
            .checked_sub(digest_line.len() + 1)
            .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
        if blake3_hex(&text.as_bytes()[..body_end]) != digest_value {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let missing = || OwnerError::OwnerRecoveryQuarantined;
        let record = Self {
            installation_id: parse_id16(values[1])?,
            installation_incarnation_id: parse_id16(values[2])?,
            data_root_id: parse_id16(values[3])?,
            epoch: parse_u64(values[4])?,
            previous_epoch: parse_u64(values[5])?,
            previous_record_digest: parse_digest32(values[6])?,
            canonical_path_digest: parse_digest32(values[7])?,
            volume_identity_digest: parse_digest32(values[8])?,
            executable_digest: parse_digest32(values[9])?,
            owner_token: parse_id16(values[10])?,
            owner_pid: parse_u32(values[11])?,
            lifecycle: LifecycleState::parse(values[12].ok_or_else(missing)?)?,
            drain_reason: DrainReasonText::parse(values[13].ok_or_else(missing)?)?,
            generation: parse_u64(values[14])?,
            record_digest: parse_digest32(Some(digest_value))?,
        };
        record.validate_shape()?;
        Ok(record)
    }

    /// Internal consistency that one writer always produces.
    fn validate_shape(&self) -> Result<(), OwnerError> {
        if self.epoch == 0 || self.generation == 0 {
            return Err(OwnerError::OwnerEpochMismatch);
        }
        if self.epoch == 1 {
            if self.previous_epoch != 0 || self.previous_record_digest != [0; 32] {
                return Err(OwnerError::OwnerEpochMismatch);
            }
        } else if self.previous_epoch != self.epoch - 1 {
            return Err(OwnerError::OwnerEpochMismatch);
        }
        match self.lifecycle {
            LifecycleState::Draining => {
                if self.drain_reason == DrainReasonText::None {
                    return Err(OwnerError::OwnerRecoveryQuarantined);
                }
            }
            LifecycleState::Active | LifecycleState::Released => {
                if self.drain_reason != DrainReasonText::None {
                    return Err(OwnerError::OwnerRecoveryQuarantined);
                }
            }
        }
        Ok(())
    }
}

/// Fresh native observation of one already-locked canonical root.
struct ObservedRoot {
    canonical_path_digest: [u8; 32],
    volume_identity_digest: [u8; 32],
    data_root_id: DataRootId,
}

/// Process-local live owner authority: non-cloneable by construction.
pub struct LiveOwner {
    canonical_root: PathBuf,
    installation_incarnation_id: InstallationIncarnationId,
    data_root_id: DataRootId,
    epoch: OwnerEpoch,
    record: DurableOwnerRecord,
    recovered_previous_active: bool,
    poisoned: bool,
}

impl core::fmt::Debug for LiveOwner {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LiveOwner")
            .field("data_root_id", &self.data_root_id)
            .field("owner_epoch", &self.epoch)
            .field("generation", &self.record.generation)
            .field("lifecycle", &self.record.lifecycle)
            .field("recovered_previous_active", &self.recovered_previous_active)
            .field("owner_token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Content-free proof that one exact owner released cleanly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownReceipt {
    /// Released owner epoch.
    pub epoch: OwnerEpoch,
    /// Final durable record generation.
    pub generation: u64,
    /// Digest of the exact `RELEASED` record bytes.
    pub record_digest: [u8; 32],
}

impl LiveOwner {
    /// Bound monotone owner epoch.
    #[must_use]
    pub const fn epoch(&self) -> OwnerEpoch {
        self.epoch
    }

    /// Whether the predecessor held an unreleased (`ACTIVE`/`DRAINING`)
    /// record; a clean `RELEASED` tombstone reports false.
    #[must_use]
    pub const fn recovered_previous_active(&self) -> bool {
        self.recovered_previous_active
    }

    /// Exact owner-side journal identity inputs for the redb follow-up.
    ///
    /// The three values use the same contract types the journal header
    /// stores, so no digest or identifier is ever relabelled across the
    /// boundary. Path and schema digests stay owned by the journal layer.
    #[must_use]
    pub const fn journal_owner_inputs(
        &self,
    ) -> (InstallationIncarnationId, DataRootId, OwnerEpoch) {
        (
            self.installation_incarnation_id,
            self.data_root_id,
            self.epoch,
        )
    }

    /// Persists `DRAINING` intent under the live lock; idempotent.
    ///
    /// # Errors
    ///
    /// A poisoned guard reports the unknown outcome instead of mutating;
    /// a released lifecycle is refused.
    pub(crate) fn begin_drain(&mut self, reason: DrainReason) -> Result<(), OwnerError> {
        if self.poisoned {
            return Err(OwnerError::OwnerAcquireOutcomeUnknown);
        }
        if self.record.lifecycle == LifecycleState::Draining {
            return Ok(());
        }
        if self.record.lifecycle != LifecycleState::Active {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        let next = self.transition_record(
            LifecycleState::Draining,
            DrainReasonText::from_policy(reason),
        )?;
        match publish_transition(&self.canonical_root, &self.record, &next) {
            Ok(()) => {
                self.record = next;
                Ok(())
            }
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    /// Persists the `RELEASED` tombstone under the live lock.
    ///
    /// The caller must have drained dependencies in reverse startup order
    /// before calling: release intent is refused without a prior drain.
    /// The OS exclusion itself is released by the outer guard drop that
    /// follows, after every storage and process resource is closed. A
    /// second call is refused: the in-memory lifecycle already left
    /// `DRAINING`.
    ///
    /// # Errors
    ///
    /// Release without a prior drain, a poisoned guard, or an unrecoverable
    /// durable contradiction fails closed; a possible write whose readback
    /// cannot be proven reports the unknown outcome.
    pub(crate) fn release_cleanly(&mut self) -> Result<ShutdownReceipt, OwnerError> {
        if self.poisoned {
            return Err(OwnerError::OwnerReleaseOutcomeUnknown);
        }
        if self.record.lifecycle != LifecycleState::Draining {
            return Err(OwnerError::OwnerDrainRequired);
        }
        let next = self.transition_record(LifecycleState::Released, DrainReasonText::None)?;
        match publish_transition(&self.canonical_root, &self.record, &next) {
            Ok(()) => {
                self.record = next.clone();
                Ok(ShutdownReceipt {
                    epoch: self.epoch,
                    generation: next.generation,
                    record_digest: next.record_digest,
                })
            }
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    fn transition_record(
        &self,
        lifecycle: LifecycleState,
        drain_reason: DrainReasonText,
    ) -> Result<DurableOwnerRecord, OwnerError> {
        let generation = self
            .record
            .generation
            .checked_add(1)
            .ok_or(OwnerError::ContractExhausted)?;
        let mut next = self.record.clone();
        next.lifecycle = lifecycle;
        next.drain_reason = drain_reason;
        next.generation = generation;
        next.record_digest = blake3_bytes(&next.encode_body());
        Ok(next)
    }
}

/// Establishes the single live owner under an already-held OS exclusion.
///
/// Binds the minted-once installation identity, the fresh physical-root and
/// executable observations, a fresh process-creation token and the next
/// monotone epoch, then publishes exactly one durable record with exact
/// readback.
///
/// # Errors
///
/// A held live lock is the caller's precondition and is never re-checked
/// here. Binding disagreement, corrupt or unreadable state, a disagreeing
/// sealed mirror, or an unprovable write outcome fail closed; fresh roots
/// start at epoch one.
pub fn establish(canonical_root: &Path) -> Result<LiveOwner, OwnerError> {
    let installation = load_or_create_installation(canonical_root)?;
    let observed = observe_physical_root(canonical_root)?;
    let executable = observe_executable()?;
    verify_sealed_head_agrees(canonical_root)?;
    let (target, prior) = newest_valid(canonical_root)?;
    let record = plan_successor(&installation, &observed, executable, prior.as_deref())?;
    write_slot(canonical_root, target, &record)?;
    let reloaded =
        read_slot(canonical_root, target).ok_or(OwnerError::OwnerAcquireOutcomeUnknown)?;
    if *reloaded != record {
        return Err(OwnerError::OwnerRecordDigestMismatch);
    }
    Ok(LiveOwner {
        canonical_root: canonical_root.to_owned(),
        installation_incarnation_id: InstallationIncarnationId::from_bytes(
            record.installation_incarnation_id,
        ),
        data_root_id: observed.data_root_id,
        epoch: OwnerEpoch::new(record.epoch).map_err(|_| OwnerError::ContractExhausted)?,
        record,
        recovered_previous_active: prior
            .as_ref()
            .is_some_and(|previous| previous.lifecycle != LifecycleState::Released),
        poisoned: false,
    })
}

struct InstallationBinding {
    installation_id: [u8; 16],
    installation_incarnation_id: [u8; 16],
}

/// Loads the minted-once installation identity or mints it atomically.
///
/// # Errors
///
/// A corrupt existing file quarantines: regenerating would fork the stable
/// installation identity a copied root must keep proving.
fn load_or_create_installation(canonical_root: &Path) -> Result<InstallationBinding, OwnerError> {
    let path = canonical_root.join(INSTALLATION_FILE);
    match fs::read(&path) {
        Ok(bytes) => parse_installation(&bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            mint_installation(canonical_root, &path)
        }
        Err(_) => Err(OwnerError::DataRootInvalid),
    }
}

fn parse_installation(bytes: &[u8]) -> Result<InstallationBinding, OwnerError> {
    if bytes.len() > MAX_INSTALLATION_BYTES {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let text = core::str::from_utf8(bytes).map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    if !text.ends_with('\n') {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() != 4 || lines[0] != INSTALLATION_MAGIC || lines[1] != FORMAT_VERSION_LINE {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let installation_id = lines[2]
        .strip_prefix("installation_id=")
        .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
    let incarnation_id = lines[3]
        .strip_prefix("installation_incarnation_id=")
        .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
    Ok(InstallationBinding {
        installation_id: parse_id16(Some(installation_id))?,
        installation_incarnation_id: parse_id16(Some(incarnation_id))?,
    })
}

fn mint_installation(
    canonical_root: &Path,
    path: &Path,
) -> Result<InstallationBinding, OwnerError> {
    let observed = observe_physical_root(canonical_root)?;
    let executable = observe_executable()?;
    let stamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| OwnerError::DataRootInvalid)?
        .as_nanos();
    let counter = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-search/owner-installation/v1\0");
    hasher.update(&stamp_nanos.to_be_bytes());
    hasher.update(&std::process::id().to_be_bytes());
    hasher.update(&counter.to_be_bytes());
    hasher.update(&executable);
    hasher.update(&observed.canonical_path_digest);
    hasher.update(&observed.volume_identity_digest);
    let digest = hasher.finalize();
    let bytes = digest.as_bytes();
    let installation_id: [u8; 16] = bytes[..16]
        .try_into()
        .map_err(|_| OwnerError::DataRootInvalid)?;
    let installation_incarnation_id: [u8; 16] = bytes[16..]
        .try_into()
        .map_err(|_| OwnerError::DataRootInvalid)?;
    let binding = InstallationBinding {
        installation_id,
        installation_incarnation_id,
    };
    let mut text = String::new();
    push_line(&mut text, INSTALLATION_MAGIC);
    push_line(&mut text, FORMAT_VERSION_LINE);
    push_field(&mut text, "installation_id", &hex(&binding.installation_id));
    push_field(
        &mut text,
        "installation_incarnation_id",
        &hex(&binding.installation_incarnation_id),
    );
    // First writer wins; an already-present file is re-read, never replaced.
    match OpenOptions::new().create_new(true).write(true).open(path) {
        Ok(mut file) => {
            file.write_all(text.as_bytes())
                .map_err(|_| OwnerError::DataRootInvalid)?;
            file.sync_all().map_err(|_| OwnerError::DataRootInvalid)?;
            drop(file);
            sync_directory(canonical_root);
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(OwnerError::DataRootInvalid),
    }
    fs::read(path).map_or(Err(OwnerError::OwnerAcquireOutcomeUnknown), |bytes| {
        parse_installation(&bytes)
    })
}

/// Observes the physical root behind an already-held exclusion.
///
/// # Errors
///
/// Any unprovable identity fails closed; relocation between the lock steps
/// is reported as a guard mismatch.
fn observe_physical_root(canonical_root: &Path) -> Result<ObservedRoot, OwnerError> {
    let fresh = fs::canonicalize(canonical_root).map_err(|_| OwnerError::DataRootInvalid)?;
    if fresh != canonical_root {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    let metadata = fs::symlink_metadata(&fresh).map_err(|_| OwnerError::DataRootInvalid)?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err(OwnerError::DataRootInvalid);
    }
    let path_bytes = canonical_path_bytes(&fresh);
    let volume_material = native_volume_material(&fresh)?;
    let canonical_path_digest =
        domain_digest(b"eliot-search/owner-canonical-path/v1\0", &[&path_bytes]);
    let volume_identity_digest = domain_digest(
        b"eliot-search/owner-volume-identity/v1\0",
        &[&volume_material],
    );
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-search/owner-data-root-id/v1\0");
    hasher.update(&volume_material);
    hasher.update(&path_bytes);
    let digest = hasher.finalize();
    let id: [u8; 16] = digest.as_bytes()[..16]
        .try_into()
        .map_err(|_| OwnerError::DataRootInvalid)?;
    Ok(ObservedRoot {
        canonical_path_digest,
        volume_identity_digest,
        data_root_id: DataRootId::from_bytes(id),
    })
}

/// Hashes the running executable image with a finite budget.
///
/// # Errors
///
/// An unreadable, non-regular or oversized image fails closed.
fn observe_executable() -> Result<[u8; 32], OwnerError> {
    let path = std::env::current_exe().map_err(|_| OwnerError::DataRootInvalid)?;
    let canonical = fs::canonicalize(&path).map_err(|_| OwnerError::DataRootInvalid)?;
    let metadata = fs::metadata(&canonical).map_err(|_| OwnerError::DataRootInvalid)?;
    if !metadata.is_file() {
        return Err(OwnerError::DataRootInvalid);
    }
    let mut file = fs::File::open(&canonical).map_err(|_| OwnerError::DataRootInvalid)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-search/owner-executable/v1\0");
    let mut chunk = vec![0_u8; READ_CHUNK_BYTES];
    let mut total: u64 = 0;
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|_| OwnerError::DataRootInvalid)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| OwnerError::DataRootInvalid)?)
            .ok_or(OwnerError::DataRootInvalid)?;
        if total > MAX_EXECUTABLE_BYTES {
            return Err(OwnerError::DataRootInvalid);
        }
        hasher.update(&chunk[..read]);
    }
    let digest = hasher.finalize();
    let mut output = [0_u8; 32];
    output.copy_from_slice(digest.as_bytes());
    Ok(output)
}

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(0);

/// Mints a reuse-resistant process-creation token for one acquisition.
fn mint_owner_token(
    observed: &ObservedRoot,
    executable: &[u8; 32],
) -> Result<[u8; 16], OwnerError> {
    let stamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| OwnerError::DataRootInvalid)?
        .as_nanos();
    let counter = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-search/owner-token/v1\0");
    hasher.update(&stamp_nanos.to_be_bytes());
    hasher.update(&std::process::id().to_be_bytes());
    hasher.update(&counter.to_be_bytes());
    hasher.update(executable);
    hasher.update(&observed.canonical_path_digest);
    hasher.update(&observed.volume_identity_digest);
    let digest = hasher.finalize();
    digest.as_bytes()[..16]
        .try_into()
        .map_err(|_| OwnerError::DataRootInvalid)
}

/// Verifies that a co-present sealed epoch mirror binds the same root.
///
/// # Errors
///
/// No sealed objects means no second authority and passes. A head that
/// cannot be authenticated, decoded or root-matched quarantines; epoch
/// numbers across the two counters are never compared.
fn verify_sealed_head_agrees(canonical_root: &Path) -> Result<(), OwnerError> {
    let head = crate::sealed_owner_epoch::latest_sealed_head(canonical_root)
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    let Some(record) = head else {
        return Ok(());
    };
    let current = crate::sealed_root_identity::root_binding_sha256(canonical_root)
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    if record.root_binding_sha256 != current {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    Ok(())
}

/// Raw per-slot read outcome, keeping absence distinct from corruption.
///
/// The valid payload is boxed: slots are usually missing, and the record is
/// hundreds of bytes next to fieldless outcomes.
enum SlotRead {
    Missing,
    Valid(Box<DurableOwnerRecord>),
    Unreadable,
}

fn read_slot(canonical_root: &Path, slot: Slot) -> Option<Box<DurableOwnerRecord>> {
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
/// # Errors
///
/// Any unreadable slot or same-generation conflict quarantines; only two
/// missing slots mean a fresh root.
fn newest_valid(
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

/// Plans the exact successor record for a fresh or bound predecessor.
///
/// # Errors
///
/// Installation or physical-root disagreement denies the succession;
/// a non-monotone chain or an exhausted counter fails closed.
fn plan_successor(
    installation: &InstallationBinding,
    observed: &ObservedRoot,
    executable: [u8; 32],
    prior: Option<&DurableOwnerRecord>,
) -> Result<DurableOwnerRecord, OwnerError> {
    let token = mint_owner_token(observed, &executable)?;
    let (epoch, previous_epoch, previous_record_digest, generation) = match prior {
        None => (1_u64, 0_u64, [0; 32], 1_u64),
        Some(previous) => {
            if previous.installation_id != installation.installation_id
                || previous.installation_incarnation_id != installation.installation_incarnation_id
            {
                return Err(OwnerError::OwnerGuardMismatch);
            }
            if previous.data_root_id != *observed.data_root_id.as_bytes()
                || previous.canonical_path_digest != observed.canonical_path_digest
                || previous.volume_identity_digest != observed.volume_identity_digest
            {
                return Err(OwnerError::OwnerGuardMismatch);
            }
            previous.validate_shape()?;
            let epoch = previous
                .epoch
                .checked_add(1)
                .ok_or(OwnerError::ContractExhausted)?;
            let generation = previous
                .generation
                .checked_add(1)
                .ok_or(OwnerError::ContractExhausted)?;
            (epoch, previous.epoch, previous.record_digest, generation)
        }
    };
    let mut record = DurableOwnerRecord {
        installation_id: installation.installation_id,
        installation_incarnation_id: installation.installation_incarnation_id,
        data_root_id: *observed.data_root_id.as_bytes(),
        epoch,
        previous_epoch,
        previous_record_digest,
        canonical_path_digest: observed.canonical_path_digest,
        volume_identity_digest: observed.volume_identity_digest,
        executable_digest: executable,
        owner_token: token,
        owner_pid: std::process::id(),
        lifecycle: LifecycleState::Active,
        drain_reason: DrainReasonText::None,
        generation,
        record_digest: [0; 32],
    };
    record.validate_shape()?;
    record.record_digest = blake3_bytes(&record.encode_body());
    Ok(record)
}

/// Publishes one slot with exact byte readback.
///
/// # Errors
///
/// Pre-publication storage failures report the root unusable; a missing or
/// contradictory readback reports the unknown outcome or digest mismatch.
fn write_slot(
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
/// # Errors
///
/// A predecessor that no longer matches the live guard quarantines instead
/// of overwriting foreign state.
fn publish_transition(
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

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn push_field(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn parse_hex_exact(value: Option<&str>, expected_bytes: usize) -> Result<Vec<u8>, OwnerError> {
    let Some(value) = value else {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    };
    if value.len() != expected_bytes * 2
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let mut output = Vec::with_capacity(expected_bytes);
    for pair in value.as_bytes().chunks(2) {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

const fn hex_value(byte: u8) -> Result<u8, OwnerError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(OwnerError::OwnerRecoveryQuarantined),
    }
}

fn parse_id16(value: Option<&str>) -> Result<[u8; 16], OwnerError> {
    parse_hex_exact(value, 16)?
        .try_into()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
}

fn parse_digest32(value: Option<&str>) -> Result<[u8; 32], OwnerError> {
    parse_hex_exact(value, 32)?
        .try_into()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
}

fn parse_u64(value: Option<&str>) -> Result<u64, OwnerError> {
    let Some(value) = value else {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    };
    if value.is_empty()
        || value.starts_with('+')
        || value.starts_with('-')
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    value
        .parse::<u64>()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
}

fn parse_u32(value: Option<&str>) -> Result<u32, OwnerError> {
    let Some(value) = value else {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    };
    if value.is_empty()
        || value.starts_with('+')
        || value.starts_with('-')
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    value
        .parse::<u32>()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
}

fn blake3_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytes);
    let finalized = hasher.finalize();
    let mut output = [0_u8; 32];
    output.copy_from_slice(finalized.as_bytes());
    output
}

fn blake3_hex(bytes: &[u8]) -> String {
    hex(&blake3_bytes(bytes))
}

fn domain_digest(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(part);
    }
    let finalized = hasher.finalize();
    let mut output = [0_u8; 32];
    output.copy_from_slice(finalized.as_bytes());
    output
}

fn canonical_path_bytes(canonical_root: &Path) -> Vec<u8> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        canonical_root
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect()
    }
    #[cfg(all(unix, not(windows)))]
    {
        use std::os::unix::ffi::OsStrExt;
        canonical_root.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(any(unix, windows)))]
    {
        canonical_root.to_string_lossy().as_bytes().to_vec()
    }
}

fn native_volume_material(canonical_root: &Path) -> Result<Vec<u8>, OwnerError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        let handle = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(canonical_root)
            .map_err(|_| OwnerError::DataRootInvalid)?;
        let metadata = handle.metadata().map_err(|_| OwnerError::DataRootInvalid)?;
        if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(OwnerError::DataRootInvalid);
        }
        let observed = eliot_searchd::native_file::observe(&handle)
            .map_err(|_| OwnerError::DataRootInvalid)?;
        Ok(observed.legacy_identity_bytes().to_vec())
    }
    #[cfg(all(unix, not(windows)))]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(canonical_root).map_err(|_| OwnerError::DataRootInvalid)?;
        let mut material = Vec::with_capacity(16);
        material.extend_from_slice(&metadata.dev().to_be_bytes());
        material.extend_from_slice(&metadata.ino().to_be_bytes());
        Ok(material)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = canonical_root;
        Ok(Vec::new())
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn sync_directory(path: &Path) {
    let _ = fs::File::open(path).and_then(|file| file.sync_all());
}

#[cfg(not(unix))]
const fn sync_directory(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

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
        // Exact byte readback of what was published.
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
        // Missing terminator.
        cases.push(canonical.trim_end().to_owned());
        // Truncated magic.
        cases.push(canonical.replacen("ELIOT-SEARCH-OWNER-STATE-V1", "ELIOT-SEARCH-OWNER", 1));
        // Duplicate field.
        cases.push(format!("{canonical}epoch=1\n"));
        // Unknown field.
        cases.push(canonical.replacen("generation=", "generation_x=", 1));
        // Uppercase digest can never be canonical.
        let digest_line = canonical
            .lines()
            .find(|line| line.starts_with("record_digest="))
            .unwrap();
        cases.push(canonical.replace(digest_line, &digest_line.to_uppercase()));
        // Leading-zero epoch is not canonical.
        cases.push(canonical.replacen("epoch=1\n", "epoch=01\n", 1));
        // Zero epoch is rejected.
        cases.push(canonical.replacen("epoch=1\n", "epoch=0\n", 1));
        // Dropped line breaks the line count.
        let dropped = canonical
            .lines()
            .filter(|line| !line.starts_with("owner_pid="))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        cases.push(dropped);
        // Flipped body byte breaks the digest.
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
        // Slots alternate: epochs one and two live side by side.
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
    fn foreign_installation_denies_succession() {
        let first = Scratch::new();
        let _ = first.establish();
        let second = Scratch::new();
        let _ = second.establish();
        // Transplanting another root's installation identity must deny,
        // never fork the succession chain.
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
        // Same bytes, different physical root: exact observation denies.
        assert!(matches!(
            establish(&second.0),
            Err(OwnerError::OwnerGuardMismatch)
        ));
        // The copy attempt wrote nothing durable of its own.
        assert!(!second.0.join(Slot::B.file_name()).exists());
    }

    #[test]
    fn corrupt_slots_quarantine_without_repair() {
        let scratch = Scratch::new();
        let _ = scratch.establish();
        for slot in [Slot::A, Slot::B] {
            fs::write(scratch.0.join(slot.file_name()), b"corrupt-and-preserved").unwrap();
        }
        assert!(matches!(
            establish(&scratch.0),
            Err(OwnerError::OwnerRecoveryQuarantined)
        ));
        // No implicit repair: corrupt bytes survive the refused acquisition.
        assert_eq!(
            fs::read(scratch.0.join(Slot::A.file_name())).unwrap(),
            b"corrupt-and-preserved"
        );
    }

    #[test]
    fn torn_non_authority_slot_heals_by_guarded_succession() {
        let scratch = Scratch::new();
        let _ = scratch.establish();
        // A torn write always lands on the non-authority slot: the single
        // writer under the lock never touches the newest valid record.
        // Overwriting it cannot destroy authority or rewind the epoch.
        fs::write(scratch.0.join(Slot::B.file_name()), b"torn-write").unwrap();
        let next = scratch.establish();
        assert_eq!(next.epoch().get(), 2);
        assert!(next.recovered_previous_active());
        let healed = DurableOwnerRecord::decode(&scratch.slot_bytes(Slot::B)).unwrap();
        assert_eq!(healed.epoch, 2);
        assert_eq!(healed, next.record);
        // The authority slot is untouched by the healing write.
        let authority = DurableOwnerRecord::decode(&scratch.slot_bytes(Slot::A)).unwrap();
        assert_eq!(authority.epoch, 1);
    }

    #[test]
    fn drain_release_lifecycle_is_guarded_and_idempotent() {
        let scratch = Scratch::new();
        let mut owner = scratch.establish();
        assert_eq!(owner.release_cleanly(), Err(OwnerError::OwnerDrainRequired));
        owner.begin_drain(DrainReason::Shutdown).unwrap();
        // Second drain is idempotent, not a second generation bump.
        let generation = owner.record.generation;
        owner.begin_drain(DrainReason::Shutdown).unwrap();
        assert_eq!(owner.record.generation, generation);
        let receipt = owner.release_cleanly().unwrap();
        assert_eq!(receipt.epoch.get(), 1);
        assert_eq!(receipt.generation, generation + 1);
        assert_eq!(receipt.record_digest, owner.record.record_digest);
        // The guard already left DRAINING: no second release tombstone.
        assert_eq!(owner.release_cleanly(), Err(OwnerError::OwnerDrainRequired));
        assert_eq!(
            owner.begin_drain(DrainReason::Shutdown),
            Err(OwnerError::OwnerInvalidTransition)
        );
        // The tombstone is durable and exact; it lands opposite the slot
        // that was newest at release time.
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
        let (installation_incarnation_id, data_root_id, owner_epoch) = owner.journal_owner_inputs();
        // The same contract types the journal header stores: path and schema
        // digests stay journal-owned, owner fields plug in exactly.
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
}
