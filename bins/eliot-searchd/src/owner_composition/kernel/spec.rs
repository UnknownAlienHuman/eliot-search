//! Closed persisted owner format and lifecycle vocabulary.

use search_runtime_owner::{DrainReason, OwnerError};

pub(super) const INSTALLATION_FILE: &str = ".eliot-search-installation.v1";
pub(super) const OWNER_SLOT_A: &str = ".eliot-search-owner-state-a.v1";
pub(super) const OWNER_SLOT_B: &str = ".eliot-search-owner-state-b.v1";
pub(super) const INSTALLATION_MAGIC: &str = "ELIOT-SEARCH-INSTALLATION-V1";
pub(super) const OWNER_STATE_MAGIC: &str = "ELIOT-SEARCH-OWNER-STATE-V1";
pub(super) const FORMAT_VERSION_LINE: &str = "format_version=1";
pub(super) const MAX_STATE_BYTES: usize = 4 * 1024;
pub(super) const MAX_INSTALLATION_BYTES: usize = 1024;
pub(super) const MAX_EXECUTABLE_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const READ_CHUNK_BYTES: usize = 64 * 1024;

/// Durable lifecycle persisted in the owner-state slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LifecycleState {
    Active,
    Draining,
    Released,
}

impl LifecycleState {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Draining => "DRAINING",
            Self::Released => "RELEASED",
        }
    }

    pub(super) fn parse(value: &str) -> Result<Self, OwnerError> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "DRAINING" => Ok(Self::Draining),
            "RELEASED" => Ok(Self::Released),
            _ => Err(OwnerError::OwnerRecoveryQuarantined),
        }
    }
}

/// Durable drain reason mirroring the policy vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DrainReasonText {
    None,
    Shutdown,
    Restart,
    ModeOrRootChange,
    Maintenance,
}

impl DrainReasonText {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Shutdown => "SHUTDOWN",
            Self::Restart => "RESTART",
            Self::ModeOrRootChange => "MODE_OR_ROOT_CHANGE",
            Self::Maintenance => "MAINTENANCE",
        }
    }

    pub(super) const fn from_policy(reason: DrainReason) -> Self {
        match reason {
            DrainReason::Shutdown => Self::Shutdown,
            DrainReason::Restart => Self::Restart,
            DrainReason::ModeOrRootChange => Self::ModeOrRootChange,
            DrainReason::Maintenance => Self::Maintenance,
        }
    }

    pub(super) fn parse(value: &str) -> Result<Self, OwnerError> {
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
pub(super) enum Slot {
    A,
    B,
}

impl Slot {
    pub(super) const fn file_name(self) -> &'static str {
        match self {
            Self::A => OWNER_SLOT_A,
            Self::B => OWNER_SLOT_B,
        }
    }
}
