//! Durable development service lifecycle journal.
//!
//! The data-root owner lock is the exclusion authority. This journal records the
//! exact service phase under that authority so startup can distinguish a clean
//! previous release from interrupted composition. The owner record remains the
//! final shutdown linearization point.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::sha256::Sha256Digest;

const SERVICE_RECORD_FORMAT: &str = "ELIOT_SEARCH_SERVICE_V1";
const MAX_SERVICE_RECORD_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServiceMode {
    Stdio,
    Loopback,
}

impl ServiceMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => "STDIO",
            Self::Loopback => "LOOPBACK",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "STDIO" => Ok(Self::Stdio),
            "LOOPBACK" => Ok(Self::Loopback),
            _ => Err("SERVICE_MODE_INVALID".to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServicePhase {
    Starting,
    Ready,
    Draining,
    OwnerReleasePending,
    Interrupted,
}

impl ServicePhase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "STARTING",
            Self::Ready => "READY",
            Self::Draining => "DRAINING",
            Self::OwnerReleasePending => "OWNER_RELEASE_PENDING",
            Self::Interrupted => "INTERRUPTED",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "STARTING" => Ok(Self::Starting),
            "READY" => Ok(Self::Ready),
            "DRAINING" => Ok(Self::Draining),
            "OWNER_RELEASE_PENDING" => Ok(Self::OwnerReleasePending),
            "INTERRUPTED" => Ok(Self::Interrupted),
            _ => Err("SERVICE_PHASE_INVALID".to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ServiceRecord {
    phase: ServicePhase,
    revision: u64,
    owner_epoch: u64,
    service_incarnation_digest: Sha256Digest,
    mode: ServiceMode,
    observed_unix_ms: u128,
    recovered_stale_owner: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ServiceTransitionReceipt {
    pub(crate) phase: ServicePhase,
    pub(crate) revision: u64,
    pub(crate) owner_epoch: u64,
    pub(crate) service_incarnation_digest: Sha256Digest,
    pub(crate) observed_unix_ms: u128,
    pub(crate) recovered_stale_owner: bool,
}

pub(crate) struct ServiceJournal {
    file: File,
    path: PathBuf,
    current: ServiceRecord,
    final_phase_recorded: bool,
}

impl ServiceJournal {
    pub(crate) fn open(
        data_root: &Path,
        owner_epoch: u64,
        service_incarnation_digest: Sha256Digest,
        recovered_stale_owner: bool,
        mode: ServiceMode,
    ) -> Result<Self, String> {
        if owner_epoch == 0 {
            return Err("SERVICE_OWNER_EPOCH_INVALID".to_owned());
        }
        let path = data_root.join(".eliot-search-service-state.tsv");
        if path.exists() {
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("SERVICE_RECORD_METADATA_ERROR:{error}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("SERVICE_RECORD_FILE_INVALID".to_owned());
            }
        }
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| format!("SERVICE_RECORD_OPEN_ERROR:{error}"))?;
        let previous = read_record(&mut file)?;
        if previous.is_some_and(|record| record.owner_epoch >= owner_epoch) {
            return Err("SERVICE_OWNER_EPOCH_REGRESSION".to_owned());
        }
        let revision = previous
            .map_or(Ok(1_u64), |record| {
                record
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| "SERVICE_REVISION_EXHAUSTED".to_owned())
            })?;
        let current = ServiceRecord {
            phase: ServicePhase::Starting,
            revision,
            owner_epoch,
            service_incarnation_digest,
            mode,
            observed_unix_ms: now_unix_ms()?,
            recovered_stale_owner,
        };
        write_record(&mut file, current)?;
        Ok(Self {
            file,
            path,
            current,
            final_phase_recorded: false,
        })
    }

    pub(crate) const fn phase(&self) -> ServicePhase {
        self.current.phase
    }

    pub(crate) const fn revision(&self) -> u64 {
        self.current.revision
    }

    pub(crate) fn verify_current(&mut self) -> Result<(), String> {
        verify_path_identity(&self.path, &self.file)?;
        let observed = read_record(&mut self.file)?
            .ok_or_else(|| "SERVICE_RECORD_READBACK_MISSING".to_owned())?;
        if observed != self.current {
            return Err("SERVICE_RECORD_READBACK_MISMATCH".to_owned());
        }
        Ok(())
    }

    pub(crate) fn mark_ready(&mut self) -> Result<ServiceTransitionReceipt, String> {
        self.transition(ServicePhase::Starting, ServicePhase::Ready)
    }

    pub(crate) fn begin_draining(&mut self) -> Result<ServiceTransitionReceipt, String> {
        match self.current.phase {
            ServicePhase::Starting | ServicePhase::Ready => {
                self.transition_any(ServicePhase::Draining)
            }
            ServicePhase::Draining => Ok(receipt(self.current)),
            ServicePhase::OwnerReleasePending
            | ServicePhase::Interrupted => Err("SERVICE_TRANSITION_INVALID".to_owned()),
        }
    }

    pub(crate) fn mark_owner_release_pending(
        &mut self,
    ) -> Result<ServiceTransitionReceipt, String> {
        let result = self.transition(ServicePhase::Draining, ServicePhase::OwnerReleasePending)?;
        self.final_phase_recorded = true;
        Ok(result)
    }

    pub(crate) fn mark_interrupted(&mut self) -> Result<ServiceTransitionReceipt, String> {
        if self.current.phase == ServicePhase::Interrupted {
            self.final_phase_recorded = true;
            return Ok(receipt(self.current));
        }
        if self.current.phase == ServicePhase::OwnerReleasePending {
            return Err("SERVICE_TRANSITION_INVALID".to_owned());
        }
        let result = self.transition_any(ServicePhase::Interrupted)?;
        self.final_phase_recorded = true;
        Ok(result)
    }

    fn transition(
        &mut self,
        expected: ServicePhase,
        next: ServicePhase,
    ) -> Result<ServiceTransitionReceipt, String> {
        if self.current.phase != expected {
            return Err("SERVICE_TRANSITION_INVALID".to_owned());
        }
        self.transition_any(next)
    }

    fn transition_any(
        &mut self,
        next: ServicePhase,
    ) -> Result<ServiceTransitionReceipt, String> {
        self.verify_current()?;
        let next_record = ServiceRecord {
            phase: next,
            revision: self
                .current
                .revision
                .checked_add(1)
                .ok_or_else(|| "SERVICE_REVISION_EXHAUSTED".to_owned())?,
            owner_epoch: self.current.owner_epoch,
            service_incarnation_digest: self.current.service_incarnation_digest,
            mode: self.current.mode,
            observed_unix_ms: now_unix_ms()?,
            recovered_stale_owner: self.current.recovered_stale_owner,
        };
        write_record(&mut self.file, next_record)?;
        self.current = next_record;
        Ok(receipt(next_record))
    }
}

impl Drop for ServiceJournal {
    fn drop(&mut self) {
        if self.final_phase_recorded {
            return;
        }
        if self.current.phase != ServicePhase::OwnerReleasePending
            && verify_path_identity(&self.path, &self.file).is_ok()
        {
            let _ = self.mark_interrupted();
        }
    }
}

fn receipt(record: ServiceRecord) -> ServiceTransitionReceipt {
    ServiceTransitionReceipt {
        phase: record.phase,
        revision: record.revision,
        owner_epoch: record.owner_epoch,
        service_incarnation_digest: record.service_incarnation_digest,
        observed_unix_ms: record.observed_unix_ms,
        recovered_stale_owner: record.recovered_stale_owner,
    }
}

fn read_record(file: &mut File) -> Result<Option<ServiceRecord>, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("SERVICE_RECORD_SEEK_ERROR:{error}"))?;
    let mut bytes = Vec::new();
    (&mut *file)
        .take(u64::try_from(MAX_SERVICE_RECORD_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("SERVICE_RECORD_READ_ERROR:{error}"))?;
    if bytes.len() > MAX_SERVICE_RECORD_BYTES {
        return Err("SERVICE_RECORD_TOO_LARGE".to_owned());
    }
    if bytes.is_empty() {
        return Ok(None);
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "SERVICE_RECORD_INVALID_UTF8".to_owned())?;
    parse_record(text).map(Some)
}

