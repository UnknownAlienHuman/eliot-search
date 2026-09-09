//! Exact technical-record preconditions for the existing disk transaction engine.
//!
//! These values do not grant authority, identify an H5 table, or decide whether
//! an opaque payload is content-free. Capability-owned codecs still supply that
//! meaning. No source bodies, live native handles or authorization callbacks are
//! introduced by this comparison primitive.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::{ControlError, ControlKey, ControlMutation, ControlRecordClass, ControlValue, JournalLimits};

/// An exact pre-state assertion about one technical record.
///
/// The record may also be written/deleted by the associated mutation. Equality
/// includes the semantic class and every byte, not just a digest or length.
/// Debug inherits the key/value types' redaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlRecordCondition {
    key: ControlKey,
    expected: Option<ControlValue>,
}

impl ControlRecordCondition {
    /// Requires the key to be absent before the transaction changes any record.
    #[must_use]
    pub const fn absent(key: ControlKey) -> Self {
        Self { key, expected: None }
    }

    /// Requires the exact class and bytes to exist before staging any changes.
    #[must_use]
    pub const fn exact(key: ControlKey, value: ControlValue) -> Self {
        Self { key, expected: Some(value) }
    }

    /// Technical key whose pre-state is compared.
    #[must_use]
    pub const fn key(&self) -> &ControlKey { &self.key }

    /// Expected exact value, or `None` for required absence.
    #[must_use]
    pub const fn expected(&self) -> Option<&ControlValue> { self.expected.as_ref() }
}

/// One ordinary control mutation plus atomic record preconditions.
///
/// This is a command, not another journal. The existing global expected generation
/// remains mandatory, so an ABA change cannot be hidden by equal record bytes.
/// An empty condition list has exactly the existing mutation's replay identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalControlMutation {
    mutation: ControlMutation,
    conditions: Vec<ControlRecordCondition>,
}

impl ConditionalControlMutation {
    /// Creates a command. The executing journal validates all active byte/item
    /// limits and duplicate conditions before database dispatch.
    #[must_use]
    pub const fn new(mutation: ControlMutation, conditions: Vec<ControlRecordCondition>) -> Self {
        Self { mutation, conditions }
    }

    /// The exact existing mutation, including its operation ID and generation.
    #[must_use]
    pub const fn mutation(&self) -> &ControlMutation { &self.mutation }

    /// Pre-state conditions; their order does not affect replay identity.
    #[must_use]
    pub fn conditions(&self) -> &[ControlRecordCondition] { &self.conditions }

    pub(crate) fn into_parts(self) -> (ControlMutation, Vec<ControlRecordCondition>) {
        (self.mutation, self.conditions)
    }
}

pub fn validate_conditions(
    mutation: &ControlMutation,
    conditions: &[ControlRecordCondition],
    limits: JournalLimits,
    mut checkpoint: impl FnMut() -> Result<(), ControlError>,
) -> Result<(), ControlError> {
    // The existing validator separately requires at least one write or delete.
    // A condition on a touched key counts again: it is additional bounded work.
    let count = mutation.writes().len().checked_add(mutation.deletes().len())
        .and_then(|count| count.checked_add(conditions.len()))
        .ok_or(ControlError::BudgetExceeded)?;
    if count > limits.max_mutation_items { return Err(ControlError::BudgetExceeded); }
    let mut keys = BTreeSet::new();
    let mut value_bytes = 0_usize;
    for condition in conditions {
        checkpoint()?;
        let key = condition.key().as_bytes();
        if key.is_empty() || key.len() > limits.max_key_bytes { return Err(ControlError::InvalidKey); }
        if !keys.insert(key) { return Err(ControlError::DuplicateMutationKey); }
        if let Some(value) = condition.expected() {
            if value.is_empty() || value.len() > limits.max_value_bytes { return Err(ControlError::InvalidValue); }
            value_bytes = value_bytes.checked_add(value.len()).ok_or(ControlError::BudgetExceeded)?;
            if value_bytes > limits.max_total_value_bytes { return Err(ControlError::BudgetExceeded); }
        }
    }
    Ok(())
}

/// Bind preconditions to the actual existing request fingerprint, not to a
/// caller's declared digest. This private SHA-256 is never a `Blake3Digest32`.
pub fn bind_conditions(
    request_sha256: [u8; 32],
    conditions: &[ControlRecordCondition],
    mut checkpoint: impl FnMut() -> Result<(), ControlError>,
) -> Result<[u8; 32], ControlError> {
    if conditions.is_empty() { return Ok(request_sha256); }
    // Called only after the executing journal has validated finite counts/bytes.
    let mut ordered = conditions.iter().collect::<Vec<_>>();
    ordered.sort_unstable_by(|left, right| left.key().cmp(right.key()));
    let mut hash = Sha256::new();
    hash.update(b"eliot-search/control-conditional-request/sha256/v1\0");
    hash.update(request_sha256);
    hash.update(length(ordered.len())?);
    for condition in ordered {
        checkpoint()?;
        hash.update(length(condition.key().as_bytes().len())?);
        hash.update(condition.key().as_bytes());
        match condition.expected() {
            None => hash.update([0]),
            Some(value) => {
                hash.update([1]);
                hash.update([class_tag(value.class())]);
                hash.update(length(value.len())?);
                hash.update(value.as_bytes());
            }
        }
    }
    Ok(hash.finalize().into())
}

fn length(value: usize) -> Result<[u8; 8], ControlError> {
    u64::try_from(value).map(u64::to_be_bytes).map_err(|_| ControlError::BudgetExceeded)
}

// Fixed tags belong to this conditional-request preimage, not enum discriminants
// or a change to the persisted value codec. Do not use Debug/JSON for identity.
const fn class_tag(class: ControlRecordClass) -> u8 {
    match class {
        ControlRecordClass::Identity => 1,
        ControlRecordClass::Revision => 2,
        ControlRecordClass::State => 3,
        ControlRecordClass::Receipt => 4,
        ControlRecordClass::Operation => 5,
        ControlRecordClass::Snapshot => 6,
        ControlRecordClass::Migration => 7,
    }
}
