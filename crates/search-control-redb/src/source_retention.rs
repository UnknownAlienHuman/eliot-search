//! Closed technical key-use accounting on the existing control journal.
//!
//! Genesis precedes key material. Exact material and the permanent residency
//! claim survive selection, release and purge. This module grants no Native,
//! source, access, key-selection or cryptographic authority.

use core::fmt;
use search_contracts::{self as c, Blake3Digest32, CanonicalValue as V, OpaqueId};
use crate::{ConditionalControlMutation, ControlError, ControlKey, ControlMutation,
    ControlRecordClass, ControlRecordCondition, ControlValue, ControlWrite,
    JournalIdentity, JournalLimits, MutationId, ScopedControlRecordsReadback};

/// Single-key selected-reference operating profile, never inferred from absence.
pub const REVISION_USE_STATE_KEY: &[u8] = b"secret/revision-use/v2/state";
const PREFIX: &[u8] = b"secret/revision-use/v2/";
const RESERVATION_PREFIX: &[u8] = b"secret/revision-use/v2/reservation/";
const STATE_MARKER: &str = "eliot.revision-use.state.v2";
const RESERVATION_MARKER: &str = "eliot.revision-use.reservation.v2";
/// Finite reserved invocations per exact key, including uncertain invocations.
pub const MAX_REVISION_USE_ATTEMPTS: u64 = 1_u64 << 32;
/// Finite cumulative authenticated-block charge per exact key.
pub const MAX_REVISION_USE_BLOCKS: u64 = 1_u64 << 32;
/// Finite cumulative plaintext-byte charge per exact key.
pub const MAX_REVISION_USE_PLAINTEXT_BYTES: u64 = 1_u64 << 36;
const MAX_OWNER_RECORD: usize = 8192;
const MAX_AAD: usize = 32 * 1024;

pub(crate) fn reserved(key: &[u8]) -> bool { key.starts_with(PREFIX) }

/// Keys required together for genesis, material binding and use preparation.
/// # Errors
/// Active journal key limits must admit the fixed profile keys.
pub fn revision_use_keys(limits: JournalLimits) -> Result<[ControlKey; 4], ControlError> {
    Ok([key(REVISION_USE_STATE_KEY, limits)?, key(crate::REVISION_DEK_INTENT_KEY, limits)?,
        key(crate::REVISION_DEK_CANDIDATE_KEY, limits)?, key(crate::REVISION_DEK_CURRENT_KEY, limits)?])
}

/// Complete point-read scope for preparing one operation-addressed reservation.
///
/// The fifth key must be explicitly observed absent; a four-key genesis/material
/// read alone cannot establish its absence.
/// # Errors
/// Refuses keys exceeding the executing journal limits.
pub fn revision_use_reservation_keys(id: MutationId, limits: JournalLimits)
    -> Result<[ControlKey; 5], ControlError> {
    let [usage, intent, candidate, current] = revision_use_keys(limits)?;
    Ok([usage, intent, candidate, current, revision_seal_reservation_key(id, limits)?])
}

/// Full operation-addressed reservation key; no truncation or second ledger.
/// # Errors
/// Refuses a key exceeding the executing journal limits.
pub fn revision_seal_reservation_key(id: MutationId, limits: JournalLimits) -> Result<ControlKey, ControlError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = RESERVATION_PREFIX.to_vec();
    for byte in id.0 {
        bytes.push(HEX[usize::from(byte >> 4)]);
        bytes.push(HEX[usize::from(byte & 15)]);
    }
    key(&bytes, limits)
}

#[derive(Clone, Eq, PartialEq)]
struct UsageState {
    preparation: Vec<u8>,
    material_intent: Option<Vec<u8>>,
    residency: Option<Vec<u8>>,
    attempts: u64,
    blocks: u64,
    plaintext_bytes: u64,
}

/// Exact owner metadata captured before sealing.
///
/// All byte fields are canonical metadata, never plaintext.
/// The crypto owner must reconstruct and compare AAD
/// before use; this technical owner does not implement the crypto codec.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionSealMetadata {
    /// Original retention operation, distinct from admission and phase IDs.
    pub operation_id: OpaqueId,
    /// Complete staged occurrence; physical ciphertext identity is absent.
    pub preseal_occurrence_canonical: Vec<u8>,
    /// Complete six-domain residency binding, including original UUIDs/bodies.
    pub residency_canonical: Vec<u8>,
    /// Exact final crypto-owner AAD with this reservation's counter.
    pub aad_canonical: Vec<u8>,
    /// Exact source-body length charged before AES.
    pub plaintext_length: u64,
    /// Exact authoritative source-body digest, checked by the crypto owner.
    pub content_digest: Blake3Digest32,
}

