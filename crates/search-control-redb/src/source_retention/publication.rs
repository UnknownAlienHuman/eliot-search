//! Late physical-object publication intent and exact technical readback.
//!
//! The existing reservation row already binds the original retention operation,
//! the complete pre-seal occurrence, the six-domain residency claim, the exact
//! crypto-owner AAD, the content digest and length, and the burned counter. That
//! row is committed before AES and is never rewritten.
//!
//! After AES the ciphertext-derived physical object identity exists, and the C07
//! owner supplies its complete canonical object metadata. This module binds that
//! metadata exactly once, in a second immutable reserved row addressed by the
//! same original retention operation, with the exact committed reservation value
//! and the exact selected Current record as atomic preconditions. The binding
//! command carries no seal use, so committing it can never mint a
//! [`CommittedRevisionSealUse`](super::CommittedRevisionSealUse) or authorize AES.
//!
//! Everything produced here is technical metadata. No readback is serving,
//! publication, retention or access authority, and no decoded row is promoted
//! back into a seal-use permit.

use core::fmt;

use search_contracts::{self as c, Blake3Digest32, CanonicalValue as V, OpaqueId};

use crate::{ControlError, ControlKey, ControlRecordClass, ControlRecordCondition, ControlValue,
    ControlWrite, JournalLimits, MutationId, ScopedControlRecordsReadback};

use super::RevisionUseCommand;

/// Maximum complete canonical object metadata accepted from the C07 owner.
///
/// The object owner supplies one already canonical encoding of the derived
/// physical object identity and its complete publication metadata. This module
/// stores those bytes verbatim and never reinterprets them.
pub const MAX_OBJECT_METADATA_BYTES: usize = 8 * 1024;

/// Tightest bound for one encoded publication-intent row.
const MAX_OBJECT_INTENT_BYTES: usize = MAX_OBJECT_METADATA_BYTES + super::MAX_OWNER_RECORD + 1024;

/// Tightest bound for one decoded reservation row.
///
/// The frozen encoder frames the guarded Current, the pre-seal occurrence and
/// the residency claim each at the owner-record cap, plus the exact AAD. A
/// tighter decode cap would classify a legitimately large committed row as
/// corrupt, so the bound mirrors the encoder's own worst case.
const MAX_RESERVATION_READBACK_BYTES: usize =
    3 * super::MAX_OWNER_RECORD + super::MAX_AAD + 1024;

/// Operation-addressed key namespace for the late object publication intent.
const OBJECT_INTENT_PREFIX: &[u8] = b"secret/revision-use/v2/object-intent/";

/// Codec marker of one late object publication intent row.
const OBJECT_INTENT_MARKER: &str = "eliot.revision-use.object-intent.v2";

/// Codec version stored in, and accepted by, an object publication intent.
const OBJECT_INTENT_VERSION: u64 = 2;

/// Lowercase hex alphabet used to frame the operation address.
const HEX: &[u8; 16] = b"0123456789abcdef";

/// Full operation-addressed publication-intent key for one retention operation.
///
/// The address is the same original retention operation identity that already
/// addresses its reservation row, so a recovering owner locates both rows from
/// the recorded operation alone. No truncation, second ledger or alias is used.
///
/// # Errors
/// Refuses a key exceeding the executing journal limits.
pub fn object_publication_intent_key(operation: MutationId, limits: JournalLimits)
    -> Result<ControlKey, ControlError>
{
    let mut bytes = OBJECT_INTENT_PREFIX.to_vec();
    for byte in operation.0 {
        bytes.push(HEX[usize::from(byte >> 4)]);
        bytes.push(HEX[usize::from(byte & 15)]);
    }
    super::key(&bytes, limits)
}

/// Complete point-read scope for binding one late object publication intent.
///
/// The order is usage state, Core intent, Current, reservation and the new
/// publication-intent key. A narrower read cannot prove the new intent is
/// absent, and a read that omits any of these keys never reports it missing.
///
/// # Errors
/// Refuses keys exceeding the executing journal limits.
pub fn object_publication_intent_keys(operation: MutationId, limits: JournalLimits)
    -> Result<[ControlKey; 5], ControlError>
{
    let [usage, intent, _candidate, current] = super::revision_use_keys(limits)?;
    Ok([usage, intent, current, super::revision_seal_reservation_key(operation, limits)?,
        object_publication_intent_key(operation, limits)?])
}

/// Decoded form of one already committed seal reservation row.
///
/// This is a bounded technical decode of genuine scoped readback. It retains the
/// complete original metadata, the guarded Current bytes, the permanently
/// consumed counter and the prepaid authenticated-block charge. It deliberately
/// cannot produce a seal-use permit: only a fresh successful reservation commit
/// constructs [`CommittedRevisionSealUse`](super::CommittedRevisionSealUse), and
/// recovery never reissues one from stored bytes.
#[derive(Clone, Eq, PartialEq)]
pub struct SealReservationReadback {
    operation_id: OpaqueId,
    current_canonical: Vec<u8>,
    preseal_occurrence_canonical: Vec<u8>,
    residency_canonical: Vec<u8>,
    aad_canonical: Vec<u8>,
    content_digest: Blake3Digest32,
    plaintext_length: u64,
    counter: u64,
    blocks: u64,
}