fn parse_record(text: &str) -> Result<ServiceRecord, String> {
    let fields = text.trim_end().split('\t').collect::<Vec<_>>();
    if fields.len() != 8 || fields[0] != SERVICE_RECORD_FORMAT {
        return Err("SERVICE_RECORD_FORMAT_INVALID".to_owned());
    }
    let phase = ServicePhase::parse(fields[1])?;
    let revision = fields[2]
        .parse::<u64>()
        .map_err(|_| "SERVICE_REVISION_INVALID".to_owned())?;
    let owner_epoch = fields[3]
        .parse::<u64>()
        .map_err(|_| "SERVICE_OWNER_EPOCH_INVALID".to_owned())?;
    if revision == 0 || owner_epoch == 0 {
        return Err("SERVICE_RECORD_COUNTER_INVALID".to_owned());
    }
    let service_incarnation_digest = Sha256Digest::from_hex(fields[4])?;
    let mode = ServiceMode::parse(fields[5])?;
    let observed_unix_ms = fields[6]
        .parse::<u128>()
        .map_err(|_| "SERVICE_TIME_INVALID".to_owned())?;
    let recovered_stale_owner = match fields[7] {
        "true" => true,
        "false" => false,
        _ => return Err("SERVICE_RECOVERY_FLAG_INVALID".to_owned()),
    };
    Ok(ServiceRecord {
        phase,
        revision,
        owner_epoch,
        service_incarnation_digest,
        mode,
        observed_unix_ms,
        recovered_stale_owner,
    })
}