impl fmt::Debug for RevisionSealMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RevisionSealMetadata([REDACTED])")
    }
}

#[derive(Clone, Eq, PartialEq)]
struct SealUse {
    current: Vec<u8>,
    metadata: RevisionSealMetadata,
    counter: u64,
    blocks: u64,
}

/// Immutable conditional command prepared from genuine scoped disk readback.
/// Holding or cloning it is not permission to encrypt.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionUseCommand {
    identity: JournalIdentity,
    command: ConditionalControlMutation,
    seal: Option<SealUse>,
}

impl fmt::Debug for RevisionUseCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RevisionUseCommand").field("seal", &self.seal.is_some()).finish_non_exhaustive()
    }
}

impl RevisionUseCommand {
    /// Retains explicit `NoMaterial` genesis while all original key rows are
    /// proven absent. Only a freshly acknowledged commit may precede RNG.
    /// # Errors
    /// Missing read scope, existing usage/material or noncanonical binding refuses.
    pub fn genesis(read: &ScopedControlRecordsReadback, id: MutationId,
        digest: Blake3Digest32, preparation: &[u8], limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        canonical(preparation, MAX_OWNER_RECORD)?;
        let keys = revision_use_keys(limits)?;
        let conditions = keys.iter().map(|key| absent(read, key)).collect::<Result<Vec<_>, _>>()?;
        let state = UsageState { preparation: preparation.to_vec(), material_intent: None,
            residency: None, attempts: 0, blocks: 0, plaintext_bytes: 0 };
        Self::build(read, id, digest, vec![ControlWrite { key: keys[0].clone(),
            value: state.encode(limits)? }], conditions, None)
    }

    /// Binds the exact durably retained Core Intent before a credential effect.
    /// There is no transition back to `NoMaterial` or another material binding.
    /// # Errors
    /// Foreign preparation, absent Intent or existing Candidate/Current refuses.
    pub fn bind_material(read: &ScopedControlRecordsReadback, id: MutationId,
        digest: Blake3Digest32, preparation: &[u8], limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        let keys = revision_use_keys(limits)?;
        let old = present(read, &keys[0])?;
        let mut state = UsageState::decode(old)?;
        if state.preparation != preparation || state.material_intent.is_some() {
            return Err(ControlError::OperationConflict);
        }
        let intent = present(read, &keys[1])?;
        canonical(intent.as_bytes(), MAX_OWNER_RECORD)?;
        state.material_intent = Some(intent.as_bytes().to_vec());
        let conditions = vec![ControlRecordCondition::exact(keys[0].clone(), old.clone()),
            ControlRecordCondition::exact(keys[1].clone(), intent.clone()),
            absent(read, &keys[2])?, absent(read, &keys[3])?];
        Self::build(read, id, digest, vec![ControlWrite { key: keys[0].clone(),
            value: state.encode(limits)? }], conditions, None)
    }

    /// Observes the next counter only from a matching, explicitly bound state.
    /// This is preparation metadata, never a use permit.
    /// # Errors
    /// Absent/incomplete genesis, foreign material/preparation or exhaustion refuses.
    pub fn next_counter(read: &ScopedControlRecordsReadback, preparation: &[u8],
        limits: JournalLimits,
    ) -> Result<u64, ControlError> {
        let (state, _) = bound_state(read, preparation, limits)?;
        if state.attempts >= MAX_REVISION_USE_ATTEMPTS { return Err(ControlError::BudgetExceeded); }
        Ok(state.attempts)
    }

