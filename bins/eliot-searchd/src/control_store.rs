//! Crash-tolerant development control state for the local daemon.
//!
//! Two alternating bounded files preserve the last valid generation if a write
//! tears. Only lifecycle and snapshot metadata are stored. Source bytes, query
//! text, excerpts, credentials, and unrestricted paths are excluded.

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u64 = 1;
const MAX_STATE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Slot {
    A,
    B,
}

impl Slot {
    const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }

    const fn file_name(self) -> &'static str {
        match self {
            Self::A => "daemon-state-a.v1",
            Self::B => "daemon-state-b.v1",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lifecycle {
    Starting,
    Ready,
    Stopped,
}

impl Lifecycle {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "STARTING",
            Self::Ready => "READY",
            Self::Stopped => "STOPPED",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "STARTING" => Ok(Self::Starting),
            "READY" => Ok(Self::Ready),
            "STOPPED" => Ok(Self::Stopped),
            _ => Err("CONTROL_STORE_LIFECYCLE_INVALID".to_owned()),
        }
    }
}

/// Snapshot fields admitted to technical control state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotControl {
    pub(crate) snapshot_id: String,
    pub(crate) manifest_fingerprint: String,
    pub(crate) fingerprint_algorithm: String,
    pub(crate) indexed_files: usize,
    pub(crate) total_bytes: u64,
    pub(crate) capture_complete: bool,
}