fn encode_record(record: ServiceRecord) -> String {
    format!(
        "{SERVICE_RECORD_FORMAT}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        record.phase.as_str(),
        record.revision,
        record.owner_epoch,
        record.service_incarnation_digest.hex(),
        record.mode.as_str(),
        record.observed_unix_ms,
        record.recovered_stale_owner,
    )
}

fn write_record(file: &mut File, record: ServiceRecord) -> Result<(), String> {
    let encoded = encode_record(record);
    if encoded.len() > MAX_SERVICE_RECORD_BYTES {
        return Err("SERVICE_RECORD_TOO_LARGE".to_owned());
    }
    file.set_len(0)
        .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
        .and_then(|()| file.write_all(encoded.as_bytes()))
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("SERVICE_RECORD_WRITE_ERROR:{error}"))?;
    let observed = read_record(file)?
        .ok_or_else(|| "SERVICE_RECORD_READBACK_MISSING".to_owned())?;
    if observed != record {
        return Err("SERVICE_RECORD_READBACK_MISMATCH".to_owned());
    }
    Ok(())
}

fn verify_path_identity(path: &Path, file: &File) -> Result<(), String> {
    let link_metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("SERVICE_RECORD_METADATA_ERROR:{error}"))?;
    if link_metadata.file_type().is_symlink() || !link_metadata.is_file() {
        return Err("SERVICE_RECORD_FILE_INVALID".to_owned());
    }
    #[cfg(unix)]
    let path_metadata = fs::metadata(path)
        .map_err(|error| format!("SERVICE_RECORD_METADATA_ERROR:{error}"))?;
    #[cfg(unix)]
    let handle_metadata = file
        .metadata()
        .map_err(|error| format!("SERVICE_RECORD_METADATA_ERROR:{error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if path_metadata.dev() != handle_metadata.dev()
            || path_metadata.ino() != handle_metadata.ino()
        {
            return Err("SERVICE_RECORD_IDENTITY_CHANGED".to_owned());
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let path_file = OpenOptions::new()
            .access_mode(0x80) // FILE_READ_ATTRIBUTES
            .share_mode(0x1 | 0x2 | 0x4)
            .custom_flags(0x0020_0000) // FILE_FLAG_OPEN_REPARSE_POINT
            .open(path)
            .map_err(|_| "SERVICE_RECORD_METADATA_ERROR".to_owned())?;
        let path_identity = eliot_searchd::native_file::observe(&path_file)
            .map_err(|error| error.code().to_owned())?;
        let handle_identity = eliot_searchd::native_file::observe(file)
            .map_err(|error| error.code().to_owned())?;
        if path_identity.legacy_identity_bytes() != handle_identity.legacy_identity_bytes() {
            return Err("SERVICE_RECORD_IDENTITY_CHANGED".to_owned());
        }
    }

    Ok(())
}

fn now_unix_ms() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "SYSTEM_CLOCK_BEFORE_EPOCH".to_owned())
        .map(|duration| duration.as_millis())
}