impl fmt::Debug for SealReservationReadback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealReservationReadback").field("counter", &self.counter)
            .field("blocks", &self.blocks).finish_non_exhaustive()
    }
}

impl SealReservationReadback {
    /// Reads and decodes the committed reservation row of one retention operation.
    ///
    /// The row must be present in this exact read scope. A key that was never
    /// requested is a refused read scope, never an absent reservation.
    ///
    /// # Errors
    /// Missing, foreign-class, oversized or malformed rows refuse.
    pub fn read(read: &ScopedControlRecordsReadback, operation: MutationId, limits: JournalLimits)
        -> Result<Self, ControlError>
    {
        let key = super::revision_seal_reservation_key(operation, limits)?;
        Self::decode(super::present(read, &key)?)
    }

    fn decode(value: &ControlValue) -> Result<Self, ControlError> {
        if value.class() != ControlRecordClass::State || value.len() > MAX_RESERVATION_READBACK_BYTES {
            return Err(ControlError::StoreCorrupt);
        }
        let Ok(V::Array(fields)) = c::parse_canonical_cbor(value.as_bytes()) else {
            return Err(ControlError::StoreCorrupt);
        };
        let [V::Text(marker), V::U64(2), V::Bytes(current), V::Text(operation_id),
            V::Bytes(preseal), V::Bytes(residency), V::Bytes(aad), V::Bytes(digest),
            V::U64(plaintext_length), V::U64(counter), V::U64(blocks)] = fields.as_slice() else {
            return Err(ControlError::StoreCorrupt);
        };
        if marker.as_str() != super::RESERVATION_MARKER {
            return Err(ControlError::StoreCorrupt);
        }
        super::canonical(current.as_slice(), super::MAX_OWNER_RECORD)?;
        super::canonical(preseal.as_slice(), super::MAX_OWNER_RECORD)?;
        super::canonical(residency.as_slice(), super::MAX_OWNER_RECORD)?;
        super::canonical(aad.as_slice(), super::MAX_AAD)?;
        let row = Self {
            operation_id: OpaqueId::new(operation_id.as_str()).map_err(|_| ControlError::StoreCorrupt)?,
            current_canonical: current.as_slice().to_vec(),
            preseal_occurrence_canonical: preseal.as_slice().to_vec(),
            residency_canonical: residency.as_slice().to_vec(),
            aad_canonical: aad.as_slice().to_vec(),
            content_digest: digest_bytes(digest.as_slice())?,
            plaintext_length: *plaintext_length,
            counter: *counter,
            blocks: *blocks,
        };
        if row.plaintext_length > super::MAX_REVISION_USE_PLAINTEXT_BYTES
            || row.counter >= super::MAX_REVISION_USE_ATTEMPTS
            || row.blocks == 0 || row.blocks > super::MAX_REVISION_USE_BLOCKS
            || row.blocks < row.plaintext_length.div_ceil(16).saturating_add(1)
        {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(row)
    }

    /// Original retention operation, distinct from admission and phase identities.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId { &self.operation_id }
    /// Complete staged occurrence retained before the ciphertext existed.
    #[must_use]
    pub fn preseal_occurrence_canonical(&self) -> &[u8] { &self.preseal_occurrence_canonical }
    /// Complete six-domain residency claim, including original UUIDs and bodies.
    #[must_use]
    pub fn residency_canonical(&self) -> &[u8] { &self.residency_canonical }
    /// Exact crypto-owner AAD retained with this reservation's counter.
    #[must_use]
    pub fn aad_canonical(&self) -> &[u8] { &self.aad_canonical }
    /// Exact source-body length charged before AES.
    #[must_use]
    pub const fn plaintext_length(&self) -> u64 { self.plaintext_length }
    /// Exact authoritative source-body digest charged before AES.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 { self.content_digest }
    /// Exact Current record guarded by the original reservation compare-and-set.
    #[must_use]
    pub fn selected_current_canonical(&self) -> &[u8] { &self.current_canonical }
    /// Permanently consumed counter; counter zero is valid.
    #[must_use]
    pub const fn counter(&self) -> u64 { self.counter }
    /// Exact prepaid GHASH block count, including the final length block.
    #[must_use]
    pub const fn authenticated_blocks(&self) -> u64 { self.blocks }
}

/// Bounded technical view of the durable finite-use accounting row.
///
/// It exists so a later Source can reuse the genuinely persisted residency claim
/// instead of drawing six new domain identities and falsely conflicting with the
/// permanent claim on the same key. It grants no permit, reservation, counter
/// advance or write: it is a read-only projection of the private bound-state
/// check after that check has proven the complete genesis and material binding.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionUseUsageReadback {
    preparation: Vec<u8>,
    residency_canonical: Option<Vec<u8>>,
    attempts: u64,
    blocks: u64,
    plaintext_bytes: u64,
}