impl SnapshotControl {
    fn validate(&self) -> Result<(), String> {
        if self.snapshot_id.is_empty()
            || self.snapshot_id.len() > 128
            || !self
                .snapshot_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || self.manifest_fingerprint.len() != 64
            || !self
                .manifest_fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.fingerprint_algorithm.is_empty()
            || self.fingerprint_algorithm.len() > 64
            || !self
                .fingerprint_algorithm
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err("CONTROL_STORE_SNAPSHOT_INVALID".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ControlState {
    schema: u64,
    generation: u64,
    lifecycle: Lifecycle,
    pid: u32,
    started_unix_ms: u128,
    recovered_previous_active: bool,
    snapshot: Option<SnapshotControl>,
}

impl ControlState {
    fn serialize(&self) -> Result<Vec<u8>, String> {
        let mut output = String::new();
        output.push_str("ELIOT_SEARCH_CONTROL_STATE_V1\n");
        output.push_str(&format!("schema={}\n", self.schema));
        output.push_str(&format!("generation={}\n", self.generation));
        output.push_str(&format!("lifecycle={}\n", self.lifecycle.as_str()));
        output.push_str(&format!("pid={}\n", self.pid));
        output.push_str(&format!("started_unix_ms={}\n", self.started_unix_ms));
        output.push_str(&format!(
            "recovered_previous_active={}\n",
            self.recovered_previous_active
        ));
        match &self.snapshot {
            Some(snapshot) => {
                snapshot.validate()?;
                output.push_str("snapshot_present=true\n");
                output.push_str(&format!("snapshot_id={}\n", snapshot.snapshot_id));
                output.push_str(&format!(
                    "manifest_fingerprint={}\n",
                    snapshot.manifest_fingerprint
                ));
                output.push_str(&format!(
                    "fingerprint_algorithm={}\n",
                    snapshot.fingerprint_algorithm
                ));
                output.push_str(&format!("indexed_files={}\n", snapshot.indexed_files));
                output.push_str(&format!("total_bytes={}\n", snapshot.total_bytes));
                output.push_str(&format!(
                    "capture_complete={}\n",
                    snapshot.capture_complete
                ));
            }
            None => output.push_str("snapshot_present=false\n"),
        }
        if output.len() > MAX_STATE_BYTES {
            return Err("CONTROL_STORE_STATE_TOO_LARGE".to_owned());
        }
        Ok(output.into_bytes())
    }

    fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_STATE_BYTES {
            return Err("CONTROL_STORE_STATE_TOO_LARGE".to_owned());
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|_| "CONTROL_STORE_STATE_NOT_UTF8".to_owned())?;
        let mut lines = text.lines();
        if lines.next() != Some("ELIOT_SEARCH_CONTROL_STATE_V1") {
            return Err("CONTROL_STORE_HEADER_MISMATCH".to_owned());
        }
        let mut schema = None;
        let mut generation = None;
        let mut lifecycle = None;
        let mut pid = None;
        let mut started_unix_ms = None;
        let mut recovered_previous_active = None;
        let mut snapshot_present = None;
        let mut snapshot_id = None;
        let mut manifest_fingerprint = None;
        let mut fingerprint_algorithm = None;
        let mut indexed_files = None;
        let mut total_bytes = None;
        let mut capture_complete = None;

        for line in lines {
            let Some((key, value)) = line.split_once('=') else {
                return Err("CONTROL_STORE_STATE_MALFORMED".to_owned());
            };
            match key {
                "schema" => schema = Some(parse_u64(value)?),
                "generation" => generation = Some(parse_u64(value)?),
                "lifecycle" => lifecycle = Some(Lifecycle::parse(value)?),
                "pid" => {
                    pid = Some(
                        value
                            .parse::<u32>()
                            .map_err(|_| "CONTROL_STORE_PID_INVALID".to_owned())?,
                    );
                }
                "started_unix_ms" => {
                    started_unix_ms = Some(
                        value
                            .parse::<u128>()
                            .map_err(|_| "CONTROL_STORE_TIME_INVALID".to_owned())?,
                    );
                }
                "recovered_previous_active" => {
                    recovered_previous_active = Some(parse_bool(value)?);
                }
                "snapshot_present" => snapshot_present = Some(parse_bool(value)?),
                "snapshot_id" => snapshot_id = Some(value.to_owned()),
                "manifest_fingerprint" => manifest_fingerprint = Some(value.to_owned()),
                "fingerprint_algorithm" => fingerprint_algorithm = Some(value.to_owned()),
                "indexed_files" => {
                    indexed_files = Some(
                        value
                            .parse::<usize>()
                            .map_err(|_| "CONTROL_STORE_COUNT_INVALID".to_owned())?,
                    );
                }
                "total_bytes" => total_bytes = Some(parse_u64(value)?),
                "capture_complete" => capture_complete = Some(parse_bool(value)?),
                _ => return Err("CONTROL_STORE_UNKNOWN_FIELD".to_owned()),
            }
        }

        let schema = schema.ok_or_else(|| "CONTROL_STORE_SCHEMA_MISSING".to_owned())?;
        if schema != SCHEMA_VERSION {
            return Err("CONTROL_STORE_SCHEMA_MISMATCH".to_owned());
        }
        let snapshot = match snapshot_present
            .ok_or_else(|| "CONTROL_STORE_SNAPSHOT_FLAG_MISSING".to_owned())?
        {
            true => {
                let snapshot = SnapshotControl {
                    snapshot_id: snapshot_id
                        .ok_or_else(|| "CONTROL_STORE_SNAPSHOT_ID_MISSING".to_owned())?,
                    manifest_fingerprint: manifest_fingerprint.ok_or_else(|| {
                        "CONTROL_STORE_MANIFEST_FINGERPRINT_MISSING".to_owned()
                    })?,
                    fingerprint_algorithm: fingerprint_algorithm.ok_or_else(|| {
                        "CONTROL_STORE_FINGERPRINT_ALGORITHM_MISSING".to_owned()
                    })?,
                    indexed_files: indexed_files
                        .ok_or_else(|| "CONTROL_STORE_INDEXED_FILES_MISSING".to_owned())?,
                    total_bytes: total_bytes
                        .ok_or_else(|| "CONTROL_STORE_TOTAL_BYTES_MISSING".to_owned())?,
                    capture_complete: capture_complete
                        .ok_or_else(|| "CONTROL_STORE_CAPTURE_STATE_MISSING".to_owned())?,
                };
                snapshot.validate()?;
                Some(snapshot)
            }
            false => {
                if snapshot_id.is_some()
                    || manifest_fingerprint.is_some()
                    || fingerprint_algorithm.is_some()
                    || indexed_files.is_some()
                    || total_bytes.is_some()
                    || capture_complete.is_some()
                {
                    return Err("CONTROL_STORE_SNAPSHOT_FIELDS_UNEXPECTED".to_owned());
                }
                None
            }
        };
        Ok(Self {
            schema,
            generation: generation
                .ok_or_else(|| "CONTROL_STORE_GENERATION_MISSING".to_owned())?,
            lifecycle: lifecycle
                .ok_or_else(|| "CONTROL_STORE_LIFECYCLE_MISSING".to_owned())?,
            pid: pid.ok_or_else(|| "CONTROL_STORE_PID_MISSING".to_owned())?,
            started_unix_ms: started_unix_ms
                .ok_or_else(|| "CONTROL_STORE_TIME_MISSING".to_owned())?,
            recovered_previous_active: recovered_previous_active
                .ok_or_else(|| "CONTROL_STORE_RECOVERY_FLAG_MISSING".to_owned())?,
            snapshot,
        })
    }
}

/// Alternating-file lifecycle store with exact readback after every write.
pub(crate) struct DevelopmentControlStore {
    directory: PathBuf,
    active_slot: Slot,
    state: ControlState,
}

impl DevelopmentControlStore {
    pub(crate) fn open(data_root: &Path) -> Result<Self, String> {
        let directory = data_root.join("control");
        fs::create_dir_all(&directory)
            .map_err(|_| "CONTROL_STORE_DIRECTORY_FAILED".to_owned())?;
        let state_a = read_slot(&directory, Slot::A);
        let state_b = read_slot(&directory, Slot::B);
        let latest = newest_valid(&state_a, &state_b)?;
        let recovered_previous_active = latest
            .as_ref()
            .is_some_and(|(_, state)| state.lifecycle != Lifecycle::Stopped);
        let generation = latest
            .as_ref()
            .map_or(Ok(1_u64), |(_, state)| {
                state
                    .generation
                    .checked_add(1)
                    .ok_or_else(|| "CONTROL_STORE_GENERATION_EXHAUSTED".to_owned())
            })?;
        let active_slot = latest.map_or(Slot::A, |(slot, _)| slot.other());
        let state = ControlState {
            schema: SCHEMA_VERSION,
            generation,
            lifecycle: Lifecycle::Starting,
            pid: std::process::id(),
            started_unix_ms: unix_millis()?,
            recovered_previous_active,
            snapshot: None,
        };
        write_slot(&directory, active_slot, &state)?;
        Ok(Self {
            directory,
            active_slot,
            state,
        })
    }

    pub(crate) fn publish_ready(
        &mut self,
        snapshot: SnapshotControl,
    ) -> Result<(), String> {
        snapshot.validate()?;
        self.advance(Lifecycle::Ready, Some(snapshot))
    }

    pub(crate) fn mark_stopped(&mut self) -> Result<(), String> {
        self.advance(Lifecycle::Stopped, self.state.snapshot.clone())
    }

    pub(crate) fn generation(&self) -> u64 {
        self.state.generation
    }

    pub(crate) fn recovered_previous_active(&self) -> bool {
        self.state.recovered_previous_active
    }

    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }

    fn advance(
        &mut self,
        lifecycle: Lifecycle,
        snapshot: Option<SnapshotControl>,
    ) -> Result<(), String> {
        let generation = self
            .state
            .generation
            .checked_add(1)
            .ok_or_else(|| "CONTROL_STORE_GENERATION_EXHAUSTED".to_owned())?;
        let next = ControlState {
            schema: SCHEMA_VERSION,
            generation,
            lifecycle,
            pid: self.state.pid,
            started_unix_ms: self.state.started_unix_ms,
            recovered_previous_active: self.state.recovered_previous_active,
            snapshot,
        };
        let slot = self.active_slot.other();
        write_slot(&self.directory, slot, &next)?;
        self.active_slot = slot;
        self.state = next;
        Ok(())
    }
}

fn newest_valid(
    state_a: &Result<Option<ControlState>, String>,
    state_b: &Result<Option<ControlState>, String>,
) -> Result<Option<(Slot, ControlState)>, String> {
    let a = state_a.as_ref().ok().and_then(Clone::clone);
    let b = state_b.as_ref().ok().and_then(Clone::clone);
    match (a, b) {
        (Some(a), Some(b)) => {
            if a.generation == b.generation && a != b {
                return Err("CONTROL_STORE_GENERATION_CONFLICT".to_owned());
            }
            Ok(Some(if a.generation >= b.generation {
                (Slot::A, a)
            } else {
                (Slot::B, b)
            }))
        }
        (Some(a), None) => Ok(Some((Slot::A, a))),
        (None, Some(b)) => Ok(Some((Slot::B, b))),
        (None, None) => {
            if state_a.is_err() && state_b.is_err() {
                Err("CONTROL_STORE_BOTH_SLOTS_CORRUPT".to_owned())
            } else {
                Ok(None)
            }
        }
    }
}

fn read_slot(directory: &Path, slot: Slot) -> Result<Option<ControlState>, String> {
    let path = directory.join(slot.file_name());
    if !path.exists() {
        return Ok(None);
    }
    let mut file = File::open(&path).map_err(|_| "CONTROL_STORE_OPEN_FAILED".to_owned())?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(u64::try_from(MAX_STATE_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| "CONTROL_STORE_READ_FAILED".to_owned())?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err("CONTROL_STORE_STATE_TOO_LARGE".to_owned());
    }
    ControlState::parse(&bytes).map(Some)
}

fn write_slot(directory: &Path, slot: Slot, state: &ControlState) -> Result<(), String> {
    let bytes = state.serialize()?;
    let path = directory.join(slot.file_name());
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|_| "CONTROL_STORE_WRITE_OPEN_FAILED".to_owned())?;
    file.write_all(&bytes)
        .map_err(|_| "CONTROL_STORE_WRITE_FAILED".to_owned())?;
    file.sync_all()
        .map_err(|_| "CONTROL_STORE_SYNC_FAILED".to_owned())?;
    let readback = read_slot(directory, slot)?
        .ok_or_else(|| "CONTROL_STORE_READBACK_MISSING".to_owned())?;
    if &readback != state {
        return Err("CONTROL_STORE_READBACK_MISMATCH".to_owned());
    }
    Ok(())
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| "CONTROL_STORE_INTEGER_INVALID".to_owned())
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("CONTROL_STORE_BOOLEAN_INVALID".to_owned()),
    }
}

fn unix_millis() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|_| "CONTROL_STORE_CLOCK_INVALID".to_owned())
}

#[allow(dead_code)]
fn io_error(code: &'static str) -> io::Error {
    io::Error::new(ErrorKind::Other, code)
}