    /// Atomically claims exact residency, burns finite use, and retains the
    /// complete original reservation. Release/purge never resets this state.
    /// # Errors
    /// Incomplete read scope, key/residency conflict, stale counter or finite-cap
    /// exhaustion refuses before any crypto or filesystem effect.
    #[allow(clippy::too_many_arguments)]
    pub fn reserve(read: &ScopedControlRecordsReadback, id: MutationId,
        digest: Blake3Digest32, preparation: &[u8], counter: u64,
        metadata: RevisionSealMetadata, limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        canonical(&metadata.preseal_occurrence_canonical, MAX_OWNER_RECORD)?;
        canonical(&metadata.residency_canonical, MAX_OWNER_RECORD)?;
        canonical(&metadata.aad_canonical, MAX_AAD)?;
        let keys = revision_use_keys(limits)?;
        let (mut state, current) = bound_state(read, preparation, limits)?;
        if state.attempts != counter || state.residency.as_ref().is_some_and(|r| r != &metadata.residency_canonical) {
            return Err(ControlError::OperationConflict);
        }
        let aad = u64::try_from(metadata.aad_canonical.len()).map_err(|_| ControlError::BudgetExceeded)?;
        let blocks = metadata.plaintext_length.div_ceil(16).checked_add(aad.div_ceil(16))
            .and_then(|n| n.checked_add(1)).ok_or(ControlError::BudgetExceeded)?;
        state.attempts = charge(state.attempts, 1, MAX_REVISION_USE_ATTEMPTS)?;
        state.blocks = charge(state.blocks, blocks, MAX_REVISION_USE_BLOCKS)?;
        state.plaintext_bytes = charge(state.plaintext_bytes, metadata.plaintext_length, MAX_REVISION_USE_PLAINTEXT_BYTES)?;
        state.residency = Some(metadata.residency_canonical.clone());
        let seal = SealUse { current: current.as_bytes().to_vec(), metadata, counter, blocks };
        let reservation = revision_seal_reservation_key(id, limits)?;
        let conditions = vec![ControlRecordCondition::exact(keys[0].clone(), present(read, &keys[0])?.clone()),
            ControlRecordCondition::exact(keys[1].clone(), present(read, &keys[1])?.clone()),
            ControlRecordCondition::exact(keys[3].clone(), current.clone()), absent(read, &reservation)?];
        let writes = vec![ControlWrite { key: keys[0].clone(), value: state.encode(limits)? },
            ControlWrite { key: reservation, value: seal.encode(limits)? }];
        Self::build(read, id, digest, writes, conditions, Some(seal))
    }

    fn build(read: &ScopedControlRecordsReadback, id: MutationId, digest: Blake3Digest32,
        writes: Vec<ControlWrite>, conditions: Vec<ControlRecordCondition>, seal: Option<SealUse>,
    ) -> Result<Self, ControlError> {
        if read.identity().schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        Ok(Self { identity: read.identity(), command: ConditionalControlMutation::new(
            ControlMutation::new(id, digest, read.generation(), writes, vec![]), conditions), seal })
    }

    /// Exact original command retained for recovery, never rebased on replay.
    #[must_use]
    pub const fn command(&self) -> &ConditionalControlMutation { &self.command }
    pub(crate) const fn identity(&self) -> JournalIdentity { self.identity }
    pub(crate) fn permit_allocation(&self) -> usize {
        self.seal.as_ref().map_or(0, |seal| seal.current.len() + seal.metadata.preseal_occurrence_canonical.len()
            + seal.metadata.residency_canonical.len() + seal.metadata.aad_canonical.len()
            + seal.metadata.operation_id.as_str().len())
    }
    pub(crate) fn fresh_permit(&self) -> Option<CommittedRevisionSealUse> {
        self.seal.clone().map(|seal| CommittedRevisionSealUse { seal })
    }
}

/// One opaque, non-Clone committed reservation.
///
/// Only the checked port's fresh
/// successful commit constructs this value; replay/recovery cannot reissue it.
/// Windows consumes it once and must compare the exact crypto-owner AAD.
pub struct CommittedRevisionSealUse { seal: SealUse }

impl fmt::Debug for CommittedRevisionSealUse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("CommittedRevisionSealUse([REDACTED])") }
}

impl CommittedRevisionSealUse {
    /// Original retention operation.
    #[must_use]
    pub const fn metadata(&self) -> &RevisionSealMetadata { &self.seal.metadata }
    /// Exact Current record guarded by the reservation CAS.
    #[must_use]
    pub fn selected_current_canonical(&self) -> &[u8] { &self.seal.current }
    /// Permanently consumed counter; counter zero is valid.
    #[must_use]
    pub const fn counter(&self) -> u64 { self.seal.counter }
    /// Exact prepaid GHASH block count, including the final length block.
    #[must_use]
    pub const fn authenticated_blocks(&self) -> u64 { self.seal.blocks }
}

fn bound_state<'a>(read: &'a ScopedControlRecordsReadback, preparation: &[u8], limits: JournalLimits)
    -> Result<(UsageState, &'a ControlValue), ControlError> {
    let keys = revision_use_keys(limits)?;
    let state = UsageState::decode(present(read, &keys[0])?)?;
    let intent = present(read, &keys[1])?;
    if state.preparation != preparation || state.material_intent.as_deref() != Some(intent.as_bytes()) {
        return Err(ControlError::OperationConflict);
    }
    let current = present(read, &keys[3])?;
    canonical(current.as_bytes(), MAX_OWNER_RECORD)?;
    Ok((state, current))
}