impl fmt::Debug for RevisionUseUsageReadback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RevisionUseUsageReadback").field("attempts", &self.attempts)
            .field("residency_claimed", &self.residency_canonical.is_some()).finish_non_exhaustive()
    }
}

impl RevisionUseUsageReadback {
    /// Reads the bound usage state for one exact preparation profile.
    ///
    /// Missing genesis, a foreign preparation, an unbound material intent or a
    /// missing/foreign Current record all refuse, because absence of that state
    /// never proves a fresh key. A valid bound state that has not yet reserved a
    /// residency returns no claim rather than a synthesized one.
    ///
    /// # Errors
    /// Incomplete read scope, unbound genesis or malformed stored state refuse.
    pub fn read(read: &ScopedControlRecordsReadback, preparation: &[u8], limits: JournalLimits)
        -> Result<Self, ControlError>
    {
        let (state, _current) = super::bound_state(read, preparation, limits)?;
        Ok(Self { preparation: state.preparation, residency_canonical: state.residency,
            attempts: state.attempts, blocks: state.blocks, plaintext_bytes: state.plaintext_bytes })
    }

    /// Exact preparation profile this usage state is bound to.
    #[must_use]
    pub fn preparation(&self) -> &[u8] { &self.preparation }
    /// Permanent residency claim in its complete canonical form, if one exists.
    ///
    /// Equal complete bodies reuse this exact value; a different claim on the
    /// same key is refused permanently by the reservation path.
    #[must_use]
    pub fn residency_canonical(&self) -> Option<&[u8]> { self.residency_canonical.as_deref() }
    /// Permanently consumed reservation attempts, including uncertain ones.
    #[must_use]
    pub const fn attempts(&self) -> u64 { self.attempts }
    /// Cumulative prepaid authenticated-block charge.
    #[must_use]
    pub const fn blocks(&self) -> u64 { self.blocks }
    /// Cumulative charged plaintext bytes.
    #[must_use]
    pub const fn plaintext_bytes(&self) -> u64 { self.plaintext_bytes }
}

/// Decoded form of one committed late object publication intent row.
///
/// The intent is immutable and complete: it retains the exact original retention
/// operation identity, the Current bytes and reservation coordinates it was
/// bound to, and the C07 owner's canonical object metadata verbatim. Recovery
/// therefore never re-encrypts to rediscover the physical object identity.
#[derive(Clone, Eq, PartialEq)]
pub struct ObjectPublicationIntentReadback {
    intent: ObjectIntent,
}

impl fmt::Debug for ObjectPublicationIntentReadback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectPublicationIntentReadback")
            .field("counter", &self.intent.counter).field("metadata_bytes", &self.intent.object_metadata.len())
            .finish_non_exhaustive()
    }
}

impl ObjectPublicationIntentReadback {
    /// Reads and decodes the publication intent of one retention operation.
    ///
    /// The row must be present in this exact read scope. A key that was never
    /// requested is a refused read scope, never an absent intent.
    ///
    /// # Errors
    /// Missing, foreign-class, oversized or malformed rows refuse.
    pub fn read(read: &ScopedControlRecordsReadback, operation: MutationId, limits: JournalLimits)
        -> Result<Self, ControlError>
    {
        let key = object_publication_intent_key(operation, limits)?;
        Ok(Self { intent: ObjectIntent::decode(super::present(read, &key)?)? })
    }

