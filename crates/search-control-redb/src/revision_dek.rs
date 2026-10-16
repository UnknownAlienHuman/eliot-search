//! Reserved revision-DEK metadata; Core owns its semantic codecs and policy.
use crate::{ConditionalControlMutation, ControlError, ControlRecordClass, ControlValue};
use search_contracts::{CanonicalValue as V, parse_canonical_cbor};

/// Sole immutable preparation intent in this data root's fresh profile.
pub const REVISION_DEK_INTENT_KEY: &[u8] = b"secret/revision-dek/v1/intent";
/// Sole non-serving candidate; this is never a Current selection or key lease.
pub const REVISION_DEK_CANDIDATE_KEY: &[u8] = b"secret/revision-dek/v1/candidate";
/// Current key; only the closed first-selection route may publish it.
pub const REVISION_DEK_CURRENT_KEY: &[u8] = b"secret/revision-dek/v1/current";
/// Retained consumer-scoped backend application intent for the first selection.
pub const REVISION_DEK_APPLICATION_INTENT_KEY: &[u8] = b"secret/revision-dek/v1/application-intent";

pub(crate) fn reserved(key: &[u8]) -> bool { key.starts_with(b"secret/revision-dek/v1/") }

/// Closed conditional command.
///
/// Construction validates framing, not Core facts, Native custody, credential
/// existence or permission. The caller accounts for
/// pure preparation; dispatch separately validates the executing journal limits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionDekCommand { command: ConditionalControlMutation }

impl RevisionDekCommand {
    /// Accepts only Intent/all-absent or Candidate/exact-Intent/others-absent.
    /// # Errors
    /// Rejects extra keys, deletes, missing/extra conditions or noncanonical
    /// State metadata with a wrong marker, codec or kind. Full semantic decoding
    /// remains Core's responsibility; the remaining four fields stay opaque.
    pub fn new(command: ConditionalControlMutation) -> Result<Self, ControlError> {
        let [write] = command.mutation().writes() else { return Err(ControlError::InvalidKey); };
        if !command.mutation().deletes().is_empty() || command.conditions().len() != 3 {
            return Err(ControlError::InvalidKey);
        }
        let kind = match write.key.as_bytes() {
            REVISION_DEK_INTENT_KEY => 0,
            REVISION_DEK_CANDIDATE_KEY => 1,
            _ => return Err(ControlError::InvalidKey),
        };
        record(&write.value, kind)?;
        for key in [REVISION_DEK_INTENT_KEY, REVISION_DEK_CANDIDATE_KEY, REVISION_DEK_CURRENT_KEY] {
            let condition = command.conditions().iter().find(|condition| condition.key().as_bytes() == key)
                .ok_or(ControlError::InvalidKey)?;
            match (kind == 1 && key == REVISION_DEK_INTENT_KEY, condition.expected()) {
                (true, Some(value)) => record(value, 0)?,
                (false, None) => {}
                _ => return Err(ControlError::InvalidValue),
            }
        }
        Ok(Self { command })
    }

    /// Immutable exact command retained for readback, replay and recovery.
    #[must_use]
    pub const fn command(&self) -> &ConditionalControlMutation { &self.command }
}

fn record(value: &ControlValue, kind: u64) -> Result<(), ControlError> {
    if value.class() != ControlRecordClass::State { return Err(ControlError::InvalidValue); }
    let V::Array(fields) = parse_canonical_cbor(value.as_bytes()).map_err(|_| ControlError::InvalidValue)?
        else { return Err(ControlError::InvalidValue); };
    match fields.as_slice() {
        [V::Text(marker), V::U64(1), V::U64(actual), _, _, _, _]
            if marker.as_str() == "eliot.revision-dek.record.v1" && *actual == kind => Ok(()),
        _ => Err(ControlError::InvalidValue),
    }
}

/// Closed first-selection command; framing proves no backend or Native authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionDekSelectionCommand { command: ConditionalControlMutation }

impl RevisionDekSelectionCommand {
    /// Accepts one application Intent or first Current against exact predecessors.
    /// # Errors
    /// Rejects deletes, unrelated keys, noncanonical frames and any condition set
    /// except exact original Intent/Candidate, absent/exact application Intent,
    /// and absent Current. Core owns the complete bodies and their semantic joins.
    pub fn new(command: ConditionalControlMutation) -> Result<Self, ControlError> {
        let [write] = command.mutation().writes() else { return Err(ControlError::InvalidKey); };
        if !command.mutation().deletes().is_empty() || command.conditions().len() != 4 {
            return Err(ControlError::InvalidKey);
        }
        let kind = match write.key.as_bytes() {
            REVISION_DEK_APPLICATION_INTENT_KEY => 0,
            REVISION_DEK_CURRENT_KEY => 1,
            _ => return Err(ControlError::InvalidKey),
        };
        selection_record(&write.value, kind)?;
        for key in [REVISION_DEK_INTENT_KEY, REVISION_DEK_CANDIDATE_KEY,
            REVISION_DEK_APPLICATION_INTENT_KEY, REVISION_DEK_CURRENT_KEY] {
            let condition = command.conditions().iter().find(|condition| condition.key().as_bytes() == key)
                .ok_or(ControlError::InvalidKey)?;
            match (key, condition.expected()) {
                (REVISION_DEK_INTENT_KEY, Some(value)) => record(value, 0)?,
                (REVISION_DEK_CANDIDATE_KEY, Some(value)) => record(value, 1)?,
                (REVISION_DEK_APPLICATION_INTENT_KEY, Some(value)) if kind == 1 => selection_record(value, 0)?,
                (REVISION_DEK_APPLICATION_INTENT_KEY, None) if kind == 0 => {}
                (REVISION_DEK_CURRENT_KEY, None) => {}
                _ => return Err(ControlError::InvalidValue),
            }
        }
        Ok(Self { command })
    }

    /// Exact command, including historical bytes, retained unchanged for recovery.
    #[must_use]
    pub const fn command(&self) -> &ConditionalControlMutation { &self.command }
}

fn selection_record(value: &ControlValue, kind: u64) -> Result<(), ControlError> {
    if value.class() != ControlRecordClass::State { return Err(ControlError::InvalidValue); }
    let V::Array(fields) = parse_canonical_cbor(value.as_bytes()).map_err(|_| ControlError::InvalidValue)?
        else { return Err(ControlError::InvalidValue); };
    let [V::Text(marker), V::U64(1), V::U64(actual), V::Array(body)] = fields.as_slice()
        else { return Err(ControlError::InvalidValue); };
    if marker.as_str() != "eliot.revision-dek.selection.v1" || *actual != kind {
        return Err(ControlError::InvalidValue);
    }
    match (kind, body.as_slice()) {
        (0, intent) if intent.len() == 8 => Ok(()),
        (1, [V::Array(intent), _, _, _]) if intent.as_slice().len() == 8 => Ok(()),
        _ => Err(ControlError::InvalidValue),
    }
}
