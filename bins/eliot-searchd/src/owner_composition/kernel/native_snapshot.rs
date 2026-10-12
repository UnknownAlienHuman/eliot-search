//! Exact native installation and owner evidence; no source/store authority.

use std::fs;
use std::path::Path;

use search_runtime_owner::{OwnerError, RootOpenState};

use super::codec::{parse_digest32, parse_id16};
use super::installation::InstallationBinding;
use super::native_objects::{NativePins, metadata, native_identity, open_existing, read_bytes};
use super::native_request::{NativeRootRequest, NativeWork, native_argument};
use super::observation::{ObservedRoot, hash_executable_file, observe_root_parts};
use super::record::DurableOwnerRecord;
use super::spec::{INSTALLATION_MAGIC, LifecycleState, MAX_INSTALLATION_BYTES, MAX_STATE_BYTES};

#[derive(Eq, PartialEq)]
pub(super) struct NativeInstallation {
    pub(super) installation_id: [u8; 16],
    pub(super) installation_incarnation_id: [u8; 16],
    initialization_id: [u8; 16],
    native_digest: [u8; 32],
    pub(super) journal_profile: bool,
}

impl NativeInstallation {
    pub(super) const fn owner_binding(&self) -> InstallationBinding {
        InstallationBinding {
            installation_id: self.installation_id,
            installation_incarnation_id: self.installation_incarnation_id,
        }
    }
}

pub(super) struct NativeSnapshot {
    pub(super) installation: NativeInstallation,
    pub(super) observed: ObservedRoot,
    pub(super) record: DurableOwnerRecord,
    pub(super) target: usize,
    // Verified format3 completion metadata only; no credential is read here.
    pub(super) selected_key_binding: Option<[u8; 32]>,
}

/// Only fixed known absence is accepted. Unknown metadata outcomes fail closed.
pub(super) fn barriers(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    root: &Path,
) -> Result<Option<RootOpenState>, OwnerError> {
    for name in [
        "control/catalog-quarantine.marker",
        "control/catalog-quarantine.tmp",
        ".eliot-search-initialization-intent.v1",
    ] {
        if metadata(request, work, &root.join(name))?.is_some() {
            return Ok(Some(RootOpenState::Quarantined));
        }
    }
    // No writable redb open, sealed secret access or recovery in this profile.
    for name in ["control/control.redb", "sealed-revisions"] {
        if metadata(request, work, &root.join(name))?.is_some() {
            return Ok(Some(RootOpenState::Unsupported));
        }
    }
    Ok(None)
}