fn present<'a>(read: &'a ScopedControlRecordsReadback, key: &ControlKey) -> Result<&'a ControlValue, ControlError> {
    let value = read.get(key).ok_or(ControlError::InvalidValue)?;
    if value.class() != ControlRecordClass::State { return Err(ControlError::InvalidValue); }
    Ok(value)
}
fn absent(read: &ScopedControlRecordsReadback, key: &ControlKey) -> Result<ControlRecordCondition, ControlError> {
    if read.get(key).is_some() || !read.missing_keys().contains(key) { return Err(ControlError::OperationConflict); }
    Ok(ControlRecordCondition::absent(key.clone()))
}
fn key(bytes: &[u8], limits: JournalLimits) -> Result<ControlKey, ControlError> { ControlKey::new(bytes.to_vec(), limits) }
fn charge(before: u64, count: u64, cap: u64) -> Result<u64, ControlError> {
    before.checked_add(count).filter(|n| *n <= cap).ok_or(ControlError::BudgetExceeded)
}
fn canonical(bytes: &[u8], cap: usize) -> Result<(), ControlError> {
    if bytes.is_empty() || bytes.len() > cap { return Err(ControlError::BudgetExceeded); }
    c::parse_canonical_cbor(bytes).map(|_| ()).map_err(|_| ControlError::InvalidValue)
}
fn text(value: &str) -> Result<V, ControlError> {
    Ok(V::Text(c::CanonicalText::new(value).map_err(|_| ControlError::InvalidValue)?))
}
fn bytes(value: &[u8]) -> Result<V, ControlError> {
    Ok(V::Bytes(c::BoundedBytes::new(value.to_vec()).map_err(|_| ControlError::InvalidValue)?))
}
fn optional(value: Option<&[u8]>) -> Result<V, ControlError> { value.map_or(Ok(V::Null), bytes) }
fn record(fields: Vec<V>, limits: JournalLimits) -> Result<ControlValue, ControlError> {
    let value = V::Array(c::BoundedList::new(fields).map_err(|_| ControlError::InvalidValue)?);
    let encoded = c::to_canonical_cbor(&value).map_err(|_| ControlError::InvalidValue)?;
    ControlValue::new(ControlRecordClass::State, encoded.as_slice().to_vec(), limits)
}

impl UsageState {
    fn encode(&self, limits: JournalLimits) -> Result<ControlValue, ControlError> {
        record(vec![text(STATE_MARKER)?, V::U64(2), bytes(&self.preparation)?,
            optional(self.material_intent.as_deref())?, optional(self.residency.as_deref())?,
            V::U64(self.attempts), V::U64(self.blocks), V::U64(self.plaintext_bytes)], limits)
    }
    fn decode(value: &ControlValue) -> Result<Self, ControlError> {
        if value.class() != ControlRecordClass::State { return Err(ControlError::StoreCorrupt); }
        let V::Array(fields) = c::parse_canonical_cbor(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?
            else { return Err(ControlError::StoreCorrupt); };
        let [V::Text(marker), V::U64(2), V::Bytes(preparation), material, residency,
            V::U64(attempts), V::U64(blocks), V::U64(plaintext_bytes)] = fields.as_slice()
            else { return Err(ControlError::StoreCorrupt); };
        if marker.as_str() != STATE_MARKER { return Err(ControlError::StoreCorrupt); }
        canonical(preparation.as_slice(), MAX_OWNER_RECORD)?;
        let decode_optional = |value: &V| match value {
            V::Null => Ok(None),
            V::Bytes(value) => { canonical(value.as_slice(), MAX_OWNER_RECORD)?; Ok(Some(value.as_slice().to_vec())) },
            _ => Err(ControlError::StoreCorrupt),
        };
        let result = Self { preparation: preparation.as_slice().to_vec(), material_intent: decode_optional(material)?,
            residency: decode_optional(residency)?, attempts: *attempts, blocks: *blocks, plaintext_bytes: *plaintext_bytes };
        if result.attempts > MAX_REVISION_USE_ATTEMPTS || result.blocks > MAX_REVISION_USE_BLOCKS
            || result.plaintext_bytes > MAX_REVISION_USE_PLAINTEXT_BYTES
            || (result.attempts == 0 && (result.blocks != 0 || result.plaintext_bytes != 0 || result.residency.is_some()))
            || (result.attempts > 0 && (result.material_intent.is_none() || result.residency.is_none() || result.blocks < result.attempts)) {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(result)
    }
}

impl SealUse {
    fn encode(&self, limits: JournalLimits) -> Result<ControlValue, ControlError> {
        record(vec![text(RESERVATION_MARKER)?, V::U64(2), bytes(&self.current)?,
            text(self.metadata.operation_id.as_str())?, bytes(&self.metadata.preseal_occurrence_canonical)?,
            bytes(&self.metadata.residency_canonical)?, bytes(&self.metadata.aad_canonical)?,
            bytes(self.metadata.content_digest.as_bytes())?, V::U64(self.metadata.plaintext_length),
            V::U64(self.counter), V::U64(self.blocks)], limits)
    }
}