    /// Original retention operation carried by this intent.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId { &self.intent.operation_id }
    /// Exact Current record the bound object was published under.
    #[must_use]
    pub fn selected_current_canonical(&self) -> &[u8] { &self.intent.current_canonical }
    /// Exact authoritative source-body digest of the published object.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 { self.intent.content_digest }
    /// Exact charged source-body length.
    #[must_use]
    pub const fn plaintext_length(&self) -> u64 { self.intent.plaintext_length }
    /// Counter of the exact reservation this intent was bound to.
    #[must_use]
    pub const fn counter(&self) -> u64 { self.intent.counter }
    /// Prepaid authenticated-block charge of that same reservation.
    #[must_use]
    pub const fn authenticated_blocks(&self) -> u64 { self.intent.blocks }
    /// Complete canonical object metadata supplied by the C07 owner, verbatim.
    #[must_use]
    pub fn object_metadata_canonical(&self) -> &[u8] { &self.intent.object_metadata }
    /// Whether this intent still describes exactly this reservation row.
    ///
    /// A false result is an unresolved contradiction between two immutable rows;
    /// this technical owner neither repairs nor deletes either claim.
    #[must_use]
    pub fn binds(&self, reservation: &SealReservationReadback) -> bool {
        self.intent.operation_id == reservation.operation_id
            && self.intent.current_canonical == reservation.current_canonical
            && self.intent.content_digest == reservation.content_digest
            && self.intent.plaintext_length == reservation.plaintext_length
            && self.intent.counter == reservation.counter
            && self.intent.blocks == reservation.blocks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObjectIntent {
    operation_id: OpaqueId,
    current_canonical: Vec<u8>,
    content_digest: Blake3Digest32,
    plaintext_length: u64,
    counter: u64,
    blocks: u64,
    object_metadata: Vec<u8>,
}

impl ObjectIntent {
    fn encode(&self, limits: JournalLimits) -> Result<ControlValue, ControlError> {
        super::record(vec![super::text(OBJECT_INTENT_MARKER)?, V::U64(OBJECT_INTENT_VERSION),
            super::text(self.operation_id.as_str())?, super::bytes(&self.current_canonical)?,
            super::bytes(self.content_digest.as_bytes())?, V::U64(self.plaintext_length),
            V::U64(self.counter), V::U64(self.blocks), super::bytes(&self.object_metadata)?], limits)
    }

    fn decode(value: &ControlValue) -> Result<Self, ControlError> {
        if value.class() != ControlRecordClass::State || value.len() > MAX_OBJECT_INTENT_BYTES {
            return Err(ControlError::StoreCorrupt);
        }
        let Ok(V::Array(fields)) = c::parse_canonical_cbor(value.as_bytes()) else {
            return Err(ControlError::StoreCorrupt);
        };
        let [V::Text(marker), V::U64(2), V::Text(operation_id), V::Bytes(current), V::Bytes(digest),
            V::U64(plaintext_length), V::U64(counter), V::U64(blocks), V::Bytes(object_metadata)]
            = fields.as_slice() else { return Err(ControlError::StoreCorrupt); };
        if marker.as_str() != OBJECT_INTENT_MARKER {
            return Err(ControlError::StoreCorrupt);
        }
        super::canonical(current.as_slice(), super::MAX_OWNER_RECORD)?;
        super::canonical(object_metadata.as_slice(), MAX_OBJECT_METADATA_BYTES)?;
        let intent = Self {
            operation_id: OpaqueId::new(operation_id.as_str()).map_err(|_| ControlError::StoreCorrupt)?,
            current_canonical: current.as_slice().to_vec(),
            content_digest: digest_bytes(digest.as_slice())?,
            plaintext_length: *plaintext_length, counter: *counter, blocks: *blocks,
            object_metadata: object_metadata.as_slice().to_vec(),
        };
        if intent.plaintext_length > super::MAX_REVISION_USE_PLAINTEXT_BYTES
            || intent.counter >= super::MAX_REVISION_USE_ATTEMPTS
            || intent.blocks == 0 || intent.blocks > super::MAX_REVISION_USE_BLOCKS
            || intent.blocks < intent.plaintext_length.div_ceil(16).saturating_add(1)
        {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(intent)
    }
}

impl RevisionUseCommand {
    /// Binds the derived physical object identity of one burned reservation.
    ///
    /// This is the only late write of the reservation profile. It runs after AES
    /// has produced the ciphertext-derived identity and before any filesystem
    /// publication, and it asserts the exact committed reservation value and the
    /// exact selected Current record atomically. The new intent row is required
    /// to be absent, so a second binding refuses and an already committed binding
    /// replays only through its retained original command.
    ///
    /// The returned command retains no seal use. Committing it therefore never
    /// yields [`CommittedRevisionSealUse`](super::CommittedRevisionSealUse) and can
    /// never authorize AES. Callers keep the original reservation operation and
    /// the exact command so recovery resolves protection through the completed
    /// operation instead of re-encrypting.
    ///
    /// # Errors
    /// A foreign preparation, an incomplete read scope, a reservation/Current
    /// contradiction, an already present intent, noncanonical or oversized object
    /// metadata, and an operation identity equal to the retention operation all
    /// refuse before any effect.
    pub fn bind_object_publication(read: &ScopedControlRecordsReadback, retention_operation: MutationId,
        operation: MutationId, digest: Blake3Digest32, preparation: &[u8], object_metadata: &[u8],
        limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        if operation == retention_operation { return Err(ControlError::OperationConflict); }
        super::canonical(object_metadata, MAX_OBJECT_METADATA_BYTES)?;
        let keys = object_publication_intent_keys(retention_operation, limits)?;
        let (state, current) = super::bound_state(read, preparation, limits)?;
        let stored = super::present(read, &keys[3])?;
        let reservation = SealReservationReadback::decode(stored)?;
        if reservation.current_canonical != current.as_bytes()
            || state.attempts <= reservation.counter
            || state.residency.as_deref() != Some(reservation.residency_canonical.as_slice())
        {
            return Err(ControlError::OperationConflict);
        }
        let value = ObjectIntent { operation_id: reservation.operation_id.clone(),
            current_canonical: reservation.current_canonical.clone(),
            content_digest: reservation.content_digest, plaintext_length: reservation.plaintext_length,
            counter: reservation.counter, blocks: reservation.blocks,
            object_metadata: object_metadata.to_vec() }.encode(limits)?;
        if value.len() > MAX_OBJECT_INTENT_BYTES { return Err(ControlError::BudgetExceeded); }
        let conditions = vec![ControlRecordCondition::exact(keys[3].clone(), stored.clone()),
            ControlRecordCondition::exact(keys[2].clone(), current.clone()),
            super::absent(read, &keys[4])?];
        Self::build(read, operation, digest, vec![ControlWrite { key: keys[4].clone(), value }],
            conditions, None)
    }
}

fn digest_bytes(bytes: &[u8]) -> Result<Blake3Digest32, ControlError> {
    let stored: [u8; 32] = bytes.try_into().map_err(|_| ControlError::StoreCorrupt)?;
    Ok(Blake3Digest32::from_stored_bytes(stored))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_retention::{revision_use_keys, RevisionSealMetadata, RevisionUseCommand};
    use crate::{BoundControlJournal, ControlMutation, ControlSnapshotPublisher, JournalIdentity,
        JournalProfileInitialization, MigrationBootstrapWork, PersistentControlJournal,
        RevisionDekCommand, RevisionDekSelectionCommand, REVISION_DEK_APPLICATION_INTENT_KEY,
        REVISION_DEK_CANDIDATE_KEY, REVISION_DEK_CURRENT_KEY, REVISION_DEK_INTENT_KEY};
    use search_contracts::{to_canonical_cbor, BoundedBytes, BoundedList, CanonicalText, DataRootId,
        InstallationIncarnationId, OpaqueRef, OwnerEpoch, RequestId};
    use search_ports::{CancellationProbe, MutationIdentity, OperationContext, PackageOpaque};
    use std::fs::{self, OpenOptions};
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const LIMITS: JournalLimits = JournalLimits::BASELINE;
    const PREPARATION_SEED: u64 = 1;
    const RESERVATION_OPERATION: MutationId = MutationId([8; 32]);

    #[derive(Debug)]
    struct Cancel;
    impl PackageOpaque for Cancel {
        fn owner_package(&self) -> &'static str { "search-control-redb" }
    }
    impl CancellationProbe for Cancel {
        fn is_cancelled(&self) -> bool { false }
    }

    fn context() -> OperationContext<Cancel> {
        OperationContext::new(RequestId::from_bytes([7; 16]), 60_000, Cancel,
            OpaqueRef::new("budget:object-publication-fixture").unwrap()).unwrap()
    }
    fn deadline() -> Instant { Instant::now() + Duration::from_secs(60) }
    fn digest32(byte: u8) -> Blake3Digest32 { Blake3Digest32::from_bytes([byte; 32]) }
    fn cbor(value: &V) -> Vec<u8> { to_canonical_cbor(value).unwrap().as_slice().to_vec() }
    fn id(byte: u8) -> MutationIdentity { crate::control_mutation_identity(MutationId([byte; 32])).unwrap() }

    struct Scratch { root: PathBuf }
    impl Scratch {
        fn new() -> Self {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let root = std::env::temp_dir()
                .join(format!("eliot-object-intent-{}-{stamp}", std::process::id()));
            fs::create_dir(&root).unwrap();
            Self { root }
        }
        fn path(&self) -> PathBuf { self.root.join("control.redb") }
    }
    impl Drop for Scratch {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
    }

    fn base_identity() -> JournalIdentity {
        JournalIdentity {
            installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
            data_root_id: DataRootId::from_bytes([2; 16]),
            owner_epoch: OwnerEpoch::new(1).unwrap(),
            path_identity_digest: digest32(3),
            schema_family_digest: digest32(4),
            schema_version: crate::REGISTRY_JOURNAL_SCHEMA_VERSION,
        }
    }

    /// Schema-5 Direct journal on an exclusively created file; no indexed route.
    fn create5(scratch: &Scratch) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).create_new(true)
            .open(scratch.path()).unwrap();
        let mut journal = PersistentControlJournal::create(file, base_identity(), LIMITS).unwrap();
        journal.initialize_operating_profile(JournalProfileInitialization::Direct,
            MutationId([90; 32]), digest32(91), &context()).unwrap();
        journal
    }

    fn dek_value(kind: u64) -> ControlValue {
        let value = V::Array(BoundedList::new(vec![
            V::Text(CanonicalText::new("eliot.revision-dek.record.v1").unwrap()),
            V::U64(1), V::U64(kind), V::Null, V::Null, V::Null, V::U64(0)]).unwrap());
        ControlValue::new(ControlRecordClass::State, cbor(&value), LIMITS).unwrap()
    }

    fn dek_command(byte: u8, generation: u64, candidate: bool) -> RevisionDekCommand {
        let conditions = vec![
            if candidate { ControlRecordCondition::exact(super::super::key(REVISION_DEK_INTENT_KEY, LIMITS).unwrap(), dek_value(0)) }
            else { ControlRecordCondition::absent(super::super::key(REVISION_DEK_INTENT_KEY, LIMITS).unwrap()) },
            ControlRecordCondition::absent(super::super::key(REVISION_DEK_CANDIDATE_KEY, LIMITS).unwrap()),
            ControlRecordCondition::absent(super::super::key(REVISION_DEK_CURRENT_KEY, LIMITS).unwrap())];
        RevisionDekCommand::new(crate::ConditionalControlMutation::new(ControlMutation::new(
            MutationId([byte; 32]), digest32(9), generation, vec![ControlWrite {
                key: super::super::key(if candidate { REVISION_DEK_CANDIDATE_KEY } else { REVISION_DEK_INTENT_KEY }, LIMITS).unwrap(),
                value: dek_value(u64::from(candidate)) }], vec![]), conditions)).unwrap()
    }

    fn selection_value(current: bool) -> ControlValue {
        let intent = V::Array(BoundedList::new(vec![V::Null; 8]).unwrap());
        let body = if current {
            V::Array(BoundedList::new(vec![intent, V::U64(1), V::U64(1), V::Null]).unwrap())
        } else { intent };
        let value = V::Array(BoundedList::new(vec![
            V::Text(CanonicalText::new("eliot.revision-dek.selection.v1").unwrap()),
            V::U64(1), V::U64(u64::from(current)), body]).unwrap());
        ControlValue::new(ControlRecordClass::State, cbor(&value), LIMITS).unwrap()
    }

    fn selection_command(byte: u8, generation: u64, current: bool) -> RevisionDekSelectionCommand {
        RevisionDekSelectionCommand::new(crate::ConditionalControlMutation::new(ControlMutation::new(
            MutationId([byte; 32]), digest32(9), generation, vec![ControlWrite {
                key: super::super::key(if current { REVISION_DEK_CURRENT_KEY } else { REVISION_DEK_APPLICATION_INTENT_KEY }, LIMITS).unwrap(),
                value: selection_value(current) }], vec![]), vec![
            ControlRecordCondition::exact(super::super::key(REVISION_DEK_INTENT_KEY, LIMITS).unwrap(), dek_value(0)),
            ControlRecordCondition::exact(super::super::key(REVISION_DEK_CANDIDATE_KEY, LIMITS).unwrap(), dek_value(1)),
            if current {
                ControlRecordCondition::exact(super::super::key(REVISION_DEK_APPLICATION_INTENT_KEY, LIMITS).unwrap(), selection_value(false))
            } else { ControlRecordCondition::absent(super::super::key(REVISION_DEK_APPLICATION_INTENT_KEY, LIMITS).unwrap()) },
            ControlRecordCondition::absent(super::super::key(REVISION_DEK_CURRENT_KEY, LIMITS).unwrap())])).unwrap()
    }

    fn seal_metadata(residency: &[u8]) -> RevisionSealMetadata {
        RevisionSealMetadata {
            operation_id: OpaqueId::new("retention-operation:disk-fixture").unwrap(),
            preseal_occurrence_canonical: cbor(&V::U64(7)),
            residency_canonical: residency.to_vec(),
            aad_canonical: cbor(&V::U64(11)),
            plaintext_length: 32,
            content_digest: digest32(42),
        }
    }

    /// Genesis, Core intent binding, candidate, application intent and Current.
    fn materialize_bound_state(bound: &mut BoundControlJournal<'_, Cancel>, preparation: &[u8]) {
        let work = |_unit: MigrationBootstrapWork| -> Result<(), &'static str> { Ok(()) };
        let keys = revision_use_keys(LIMITS).unwrap();
        let read = bound.read_control_keys_with_original_context(&keys, deadline(), &work).unwrap();
        let genesis = RevisionUseCommand::genesis(&read, MutationId([1; 32]), digest32(1), preparation, LIMITS).unwrap();
        let (_, grant) = bound.transact_revision_use_with_original_context(&genesis, &id(1), deadline(), &work).unwrap();
        assert!(grant.is_none(), "genesis is not a use permit");
        let intent = dek_command(2, 2, false);
        bound.transact_revision_dek_with_original_context(&intent, &id(2), deadline(), &work).unwrap();
        let read = bound.read_control_keys_with_original_context(&keys, deadline(), &work).unwrap();
        let binding = RevisionUseCommand::bind_material(&read, MutationId([3; 32]), digest32(3), preparation, LIMITS).unwrap();
        let (_, grant) = bound.transact_revision_use_with_original_context(&binding, &id(3), deadline(), &work).unwrap();
        assert!(grant.is_none(), "material binding is not a use permit");
        let candidate = dek_command(4, 4, true);
        bound.transact_revision_dek_with_original_context(&candidate, &id(4), deadline(), &work).unwrap();
        let application = selection_command(5, 5, false);
        bound.transact_revision_dek_selection_with_original_context(&application, &id(5), deadline(), &work).unwrap();
        let current = selection_command(6, 6, true);
        bound.transact_revision_dek_selection_with_original_context(&current, &id(6), deadline(), &work).unwrap();
    }

    /// The exact field shape the frozen reservation encoder writes.
    fn reservation_bytes(marker: &str, operation: &str, digest: &[u8], plaintext_length: u64,
        counter: u64, blocks: u64,
    ) -> Vec<u8> {
        let field = |bytes: Vec<u8>| V::Bytes(BoundedBytes::new(bytes).unwrap());
        cbor(&V::Array(BoundedList::new(vec![
            V::Text(CanonicalText::new(marker).unwrap()),
            V::U64(2), field(cbor(&V::U64(5))), V::Text(CanonicalText::new(operation).unwrap()),
            field(cbor(&V::U64(7))), field(cbor(&V::U64(3))), field(cbor(&V::U64(11))),
            field(digest.to_vec()), V::U64(plaintext_length), V::U64(counter), V::U64(blocks)]).unwrap()))
    }

    #[test]
    fn readback_codecs_accept_the_frozen_shape_and_refuse_foreign_rows() {
        let digest = [0xA5_u8; 32];
        let row = ControlValue::new(ControlRecordClass::State,
            reservation_bytes(super::super::RESERVATION_MARKER, "retention-operation:pure",
                &digest, 32, 0, 4), LIMITS).unwrap();
        let decoded = SealReservationReadback::decode(&row).unwrap();
        assert_eq!(decoded.operation_id().as_str(), "retention-operation:pure");
        assert_eq!(decoded.plaintext_length(), 32);
        assert_eq!(decoded.counter(), 0);
        assert_eq!(decoded.authenticated_blocks(), 4);
        assert_eq!(decoded.content_digest(), Blake3Digest32::from_bytes(digest));
        assert_eq!(decoded.preseal_occurrence_canonical(), cbor(&V::U64(7)).as_slice());
        // A foreign marker, a widened counter and a block charge below the body
        // length are stored-state contradictions, never adopted metadata.
        for (bytes, label) in [
            (reservation_bytes(super::super::STATE_MARKER, "retention-operation:pure",
                &digest, 32, 0, 4), "foreign marker"),
            (reservation_bytes(super::super::RESERVATION_MARKER, "retention-operation:pure",
                &digest, 32, super::super::MAX_REVISION_USE_ATTEMPTS, 4), "counter at the cap"),
            (reservation_bytes(super::super::RESERVATION_MARKER, "retention-operation:pure",
                &digest, 32, 0, 1), "undercharged blocks"),
        ] {
            let foreign = ControlValue::new(ControlRecordClass::State, bytes, LIMITS).unwrap();
            assert_eq!(SealReservationReadback::decode(&foreign), Err(ControlError::StoreCorrupt), "{label}");
        }
        // The intent codec round-trips exactly and refuses the same contradictions.
        let intent = ObjectIntent { operation_id: OpaqueId::new("retention-operation:pure").unwrap(),
            current_canonical: cbor(&V::U64(5)), content_digest: Blake3Digest32::from_bytes(digest),
            plaintext_length: 32, counter: 0, blocks: 4, object_metadata: cbor(&V::U64(9)) };
        let encoded = intent.encode(LIMITS).unwrap();
        assert_eq!(ObjectIntent::decode(&encoded), Ok(intent.clone()));
        let saturated = ObjectIntent { blocks: super::super::MAX_REVISION_USE_BLOCKS + 1, ..intent };
        assert_eq!(ObjectIntent::decode(&saturated.encode(LIMITS).unwrap()), Err(ControlError::StoreCorrupt));
        // Oversized object metadata is refused before any encoding work.
        let oversized = cbor(&V::Bytes(BoundedBytes::new(vec![0_u8; MAX_OBJECT_METADATA_BYTES]).unwrap()));
        assert!(oversized.len() > MAX_OBJECT_METADATA_BYTES);
        assert_eq!(super::super::canonical(&oversized, MAX_OBJECT_METADATA_BYTES), Err(ControlError::BudgetExceeded));
    }

    #[test]
    fn object_binding_burns_no_permit_and_replays_only_through_the_retained_command() {
        let scratch = Scratch::new();
        let mut journal = create5(&scratch);
        let mut publisher = ControlSnapshotPublisher::new();
        let preparation = cbor(&V::U64(PREPARATION_SEED));
        let residency = cbor(&V::U64(3));
        let metadata = cbor(&V::U64(77));
        let intent_key = object_publication_intent_key(RESERVATION_OPERATION, LIMITS).unwrap();
        let work = |_unit: MigrationBootstrapWork| -> Result<(), &'static str> { Ok(()) };
        {
            let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
            materialize_bound_state(&mut bound, &preparation);
            let mut scope = revision_use_keys(LIMITS).unwrap().to_vec();
            scope.push(crate::source_retention::revision_seal_reservation_key(RESERVATION_OPERATION, LIMITS).unwrap());
            let read = bound.read_control_keys_with_original_context(&scope, deadline(), &work).unwrap();
            // An unbound key is never absence: the narrow genesis/material read
            // cannot even report the residency claim of this exact key.
            let usage = RevisionUseUsageReadback::read(&read, &preparation, LIMITS).unwrap();
            assert_eq!(usage.residency_canonical(), None);
            assert_eq!(usage.attempts(), 0);
            let use_command = RevisionUseCommand::reserve(&read, RESERVATION_OPERATION, digest32(8),
                &preparation, 0, seal_metadata(&residency), LIMITS).unwrap();
            let (receipt, permit) = bound.transact_revision_use_with_original_context(
                &use_command, &id(8), deadline(), &work).unwrap();
            assert!(!receipt.replayed && receipt.after_generation == 8);
            assert_eq!(permit.expect("the reservation itself still mints the one permit").counter(), 0);

            // The late binding needs the reservation and the new key in one scope.
            scope.push(intent_key.clone());
            let read = bound.read_control_keys_with_original_context(&scope, deadline(), &work).unwrap();
            assert_eq!(RevisionUseUsageReadback::read(&read, &preparation, LIMITS).unwrap()
                .residency_canonical(), Some(residency.as_slice()));
            assert_eq!(RevisionUseCommand::bind_object_publication(&read, RESERVATION_OPERATION,
                RESERVATION_OPERATION, digest32(12), &preparation, &metadata, LIMITS),
                Err(ControlError::OperationConflict));
            let bind = RevisionUseCommand::bind_object_publication(&read, RESERVATION_OPERATION,
                MutationId([12; 32]), digest32(12), &preparation, &metadata, LIMITS).unwrap();
            assert_eq!(bind.command().mutation().expected_generation(), 8);
            let (receipt, permit) = bound.transact_revision_use_with_original_context(
                &bind, &id(12), deadline(), &work).unwrap();
            assert_eq!(receipt.after_generation, 9);
            assert!(permit.is_none(), "a late object binding is never a crypto permit");

            let read = bound.read_control_keys_with_original_context(&scope, deadline(), &work).unwrap();
            let reservation = SealReservationReadback::read(&read, RESERVATION_OPERATION, LIMITS).unwrap();
            assert_eq!(reservation.counter(), 0);
            assert_eq!(reservation.authenticated_blocks(), 4);
            assert_eq!(reservation.content_digest(), digest32(42));
            assert_eq!(reservation.aad_canonical(), cbor(&V::U64(11)).as_slice());
            let intent = ObjectPublicationIntentReadback::read(&read, RESERVATION_OPERATION, LIMITS).unwrap();
            assert_eq!(intent.operation_id().as_str(), "retention-operation:disk-fixture");
            assert_eq!(intent.object_metadata_canonical(), metadata.as_slice());
            assert_eq!(intent.selected_current_canonical(), reservation.selected_current_canonical());
            assert!(intent.binds(&reservation));
            // The burned counter is permanent and the intent is immutable.
            assert_eq!(RevisionUseUsageReadback::read(&read, &preparation, LIMITS).unwrap().attempts(), 1);
            let (replay, permit) = bound.transact_revision_use_with_original_context(
                &bind, &id(12), deadline(), &work).unwrap();
            assert!(replay.replayed && replay.after_generation == 9);
            assert!(permit.is_none());
            // A present intent refuses reconstruction; only the retained command replays.
            assert_eq!(RevisionUseCommand::bind_object_publication(&read, RESERVATION_OPERATION,
                MutationId([12; 32]), digest32(12), &preparation, &metadata, LIMITS),
                Err(ControlError::OperationConflict));
        }
        // Generic writers may neither write nor delete the reserved intent key.
        let before = journal.verify().unwrap();
        for deleting in [false, true] {
            let command = crate::ConditionalControlMutation::new(ControlMutation::new(MutationId([11; 32]),
                digest32(11), 9,
                if deleting { vec![] } else { vec![ControlWrite { key: intent_key.clone(),
                    value: ControlValue::new(ControlRecordClass::State, b"opaque".to_vec(), LIMITS).unwrap() }] },
                if deleting { vec![intent_key.clone()] } else { vec![] }), vec![]);
            assert_eq!(journal.transact_conditionally(command, &context()).unwrap_err().control_error(),
                ControlError::InvalidKey);
        }
        assert_eq!(journal.verify().unwrap(), before);
    }
}