pub(super) fn read_snapshot(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    root: &Path,
    pins: &mut NativePins,
) -> Result<NativeSnapshot, OwnerError> {
    let installation = parse_native_installation(&read_bytes(
        request,
        work,
        &mut pins.files[5],
        MAX_INSTALLATION_BYTES,
    )?)?;
    if installation.native_digest != pins.digest(request, work)? {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    if installation.journal_profile != (pins.files.len() == 11) {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    let path_bytes = native_argument(root.as_os_str(), 256 * 1024)?;
    let identity = work.call(request, || native_identity(&pins.files[0]))?;
    // Persisted Windows owner identity is 12 bytes; native pins separately use 20.
    let volume = if cfg!(windows) {
        &identity[..12]
    } else {
        identity.as_slice()
    };
    let observed = observe_root_parts(&path_bytes, volume)?;
    let first = DurableOwnerRecord::decode(&read_bytes(
        request,
        work,
        &mut pins.files[6],
        MAX_STATE_BYTES,
    )?)?;
    let second = DurableOwnerRecord::decode(&read_bytes(
        request,
        work,
        &mut pins.files[7],
        MAX_STATE_BYTES,
    )?)?;
    let (target, record) = choose_pair(first, second)?;
    let executable = executable_digest(request, work)?;
    let selected_key_binding = if installation.journal_profile {
        Some(verify_completed_profile(
            request,
            work,
            pins,
            &installation,
            &observed,
            executable,
        )?)
    } else {
        None
    };
    if record.installation_id != installation.installation_id
        || record.installation_incarnation_id != installation.installation_incarnation_id
        || record.data_root_id != *observed.data_root_id.as_bytes()
        || record.canonical_path_digest != observed.canonical_path_digest
        || record.volume_identity_digest != observed.volume_identity_digest
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    if record.executable_digest != executable {
        return Err(OwnerError::OwnerExecutableIdentityMismatch);
    }
    request.check()?;
    Ok(NativeSnapshot {
        installation,
        observed,
        record,
        target,
        selected_key_binding,
    })
}

pub(super) fn executable_digest(
    request: &NativeRootRequest,
    work: &mut NativeWork,
) -> Result<[u8; 32], OwnerError> {
    let path = work.call(request, || {
        std::env::current_exe().map_err(|_| OwnerError::DataRootInvalid)
    })?;
    let canonical = work.call(request, || {
        fs::canonicalize(&path).map_err(|_| OwnerError::DataRootInvalid)
    })?;
    let _ = native_argument(canonical.as_os_str(), 256 * 1024)?;
    let mut file = open_existing(request, work, &canonical, false, false)?;
    let before = work.call(request, || {
        file.metadata().map_err(|_| OwnerError::DataRootInvalid)
    })?;
    if before.len() > 64 * 1024 * 1024 {
        return Err(OwnerError::ContractExhausted);
    }
    let digest = hash_executable_file(&mut file, |file, chunk| {
        use std::io::Read;
        work.read(request, chunk.len())?;
        work.call(request, || {
            file.read(chunk).map_err(|_| OwnerError::DataRootInvalid)
        })
    })?;
    let after = work.call(request, || {
        file.metadata().map_err(|_| OwnerError::DataRootInvalid)
    })?;
    if after.len() != before.len() || after.modified().ok() != before.modified().ok() {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    let current = open_existing(request, work, &canonical, false, false)?;
    let identity = work.call(request, || native_identity(&file))?;
    if work.call(request, || native_identity(&current))? != identity {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    request.check()?;
    Ok(digest)
}

pub(super) fn parse_native_installation(bytes: &[u8]) -> Result<NativeInstallation, OwnerError> {
    if bytes.len() > MAX_INSTALLATION_BYTES || !bytes.ends_with(b"\n") {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    let lines: Vec<_> = text.lines().take(8).collect();
    if lines.len() != 7
        || lines[0] != INSTALLATION_MAGIC
        || !matches!(
            (lines[1], lines[5]),
            ("format_version=2", "layout_profile=legacy-direct-root-v2")
                | (
                    "format_version=3",
                    "layout_profile=legacy-direct-root-v3-journal-v2"
                )
        )
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let installation_id = parse_id16(lines[2].strip_prefix("installation_id="))?;
    let incarnation_id = parse_id16(lines[3].strip_prefix("installation_incarnation_id="))?;
    let initialization_id = parse_id16(lines[4].strip_prefix("initialization_id="))?;
    let native_digest = parse_digest32(lines[6].strip_prefix("native_objects_digest="))?;
    if installation_id == [0; 16]
        || incarnation_id == [0; 16]
        || initialization_id == [0; 16]
        || native_digest == [0; 32]
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    Ok(NativeInstallation {
        installation_id,
        installation_incarnation_id: incarnation_id,
        initialization_id,
        native_digest,
        journal_profile: lines[1] == "format_version=3",
    })
}

fn verify_completed_profile(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    pins: &mut NativePins,
    installation: &NativeInstallation,
    observed: &ObservedRoot,
    executable: [u8; 32],
) -> Result<[u8; 32], OwnerError> {
    use super::initialization_journal::{MAX_JOURNAL, decode_journal};
    let bytes = read_bytes(request, work, &mut pins.files[10], MAX_JOURNAL)?;
    let journal = decode_journal(&bytes, &mut |bytes| work.canonical(request, bytes))?;
    let seal = journal
        .completion
        .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
    let subject = journal.subject;
    if subject.installation != installation.installation_id
        || subject.incarnation != installation.installation_incarnation_id
        || subject.initialization != installation.initialization_id
        || subject.root_id != *observed.data_root_id.as_bytes()
        || subject.path_digest != observed.canonical_path_digest
        || subject.volume_digest != observed.volume_identity_digest
        || subject.executable != executable
        || seal.native_digest != installation.native_digest
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    for (file, expected) in pins.files.iter().zip(&seal.native_objects) {
        if work.call(request, || native_identity(file))?.as_slice() != expected {
            return Err(OwnerError::OwnerGuardMismatch);
        }
    }
    if read_bytes(request, work, &mut pins.files[5], MAX_INSTALLATION_BYTES)?
        != seal.installation_bytes
        || read_bytes(request, work, &mut pins.files[8], 65)? != seal.namespace_bytes
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    // No secret or catalog read here: this grants root-owner authority only.
    request.check()?;
    Ok(seal.selected_key_binding)
}

/// A corrupt peer is never ignored in favor of an older RELEASED record.
fn choose_pair(
    first: DurableOwnerRecord,
    second: DurableOwnerRecord,
) -> Result<(usize, DurableOwnerRecord), OwnerError> {
    if first.generation == second.generation {
        if first != second {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        return Ok((7, first));
    }
    let (target, newer, older) = if first.generation > second.generation {
        (7, first, second)
    } else {
        (6, second, first)
    };
    if older.generation.checked_add(1) != Some(newer.generation)
        || newer.installation_id != older.installation_id
        || newer.installation_incarnation_id != older.installation_incarnation_id
        || newer.data_root_id != older.data_root_id
        || newer.canonical_path_digest != older.canonical_path_digest
        || newer.volume_identity_digest != older.volume_identity_digest
        || newer.executable_digest != older.executable_digest
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    if newer.epoch == older.epoch {
        if newer.owner_token != older.owner_token
            || newer.owner_pid != older.owner_pid
            || newer.previous_epoch != older.previous_epoch
            || newer.previous_record_digest != older.previous_record_digest
            || !matches!(
                (older.lifecycle, newer.lifecycle),
                (LifecycleState::Active, LifecycleState::Draining)
                    | (LifecycleState::Draining, LifecycleState::Released)
            )
        {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
    } else if older.epoch.checked_add(1) != Some(newer.epoch)
        || older.lifecycle != LifecycleState::Released
        || newer.lifecycle != LifecycleState::Active
        || newer.previous_epoch != older.epoch
        || newer.previous_record_digest != older.record_digest
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    Ok((target, newer))
}
