//! Version-one C07 derived-artifact use and late publication metadata.
//!
//! This profile stores bounded technical records beside the existing selected-v2
//! usage state. It keeps C07 canonical descriptor, AAD, object-reference and
//! address bytes opaque: this module compares and retains them, but does not
//! decode C07 semantics or turn a digest into owner authentication.

use core::fmt;
use std::collections::BTreeSet;

use search_contracts::{
    self as c, Blake3Digest32, CanonicalValue as V, MaterializationId, OpaqueId, RepresentationId,
    SourceId, SourceNamespaceId, SourceRevisionId, SourceRevisionRef,
};

use crate::{
    ConditionalControlMutation, ControlError, ControlKey, ControlMutation, ControlRecordClass,
    ControlRecordCondition, ControlValue, ControlWrite, JournalLimits, MutationId,
    ScopedControlRecordsReadback,
};

/// Maximum C07 canonical descriptor bytes retained by this profile.
pub const MAX_DERIVED_ARTIFACT_DESCRIPTOR_BYTES: usize = 16 * 1024;
/// Maximum exact C07 canonical object-reference bytes retained by late intent.
pub const MAX_DERIVED_ARTIFACT_REFERENCE_BYTES: usize = MAX_DERIVED_ARTIFACT_DESCRIPTOR_BYTES + 512;
/// Maximum opaque C07 address bytes retained by late intent.
pub const MAX_DERIVED_ARTIFACT_ADDRESS_BYTES: usize = 4 * 1024;

const SUBJECT_PREFIX: &[u8] = b"secret/revision-use/v2/derived-artifact/v1/subject/";
const RESERVATION_PREFIX: &[u8] = b"secret/revision-use/v2/derived-artifact/v1/reservation/";
const GUARD_PREFIX: &[u8] = b"secret/revision-use/v2/derived-artifact/v1/guards/";
const OBJECT_INTENT_PREFIX: &[u8] = b"secret/revision-use/v2/derived-artifact/v1/object-intent/";
const SUBJECT_MARKER: &str = "eliot.derived-artifact.subject.v1";
const RESERVATION_MARKER: &str = "eliot.derived-artifact.reservation.v1";
const GUARD_MARKER: &str = "eliot.derived-artifact.guards.v1";
const OBJECT_INTENT_MARKER: &str = "eliot.derived-artifact.object-intent.v1";
const MAX_SUBJECT_RECORD_BYTES: usize = 40 * 1024;
const MAX_RESERVATION_RECORD_BYTES: usize = 48 * 1024;
const MAX_GUARD_RECORD_BYTES: usize = 62 * 1024;
const MAX_OBJECT_INTENT_RECORD_BYTES: usize = 24 * 1024;
const HEX: &[u8; 16] = b"0123456789abcdef";

/// The five exact Root/source/namespace/policy/purge keys read with one scope.
///
/// These are caller-supplied record addresses, not key material or issued
/// authority. Their exact values are captured from one scoped read and are
/// required again at late object-intent publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedArtifactScopeGuardKeysV1 {
    root_owner: ControlKey,
    source_revision: ControlKey,
    namespace_owner: ControlKey,
    residency_policy: ControlKey,
    purge_fence: ControlKey,
}

impl DerivedArtifactScopeGuardKeysV1 {
    /// Binds the existing owner-provided keys in the required semantic order.
    ///
    /// # Errors
    /// Duplicate keys cannot represent the five independent readback guards.
    pub fn new(
        root_owner: ControlKey,
        source_revision: ControlKey,
        namespace_owner: ControlKey,
        residency_policy: ControlKey,
        purge_fence: ControlKey,
    ) -> Result<Self, ControlError> {
        let keys = [
            &root_owner,
            &source_revision,
            &namespace_owner,
            &residency_policy,
            &purge_fence,
        ];
        let mut unique = BTreeSet::new();
        if keys.iter().any(|key| !unique.insert(key.as_bytes())) {
            return Err(ControlError::DuplicateMutationKey);
        }
        Ok(Self {
            root_owner,
            source_revision,
            namespace_owner,
            residency_policy,
            purge_fence,
        })
    }

    /// Actual Root-owner key supplied by the caller.
    #[must_use]
    pub const fn root_owner(&self) -> &ControlKey {
        &self.root_owner
    }
    /// Actual source-revision key supplied by the caller.
    #[must_use]
    pub const fn source_revision(&self) -> &ControlKey {
        &self.source_revision
    }
    /// Actual namespace-owner key supplied by the caller.
    #[must_use]
    pub const fn namespace_owner(&self) -> &ControlKey {
        &self.namespace_owner
    }
    /// Actual residency-policy key supplied by the caller.
    #[must_use]
    pub const fn residency_policy(&self) -> &ControlKey {
        &self.residency_policy
    }
    /// Actual purge-fence key supplied by the caller.
    #[must_use]
    pub const fn purge_fence(&self) -> &ControlKey {
        &self.purge_fence
    }

    const fn ordered(&self) -> [&ControlKey; 5] {
        [
            &self.root_owner,
            &self.source_revision,
            &self.namespace_owner,
            &self.residency_policy,
            &self.purge_fence,
        ]
    }
}

/// Exact C07 subject and sealing bytes prepared before selected-key use.
///
/// `descriptor_canonical`, `aad_canonical`, `residency_canonical`, and
/// `selected_current_canonical` are retained verbatim. The supplied source and
/// Root IDs remain equality metadata; Root/C07 must join them to actual owner
/// state before use. `expected_counter` is conditional preparation only.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactUseMetadataV1 {
    /// Original Root operation carried by the C07 descriptor.
    pub operation_id: OpaqueId,
    /// Exact original source revision; this profile appends no source occurrence.
    pub source_revision: SourceRevisionRef,
    /// Genuine Root-supplied materialization ID as data for parent verification.
    pub materialization_id: MaterializationId,
    /// Genuine Root-supplied representation ID as data for parent verification.
    pub representation_id: RepresentationId,
    /// Complete canonical C07 `DerivedArtifactDescriptorV1::encode()` bytes.
    pub descriptor_canonical: Vec<u8>,
    /// Complete existing six-domain residency encoding, supplied by its owner.
    pub residency_canonical: Vec<u8>,
    /// Exact selected Current record included in the C07 AAD.
    pub selected_current_canonical: Vec<u8>,
    /// Complete canonical C07 `DerivedArtifactSealBindingV1::aad_bytes()` bytes.
    pub aad_canonical: Vec<u8>,
    /// Exact artifact plaintext length P charged by the existing selected-key profile.
    pub plaintext_length: u64,
    /// Counter read from the shared selected-v2 usage state before AAD construction.
    pub expected_counter: u64,
}

impl fmt::Debug for DerivedArtifactUseMetadataV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DerivedArtifactUseMetadataV1")
            .field("descriptor_bytes", &self.descriptor_canonical.len())
            .field("aad_bytes", &self.aad_canonical.len())
            .field("plaintext_length", &self.plaintext_length)
            .field("expected_counter", &self.expected_counter)
            .finish_non_exhaustive()
    }
}

/// C07's exact post-AES object observation for a late publication intent.
///
/// `object_reference_canonical` and `address_bytes` are opaque C07 outputs.
/// `stored_object_digest` and `stored_object_length` describe those stored
/// object bytes; they are never source-body digest/length fields.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactObjectPublicationV1 {
    /// Exact C07 `DerivedArtifactObjectRefV1::encode()` bytes.
    pub object_reference_canonical: Vec<u8>,
    /// Exact C07 relative address bytes, such as `DerivedArtifactAddressV1::relative_path()`.
    pub address_bytes: Vec<u8>,
    /// C07 digest of the actual stored object bytes, including the envelope when sealed.
    pub stored_object_digest: Blake3Digest32,
    /// Exact length of the actual stored object bytes.
    pub stored_object_length: u64,
}

impl fmt::Debug for DerivedArtifactObjectPublicationV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DerivedArtifactObjectPublicationV1")
            .field("reference_bytes", &self.object_reference_canonical.len())
            .field("address_bytes", &self.address_bytes.len())
            .field("stored_object_length", &self.stored_object_length)
            .finish_non_exhaustive()
    }
}

/// The C02-owned non-Clone permit returned only by a fresh acknowledged commit.
///
/// Dropping it does not refund usage. Replay, recovery and unknown outcomes never
/// construct this type. The permit carries exact metadata only; the Windows/Crypto
/// owner still validates key selection, nonce policy, AAD parity and AES use.
pub struct CommittedDerivedArtifactSealUse {
    metadata: DerivedArtifactUseMetadataV1,
    preparation: Vec<u8>,
    authenticated_blocks: u64,
}

impl fmt::Debug for CommittedDerivedArtifactSealUse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CommittedDerivedArtifactSealUse([REDACTED])")
    }
}

impl CommittedDerivedArtifactSealUse {
    /// Exact C07 subject, AAD, current, residency and pre-seal coordinates.
    #[must_use]
    pub const fn metadata(&self) -> &DerivedArtifactUseMetadataV1 {
        &self.metadata
    }
    /// Exact selected Current record guarded by the reservation transaction.
    #[must_use]
    pub fn selected_current_canonical(&self) -> &[u8] {
        &self.metadata.selected_current_canonical
    }
    /// Complete canonical C07 AAD bytes guarded by this fresh reservation.
    #[must_use]
    pub fn aad_canonical(&self) -> &[u8] {
        &self.metadata.aad_canonical
    }
    /// Permanently consumed counter; zero is a valid first use.
    #[must_use]
    pub const fn counter(&self) -> u64 {
        self.metadata.expected_counter
    }
    /// Exact artifact plaintext length P charged before AES.
    #[must_use]
    pub const fn plaintext_length(&self) -> u64 {
        self.metadata.plaintext_length
    }
    /// Exact prepaid `ceil(P/16) + ceil(AAD/16) + 1` charge.
    #[must_use]
    pub const fn authenticated_blocks(&self) -> u64 {
        self.authenticated_blocks
    }
    /// Original Root source revision; never the derived object's body identity.
    #[must_use]
    pub const fn source_revision_ref(&self) -> &SourceRevisionRef {
        &self.metadata.source_revision
    }
    /// Root-supplied materialization identity, retained for parent verification.
    #[must_use]
    pub const fn materialization_id(&self) -> MaterializationId {
        self.metadata.materialization_id
    }
    /// Root-supplied representation identity, retained for parent verification.
    #[must_use]
    pub const fn representation_id(&self) -> RepresentationId {
        self.metadata.representation_id
    }
    /// Original canonical C07 descriptor bytes, stored and compared as opaque data.
    #[must_use]
    pub fn descriptor_canonical(&self) -> &[u8] {
        &self.metadata.descriptor_canonical
    }
    /// Exact complete residency claim shared with selected-v2 usage state.
    #[must_use]
    pub fn residency_canonical(&self) -> &[u8] {
        &self.metadata.residency_canonical
    }
    /// Exact original C02 preparation profile shared with selected-v2 usage state.
    #[must_use]
    pub fn preparation_canonical(&self) -> &[u8] {
        &self.preparation
    }
}

/// Exact v1 reservation readback; it cannot mint or reconstruct a use permit.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactReservationReadback {
    mutation_id: MutationId,
    subject: DerivedSubject,
    reservation: DerivedReservation,
    guards: DerivedGuardSnapshot,
}

impl fmt::Debug for DerivedArtifactReservationReadback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DerivedArtifactReservationReadback")
            .field("counter", &self.reservation.counter)
            .field("descriptor_bytes", &self.subject.descriptor_canonical.len())
            .finish_non_exhaustive()
    }
}

impl DerivedArtifactReservationReadback {
    /// Reads the complete subject, reservation and guard rows from one scoped snapshot.
    ///
    /// # Errors
    /// Incomplete scope, malformed rows or disagreement with shared usage state refuse.
    pub fn read(
        read: &ScopedControlRecordsReadback,
        operation: MutationId,
        limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        if read.identity().schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        let subject_key = derived_artifact_subject_key(operation, limits)?;
        let reservation_key = derived_artifact_reservation_key(operation, limits)?;
        let guard_key = derived_artifact_guard_snapshot_key(operation, limits)?;
        let subject =
            DerivedSubject::decode(read.get(&subject_key).ok_or(ControlError::InvalidValue)?)?;
        let reservation = DerivedReservation::decode(
            read.get(&reservation_key)
                .ok_or(ControlError::InvalidValue)?,
        )?;
        let guards = DerivedGuardSnapshot::decode(
            read.get(&guard_key).ok_or(ControlError::InvalidValue)?,
            limits,
        )?;
        let usage = super::RevisionUseUsageReadback::read(read, &subject.preparation, limits)?;
        if usage.attempts() <= reservation.counter
            || usage.residency_canonical() != Some(subject.residency_canonical.as_slice())
        {
            return Err(ControlError::OperationConflict);
        }
        Ok(Self {
            mutation_id: operation,
            subject,
            reservation,
            guards,
        })
    }

    /// Mutation ID that addresses the immutable subject and reservation rows.
    #[must_use]
    pub const fn mutation_id(&self) -> MutationId {
        self.mutation_id
    }
    /// Original Root operation in the opaque C07 descriptor's parent fields.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.subject.operation_id
    }
    /// Exact original source revision; no source-history occurrence is added.
    #[must_use]
    pub const fn source_revision_ref(&self) -> &SourceRevisionRef {
        &self.subject.source_revision
    }
    /// Genuine Root-supplied materialization ID as opaque equality metadata.
    #[must_use]
    pub const fn materialization_id(&self) -> MaterializationId {
        self.subject.materialization_id
    }
    /// Genuine Root-supplied representation ID as opaque equality metadata.
    #[must_use]
    pub const fn representation_id(&self) -> RepresentationId {
        self.subject.representation_id
    }
    /// Complete canonical C07 descriptor bytes, retained verbatim.
    #[must_use]
    pub fn descriptor_canonical(&self) -> &[u8] {
        &self.subject.descriptor_canonical
    }
    /// Complete canonical six-domain residency bytes, retained verbatim.
    #[must_use]
    pub fn residency_canonical(&self) -> &[u8] {
        &self.subject.residency_canonical
    }
    /// Exact selected Current record protected by the reservation.
    #[must_use]
    pub fn selected_current_canonical(&self) -> &[u8] {
        &self.reservation.current_canonical
    }
    /// Complete canonical C07 AAD bytes, retained verbatim.
    #[must_use]
    pub fn aad_canonical(&self) -> &[u8] {
        &self.reservation.aad_canonical
    }
    /// Exact artifact plaintext length P.
    #[must_use]
    pub const fn plaintext_length(&self) -> u64 {
        self.reservation.plaintext_length
    }
    /// Permanently consumed shared selected-v2 usage counter.
    #[must_use]
    pub const fn counter(&self) -> u64 {
        self.reservation.counter
    }
    /// Exact charged authenticated-block count.
    #[must_use]
    pub const fn authenticated_blocks(&self) -> u64 {
        self.reservation.blocks
    }
    /// Exact preparation profile shared with selected-v2 usage state.
    #[must_use]
    pub fn preparation_canonical(&self) -> &[u8] {
        &self.subject.preparation
    }
    /// Five original guard keys in Root/source/namespace/policy/purge order.
    #[must_use]
    pub fn scope_guard_keys(&self) -> [&ControlKey; 5] {
        self.guards.keys()
    }
}

/// Readback of the immutable post-AES object-publication intent.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactObjectIntentReadback {
    mutation_id: MutationId,
    intent: DerivedObjectIntent,
}

impl fmt::Debug for DerivedArtifactObjectIntentReadback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DerivedArtifactObjectIntentReadback")
            .field(
                "reference_bytes",
                &self.intent.object_reference_canonical.len(),
            )
            .field("stored_object_length", &self.intent.stored_object_length)
            .finish_non_exhaustive()
    }
}

impl DerivedArtifactObjectIntentReadback {
    /// Reads one exact late-intent row; this is technical state, not a publish receipt.
    ///
    /// # Errors
    /// Missing, malformed or oversized rows refuse.
    pub fn read(
        read: &ScopedControlRecordsReadback,
        operation: MutationId,
        limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        let key = derived_artifact_object_intent_key(operation, limits)?;
        let intent =
            DerivedObjectIntent::decode(read.get(&key).ok_or(ControlError::InvalidValue)?)?;
        Ok(Self {
            mutation_id: operation,
            intent,
        })
    }

    /// Confirms this intent names the exact persisted reservation coordinates.
    /// C07 must additionally compare its reference descriptor/AAD and verify owner state.
    #[must_use]
    pub fn binds(&self, reservation: &DerivedArtifactReservationReadback) -> bool {
        self.mutation_id == reservation.mutation_id
            && self.intent.reservation_mutation_id == reservation.mutation_id
            && self.intent.operation_id == reservation.subject.operation_id
            && self.intent.counter == reservation.reservation.counter
            && self.intent.blocks == reservation.reservation.blocks
            && self.intent.plaintext_length == reservation.reservation.plaintext_length
    }

    /// Original Root operation carried by the paired reservation.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.intent.operation_id
    }
    /// Exact opaque canonical C07 object-reference bytes produced after AES.
    #[must_use]
    pub fn object_reference_canonical(&self) -> &[u8] {
        &self.intent.object_reference_canonical
    }
    /// Exact opaque C07 address bytes produced after AES.
    #[must_use]
    pub fn address_bytes(&self) -> &[u8] {
        &self.intent.address_bytes
    }
    /// C07's digest of actual stored bytes, never a source-body digest.
    #[must_use]
    pub const fn stored_object_digest(&self) -> Blake3Digest32 {
        self.intent.stored_object_digest
    }
    /// Exact actual stored-object length, including its envelope when sealed.
    #[must_use]
    pub const fn stored_object_length(&self) -> u64 {
        self.intent.stored_object_length
    }
    /// Counter of the paired immutable reservation.
    #[must_use]
    pub const fn counter(&self) -> u64 {
        self.intent.counter
    }
    /// Exact GHASH charge of the paired immutable reservation.
    #[must_use]
    pub const fn authenticated_blocks(&self) -> u64 {
        self.intent.blocks
    }
    /// Exact artifact plaintext length of the paired reservation.
    #[must_use]
    pub const fn plaintext_length(&self) -> u64 {
        self.intent.plaintext_length
    }
}

/// Immutable reservation command; holding or cloning it does not authorize AES.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactUseCommand {
    identity: crate::JournalIdentity,
    command: ConditionalControlMutation,
    metadata: DerivedArtifactUseMetadataV1,
    preparation: Vec<u8>,
    authenticated_blocks: u64,
}

impl fmt::Debug for DerivedArtifactUseCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DerivedArtifactUseCommand")
            .field("counter", &self.metadata.expected_counter)
            .field(
                "descriptor_bytes",
                &self.metadata.descriptor_canonical.len(),
            )
            .finish_non_exhaustive()
    }
}

impl DerivedArtifactUseCommand {
    /// Reserves one derived subject using the existing selected-v2 usage state.
    ///
    /// The one conditional transaction captures the exact Root/source/namespace/
    /// policy/purge values, claims the same six-domain residency, increments the
    /// existing attempts/blocks/plaintext counters, and writes versioned subject
    /// and reservation records. A stale `expected_counter` refuses; this method
    /// never edits an already persisted AAD.
    ///
    /// # Errors
    /// Missing scope rows, stale counter/current, foreign residency, noncanonical
    /// opaque bytes, or any existing operation row refuse before AES/filesystem effects.
    pub fn reserve(
        read: &ScopedControlRecordsReadback,
        operation: MutationId,
        command_digest: Blake3Digest32,
        preparation: &[u8],
        metadata: DerivedArtifactUseMetadataV1,
        guard_keys: &DerivedArtifactScopeGuardKeysV1,
        limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        if read.identity().schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        super::canonical(preparation, super::MAX_OWNER_RECORD)?;
        super::canonical(
            &metadata.descriptor_canonical,
            MAX_DERIVED_ARTIFACT_DESCRIPTOR_BYTES,
        )?;
        super::canonical(&metadata.residency_canonical, super::MAX_OWNER_RECORD)?;
        super::canonical(
            &metadata.selected_current_canonical,
            super::MAX_OWNER_RECORD,
        )?;
        super::canonical(&metadata.aad_canonical, super::MAX_AAD)?;

        let keys = super::revision_use_keys(limits)?;
        let (mut state, current) = super::bound_state(read, preparation, limits)?;
        if metadata.expected_counter != state.attempts
            || current.as_bytes() != metadata.selected_current_canonical
            || state
                .residency
                .as_ref()
                .is_some_and(|claim| claim != &metadata.residency_canonical)
        {
            return Err(ControlError::OperationConflict);
        }
        let blocks = authenticated_blocks(metadata.plaintext_length, metadata.aad_canonical.len())?;
        state.attempts = super::charge(state.attempts, 1, super::MAX_REVISION_USE_ATTEMPTS)?;
        state.blocks = super::charge(state.blocks, blocks, super::MAX_REVISION_USE_BLOCKS)?;
        state.plaintext_bytes = super::charge(
            state.plaintext_bytes,
            metadata.plaintext_length,
            super::MAX_REVISION_USE_PLAINTEXT_BYTES,
        )?;
        state.residency = Some(metadata.residency_canonical.clone());

        let subject = DerivedSubject {
            operation_id: metadata.operation_id.clone(),
            source_revision: metadata.source_revision,
            materialization_id: metadata.materialization_id,
            representation_id: metadata.representation_id,
            preparation: preparation.to_vec(),
            descriptor_canonical: metadata.descriptor_canonical.clone(),
            residency_canonical: metadata.residency_canonical.clone(),
        };
        let reservation = DerivedReservation {
            current_canonical: metadata.selected_current_canonical.clone(),
            aad_canonical: metadata.aad_canonical.clone(),
            plaintext_length: metadata.plaintext_length,
            counter: metadata.expected_counter,
            blocks,
        };
        let guards = DerivedGuardSnapshot::capture(read, guard_keys)?;
        let subject_key = derived_artifact_subject_key(operation, limits)?;
        let reservation_key = derived_artifact_reservation_key(operation, limits)?;
        let guard_key = derived_artifact_guard_snapshot_key(operation, limits)?;
        let subject_value = subject.encode(limits)?;
        let reservation_value = reservation.encode(limits)?;
        let guard_value = guards.encode(limits)?;

        let mut conditions = vec![
            ControlRecordCondition::exact(keys[0].clone(), super::present(read, &keys[0])?.clone()),
            ControlRecordCondition::exact(keys[1].clone(), super::present(read, &keys[1])?.clone()),
            ControlRecordCondition::exact(keys[3].clone(), current.clone()),
            super::absent(read, &subject_key)?,
            super::absent(read, &reservation_key)?,
            super::absent(read, &guard_key)?,
        ];
        conditions.extend(guards.conditions());
        ensure_unique_conditions(&conditions)?;

        let writes = vec![
            ControlWrite {
                key: keys[0].clone(),
                value: state.encode(limits)?,
            },
            ControlWrite {
                key: subject_key,
                value: subject_value,
            },
            ControlWrite {
                key: reservation_key,
                value: reservation_value,
            },
            ControlWrite {
                key: guard_key,
                value: guard_value,
            },
        ];
        let mutation =
            ControlMutation::new(operation, command_digest, read.generation(), writes, vec![]);
        Ok(Self {
            identity: read.identity(),
            command: ConditionalControlMutation::new(mutation, conditions),
            metadata,
            preparation: preparation.to_vec(),
            authenticated_blocks: blocks,
        })
    }

    /// Exact conditional command executed by the existing revision-use transaction route.
    #[must_use]
    pub const fn command(&self) -> &ConditionalControlMutation {
        &self.command
    }
    pub(crate) const fn identity(&self) -> crate::JournalIdentity {
        self.identity
    }
    pub(crate) fn permit_allocation(&self) -> usize {
        self.metadata.descriptor_canonical.len()
            + self.metadata.residency_canonical.len()
            + self.metadata.selected_current_canonical.len()
            + self.metadata.aad_canonical.len()
            + self.preparation.len()
            + self.metadata.operation_id.as_str().len()
            + std::mem::size_of::<CommittedDerivedArtifactSealUse>()
    }
    pub(crate) fn fresh_permit(&self) -> CommittedDerivedArtifactSealUse {
        CommittedDerivedArtifactSealUse {
            metadata: self.metadata.clone(),
            preparation: self.preparation.clone(),
            authenticated_blocks: self.authenticated_blocks,
        }
    }

    /// Builds the immutable post-AES intent that must commit before C07 filesystem publication.
    ///
    /// The new operation is distinct from the reservation operation. The exact
    /// reservation, subject, original five guards and selected Current are all
    /// conditional pre-state. This command carries no seal permit.
    ///
    /// # Errors
    /// Changed guards/current, incomplete scope, conflicting reservation or an
    /// existing intent refuses without changing either immutable reservation row.
    pub fn bind_object_publication(
        read: &ScopedControlRecordsReadback,
        reservation_operation: MutationId,
        operation: MutationId,
        command_digest: Blake3Digest32,
        object: DerivedArtifactObjectPublicationV1,
        guard_keys: &DerivedArtifactScopeGuardKeysV1,
        limits: JournalLimits,
    ) -> Result<DerivedArtifactObjectIntentCommand, ControlError> {
        if operation == reservation_operation {
            return Err(ControlError::OperationConflict);
        }
        super::canonical(
            &object.object_reference_canonical,
            MAX_DERIVED_ARTIFACT_REFERENCE_BYTES,
        )?;
        if object.address_bytes.is_empty()
            || object.address_bytes.len() > MAX_DERIVED_ARTIFACT_ADDRESS_BYTES
            || object.stored_object_length == 0
        {
            return Err(ControlError::BudgetExceeded);
        }

        let reservation =
            DerivedArtifactReservationReadback::read(read, reservation_operation, limits)?;
        if !reservation.guards.matches_keys(guard_keys) || !reservation.guards.matches_read(read) {
            return Err(ControlError::OperationConflict);
        }
        let keys = super::revision_use_keys(limits)?;
        let (state, current) = super::bound_state(read, &reservation.subject.preparation, limits)?;
        if current.as_bytes() != reservation.reservation.current_canonical
            || state.attempts <= reservation.reservation.counter
            || state.residency.as_deref()
                != Some(reservation.subject.residency_canonical.as_slice())
        {
            return Err(ControlError::OperationConflict);
        }

        let subject_key = derived_artifact_subject_key(reservation_operation, limits)?;
        let reservation_key = derived_artifact_reservation_key(reservation_operation, limits)?;
        let guard_key = derived_artifact_guard_snapshot_key(reservation_operation, limits)?;
        let intent_key = derived_artifact_object_intent_key(reservation_operation, limits)?;
        let subject_value = read
            .get(&subject_key)
            .ok_or(ControlError::InvalidValue)?
            .clone();
        let reservation_value = read
            .get(&reservation_key)
            .ok_or(ControlError::InvalidValue)?
            .clone();
        let guard_value = read
            .get(&guard_key)
            .ok_or(ControlError::InvalidValue)?
            .clone();
        let intent = DerivedObjectIntent {
            reservation_mutation_id: reservation_operation,
            operation_id: reservation.subject.operation_id.clone(),
            counter: reservation.reservation.counter,
            blocks: reservation.reservation.blocks,
            plaintext_length: reservation.reservation.plaintext_length,
            object_reference_canonical: object.object_reference_canonical,
            address_bytes: object.address_bytes,
            stored_object_digest: object.stored_object_digest,
            stored_object_length: object.stored_object_length,
        };
        let intent_value = intent.encode(limits)?;
        let mut conditions = vec![
            ControlRecordCondition::exact(subject_key, subject_value),
            ControlRecordCondition::exact(reservation_key, reservation_value),
            ControlRecordCondition::exact(guard_key, guard_value),
            ControlRecordCondition::exact(keys[3].clone(), current.clone()),
            super::absent(read, &intent_key)?,
        ];
        conditions.extend(reservation.guards.conditions());
        ensure_unique_conditions(&conditions)?;
        let mutation = ControlMutation::new(
            operation,
            command_digest,
            read.generation(),
            vec![ControlWrite {
                key: intent_key,
                value: intent_value,
            }],
            vec![],
        );
        Ok(DerivedArtifactObjectIntentCommand {
            identity: read.identity(),
            command: ConditionalControlMutation::new(mutation, conditions),
        })
    }
}

/// Immutable late object-intent command; it carries no cryptographic permit.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactObjectIntentCommand {
    identity: crate::JournalIdentity,
    command: ConditionalControlMutation,
}

impl fmt::Debug for DerivedArtifactObjectIntentCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DerivedArtifactObjectIntentCommand([REDACTED])")
    }
}

impl DerivedArtifactObjectIntentCommand {
    /// Exact conditional command executed before filesystem publication.
    #[must_use]
    pub const fn command(&self) -> &ConditionalControlMutation {
        &self.command
    }
    pub(crate) const fn identity(&self) -> crate::JournalIdentity {
        self.identity
    }
}

/// Full operation-addressed key for one versioned derived subject record.
/// These are technical journal addresses, not encryption keys.
///
/// # Errors
/// Refuses a key exceeding the active journal key limit.
pub fn derived_artifact_subject_key(
    operation: MutationId,
    limits: JournalLimits,
) -> Result<ControlKey, ControlError> {
    operation_key(SUBJECT_PREFIX, operation, limits)
}

/// Full operation-addressed key for one versioned derived seal reservation.
///
/// # Errors
/// Refuses a key exceeding the active journal key limit.
pub fn derived_artifact_reservation_key(
    operation: MutationId,
    limits: JournalLimits,
) -> Result<ControlKey, ControlError> {
    operation_key(RESERVATION_PREFIX, operation, limits)
}

/// Full operation-addressed key for the captured five-owner guard values.
///
/// # Errors
/// Refuses a key exceeding the active journal key limit.
pub fn derived_artifact_guard_snapshot_key(
    operation: MutationId,
    limits: JournalLimits,
) -> Result<ControlKey, ControlError> {
    operation_key(GUARD_PREFIX, operation, limits)
}

/// Full operation-addressed key for one late object publication intent.
///
/// # Errors
/// Refuses a key exceeding the active journal key limit.
pub fn derived_artifact_object_intent_key(
    operation: MutationId,
    limits: JournalLimits,
) -> Result<ControlKey, ControlError> {
    operation_key(OBJECT_INTENT_PREFIX, operation, limits)
}

/// Complete point-read scope for reservation construction.
///
/// Includes shared usage/material/current rows, explicit absence for all three
/// new v1 rows, and the five actual owner guard keys from one coherent read.
///
/// # Errors
/// Refuses over-limit keys or a read scope containing duplicate key addresses.
pub fn derived_artifact_reservation_keys(
    operation: MutationId,
    guards: &DerivedArtifactScopeGuardKeysV1,
    limits: JournalLimits,
) -> Result<Vec<ControlKey>, ControlError> {
    let mut keys = super::revision_use_keys(limits)?.to_vec();
    keys.extend([
        derived_artifact_subject_key(operation, limits)?,
        derived_artifact_reservation_key(operation, limits)?,
        derived_artifact_guard_snapshot_key(operation, limits)?,
    ]);
    keys.extend(guards.ordered().into_iter().cloned());
    ensure_unique_keys(&keys)?;
    Ok(keys)
}

/// Complete point-read scope for late-intent binding/readback.
///
/// # Errors
/// Refuses over-limit keys or a read scope containing duplicate key addresses.
pub fn derived_artifact_object_intent_keys(
    operation: MutationId,
    guards: &DerivedArtifactScopeGuardKeysV1,
    limits: JournalLimits,
) -> Result<Vec<ControlKey>, ControlError> {
    let mut keys = super::revision_use_keys(limits)?.to_vec();
    keys.extend([
        derived_artifact_subject_key(operation, limits)?,
        derived_artifact_reservation_key(operation, limits)?,
        derived_artifact_guard_snapshot_key(operation, limits)?,
        derived_artifact_object_intent_key(operation, limits)?,
    ]);
    keys.extend(guards.ordered().into_iter().cloned());
    ensure_unique_keys(&keys)?;
    Ok(keys)
}

#[derive(Clone, Eq, PartialEq)]
struct DerivedSubject {
    operation_id: OpaqueId,
    source_revision: SourceRevisionRef,
    materialization_id: MaterializationId,
    representation_id: RepresentationId,
    preparation: Vec<u8>,
    descriptor_canonical: Vec<u8>,
    residency_canonical: Vec<u8>,
}

impl DerivedSubject {
    fn encode(&self, limits: JournalLimits) -> Result<ControlValue, ControlError> {
        super::record(
            vec![
                super::text(SUBJECT_MARKER)?,
                V::U64(1),
                super::text(self.operation_id.as_str())?,
                super::bytes(self.source_revision.source_namespace_id.as_bytes())?,
                super::bytes(self.source_revision.source_id.as_bytes())?,
                super::bytes(self.source_revision.revision_id.as_bytes())?,
                super::bytes(self.source_revision.content_digest.as_bytes())?,
                V::U64(self.source_revision.byte_length),
                super::bytes(self.materialization_id.as_bytes())?,
                super::bytes(self.representation_id.as_bytes())?,
                super::bytes(&self.preparation)?,
                super::bytes(&self.descriptor_canonical)?,
                super::bytes(&self.residency_canonical)?,
            ],
            limits,
        )
    }

    fn decode(value: &ControlValue) -> Result<Self, ControlError> {
        if value.class() != ControlRecordClass::State || value.len() > MAX_SUBJECT_RECORD_BYTES {
            return Err(ControlError::StoreCorrupt);
        }
        let V::Array(fields) =
            c::parse_canonical_cbor(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?
        else {
            return Err(ControlError::StoreCorrupt);
        };
        let [
            V::Text(marker),
            V::U64(1),
            V::Text(operation_id),
            V::Bytes(namespace),
            V::Bytes(source),
            V::Bytes(revision),
            V::Bytes(content_digest),
            V::U64(source_length),
            V::Bytes(materialization),
            V::Bytes(representation),
            V::Bytes(preparation),
            V::Bytes(descriptor),
            V::Bytes(residency),
        ] = fields.as_slice()
        else {
            return Err(ControlError::StoreCorrupt);
        };
        if marker.as_str() != SUBJECT_MARKER {
            return Err(ControlError::StoreCorrupt);
        }
        let result = Self {
            operation_id: OpaqueId::new(operation_id.as_str())
                .map_err(|_| ControlError::StoreCorrupt)?,
            source_revision: SourceRevisionRef {
                source_namespace_id: uuid16(namespace.as_slice())?,
                source_id: uuid16(source.as_slice())?,
                revision_id: uuid16(revision.as_slice())?,
                content_digest: digest32(content_digest.as_slice())?,
                byte_length: *source_length,
            },
            materialization_id: uuid16(materialization.as_slice())?,
            representation_id: uuid16(representation.as_slice())?,
            preparation: preparation.as_slice().to_vec(),
            descriptor_canonical: descriptor.as_slice().to_vec(),
            residency_canonical: residency.as_slice().to_vec(),
        };
        super::canonical(&result.preparation, super::MAX_OWNER_RECORD)?;
        super::canonical(
            &result.descriptor_canonical,
            MAX_DERIVED_ARTIFACT_DESCRIPTOR_BYTES,
        )?;
        super::canonical(&result.residency_canonical, super::MAX_OWNER_RECORD)?;
        Ok(result)
    }
}

#[derive(Clone, Eq, PartialEq)]
struct DerivedReservation {
    current_canonical: Vec<u8>,
    aad_canonical: Vec<u8>,
    plaintext_length: u64,
    counter: u64,
    blocks: u64,
}

impl DerivedReservation {
    fn encode(&self, limits: JournalLimits) -> Result<ControlValue, ControlError> {
        super::record(
            vec![
                super::text(RESERVATION_MARKER)?,
                V::U64(1),
                super::bytes(&self.current_canonical)?,
                super::bytes(&self.aad_canonical)?,
                V::U64(self.plaintext_length),
                V::U64(self.counter),
                V::U64(self.blocks),
            ],
            limits,
        )
    }

    fn decode(value: &ControlValue) -> Result<Self, ControlError> {
        if value.class() != ControlRecordClass::State || value.len() > MAX_RESERVATION_RECORD_BYTES
        {
            return Err(ControlError::StoreCorrupt);
        }
        let V::Array(fields) =
            c::parse_canonical_cbor(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?
        else {
            return Err(ControlError::StoreCorrupt);
        };
        let [
            V::Text(marker),
            V::U64(1),
            V::Bytes(current),
            V::Bytes(aad),
            V::U64(plaintext_length),
            V::U64(counter),
            V::U64(blocks),
        ] = fields.as_slice()
        else {
            return Err(ControlError::StoreCorrupt);
        };
        if marker.as_str() != RESERVATION_MARKER {
            return Err(ControlError::StoreCorrupt);
        }
        let reservation = Self {
            current_canonical: current.as_slice().to_vec(),
            aad_canonical: aad.as_slice().to_vec(),
            plaintext_length: *plaintext_length,
            counter: *counter,
            blocks: *blocks,
        };
        super::canonical(&reservation.current_canonical, super::MAX_OWNER_RECORD)?;
        super::canonical(&reservation.aad_canonical, super::MAX_AAD)?;
        if reservation.plaintext_length > super::MAX_REVISION_USE_PLAINTEXT_BYTES
            || reservation.counter >= super::MAX_REVISION_USE_ATTEMPTS
            || reservation.blocks == 0
            || reservation.blocks > super::MAX_REVISION_USE_BLOCKS
            || authenticated_blocks(
                reservation.plaintext_length,
                reservation.aad_canonical.len(),
            )? != reservation.blocks
        {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(reservation)
    }
}

#[derive(Clone, Eq, PartialEq)]
struct DerivedGuardSnapshot {
    entries: Vec<(ControlKey, ControlValue)>,
}

impl DerivedGuardSnapshot {
    fn capture(
        read: &ScopedControlRecordsReadback,
        keys: &DerivedArtifactScopeGuardKeysV1,
    ) -> Result<Self, ControlError> {
        let mut entries = Vec::with_capacity(5);
        let mut encoded_bytes = 0_usize;
        for key in keys.ordered() {
            let value = read.get(key).ok_or(ControlError::InvalidValue)?.clone();
            if value.len() > super::MAX_OWNER_RECORD {
                return Err(ControlError::BudgetExceeded);
            }
            encoded_bytes = encoded_bytes
                .checked_add(key.as_bytes().len())
                .and_then(|n| n.checked_add(value.len()))
                .ok_or(ControlError::BudgetExceeded)?;
            entries.push((key.clone(), value));
        }
        if entries.len() != 5 || encoded_bytes > MAX_GUARD_RECORD_BYTES {
            return Err(ControlError::BudgetExceeded);
        }
        Ok(Self { entries })
    }

    fn encode(&self, limits: JournalLimits) -> Result<ControlValue, ControlError> {
        let mut encoded = Vec::with_capacity(self.entries.len());
        for (key, value) in &self.entries {
            encoded.push(array(vec![
                super::bytes(key.as_bytes())?,
                V::U64(class_code(value.class())),
                super::bytes(value.as_bytes())?,
            ])?);
        }
        super::record(
            vec![super::text(GUARD_MARKER)?, V::U64(1), array(encoded)?],
            limits,
        )
    }

    fn decode(value: &ControlValue, limits: JournalLimits) -> Result<Self, ControlError> {
        if value.class() != ControlRecordClass::State || value.len() > MAX_GUARD_RECORD_BYTES {
            return Err(ControlError::StoreCorrupt);
        }
        let V::Array(fields) =
            c::parse_canonical_cbor(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?
        else {
            return Err(ControlError::StoreCorrupt);
        };
        let [V::Text(marker), V::U64(1), V::Array(entries)] = fields.as_slice() else {
            return Err(ControlError::StoreCorrupt);
        };
        if marker.as_str() != GUARD_MARKER || entries.as_slice().len() != 5 {
            return Err(ControlError::StoreCorrupt);
        }
        let mut decoded = Vec::with_capacity(5);
        for entry in entries.as_slice() {
            let V::Array(parts) = entry else {
                return Err(ControlError::StoreCorrupt);
            };
            let [V::Bytes(key_bytes), V::U64(class), V::Bytes(value_bytes)] = parts.as_slice()
            else {
                return Err(ControlError::StoreCorrupt);
            };
            let key = ControlKey::new(key_bytes.as_slice().to_vec(), limits)
                .map_err(|_| ControlError::StoreCorrupt)?;
            let class = class_from_code(*class)?;
            let value = ControlValue::new(class, value_bytes.as_slice().to_vec(), limits)
                .map_err(|_| ControlError::StoreCorrupt)?;
            if value.len() > super::MAX_OWNER_RECORD {
                return Err(ControlError::StoreCorrupt);
            }
            decoded.push((key, value));
        }
        ensure_unique_guard_entries(&decoded)?;
        Ok(Self { entries: decoded })
    }

    fn keys(&self) -> [&ControlKey; 5] {
        [
            &self.entries[0].0,
            &self.entries[1].0,
            &self.entries[2].0,
            &self.entries[3].0,
            &self.entries[4].0,
        ]
    }

    fn matches_keys(&self, keys: &DerivedArtifactScopeGuardKeysV1) -> bool {
        self.keys()
            .into_iter()
            .zip(keys.ordered())
            .all(|(stored, supplied)| stored == supplied)
    }

    fn matches_read(&self, read: &ScopedControlRecordsReadback) -> bool {
        self.entries
            .iter()
            .all(|(key, expected)| read.get(key) == Some(expected))
    }

    fn conditions(&self) -> Vec<ControlRecordCondition> {
        self.entries
            .iter()
            .map(|(key, value)| ControlRecordCondition::exact(key.clone(), value.clone()))
            .collect()
    }
}

#[derive(Clone, Eq, PartialEq)]
struct DerivedObjectIntent {
    reservation_mutation_id: MutationId,
    operation_id: OpaqueId,
    counter: u64,
    blocks: u64,
    plaintext_length: u64,
    object_reference_canonical: Vec<u8>,
    address_bytes: Vec<u8>,
    stored_object_digest: Blake3Digest32,
    stored_object_length: u64,
}

impl DerivedObjectIntent {
    fn encode(&self, limits: JournalLimits) -> Result<ControlValue, ControlError> {
        super::record(
            vec![
                super::text(OBJECT_INTENT_MARKER)?,
                V::U64(1),
                super::bytes(&self.reservation_mutation_id.0)?,
                super::text(self.operation_id.as_str())?,
                V::U64(self.counter),
                V::U64(self.blocks),
                V::U64(self.plaintext_length),
                super::bytes(&self.object_reference_canonical)?,
                super::bytes(&self.address_bytes)?,
                super::bytes(self.stored_object_digest.as_bytes())?,
                V::U64(self.stored_object_length),
            ],
            limits,
        )
    }

    fn decode(value: &ControlValue) -> Result<Self, ControlError> {
        if value.class() != ControlRecordClass::State
            || value.len() > MAX_OBJECT_INTENT_RECORD_BYTES
        {
            return Err(ControlError::StoreCorrupt);
        }
        let V::Array(fields) =
            c::parse_canonical_cbor(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?
        else {
            return Err(ControlError::StoreCorrupt);
        };
        let [
            V::Text(marker),
            V::U64(1),
            V::Bytes(reservation_id),
            V::Text(operation_id),
            V::U64(counter),
            V::U64(blocks),
            V::U64(plaintext_length),
            V::Bytes(reference),
            V::Bytes(address),
            V::Bytes(digest),
            V::U64(stored_length),
        ] = fields.as_slice()
        else {
            return Err(ControlError::StoreCorrupt);
        };
        if marker.as_str() != OBJECT_INTENT_MARKER {
            return Err(ControlError::StoreCorrupt);
        }
        let result = Self {
            reservation_mutation_id: MutationId(
                reservation_id
                    .as_slice()
                    .try_into()
                    .map_err(|_| ControlError::StoreCorrupt)?,
            ),
            operation_id: OpaqueId::new(operation_id.as_str())
                .map_err(|_| ControlError::StoreCorrupt)?,
            counter: *counter,
            blocks: *blocks,
            plaintext_length: *plaintext_length,
            object_reference_canonical: reference.as_slice().to_vec(),
            address_bytes: address.as_slice().to_vec(),
            stored_object_digest: digest32(digest.as_slice())?,
            stored_object_length: *stored_length,
        };
        super::canonical(
            &result.object_reference_canonical,
            MAX_DERIVED_ARTIFACT_REFERENCE_BYTES,
        )?;
        if result.address_bytes.is_empty()
            || result.address_bytes.len() > MAX_DERIVED_ARTIFACT_ADDRESS_BYTES
            || result.counter >= super::MAX_REVISION_USE_ATTEMPTS
            || result.blocks == 0
            || result.blocks > super::MAX_REVISION_USE_BLOCKS
            || result.plaintext_length > super::MAX_REVISION_USE_PLAINTEXT_BYTES
            || result.stored_object_length == 0
        {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(result)
    }
}

fn operation_key(
    prefix: &[u8],
    operation: MutationId,
    limits: JournalLimits,
) -> Result<ControlKey, ControlError> {
    let mut bytes = prefix.to_vec();
    for byte in operation.0 {
        bytes.push(HEX[usize::from(byte >> 4)]);
        bytes.push(HEX[usize::from(byte & 15)]);
    }
    super::key(&bytes, limits)
}

fn authenticated_blocks(plaintext_length: u64, aad_length: usize) -> Result<u64, ControlError> {
    let aad = u64::try_from(aad_length).map_err(|_| ControlError::BudgetExceeded)?;
    plaintext_length
        .div_ceil(16)
        .checked_add(aad.div_ceil(16))
        .and_then(|blocks| blocks.checked_add(1))
        .ok_or(ControlError::BudgetExceeded)
}

fn array(values: Vec<V>) -> Result<V, ControlError> {
    Ok(V::Array(
        c::BoundedList::new(values).map_err(|_| ControlError::InvalidValue)?,
    ))
}

fn uuid16<T>(bytes: &[u8]) -> Result<T, ControlError>
where
    T: UuidFromBytes,
{
    let value: [u8; 16] = bytes.try_into().map_err(|_| ControlError::StoreCorrupt)?;
    Ok(T::from_uuid_bytes(value))
}

trait UuidFromBytes {
    fn from_uuid_bytes(bytes: [u8; 16]) -> Self;
}

macro_rules! uuid_from_bytes {
    ($($id:ty),+ $(,)?) => {
        $(impl UuidFromBytes for $id {
            fn from_uuid_bytes(bytes: [u8; 16]) -> Self { <$id>::from_bytes(bytes) }
        })+
    };
}

uuid_from_bytes!(
    SourceNamespaceId,
    SourceId,
    SourceRevisionId,
    MaterializationId,
    RepresentationId
);

fn digest32(bytes: &[u8]) -> Result<Blake3Digest32, ControlError> {
    let stored: [u8; 32] = bytes.try_into().map_err(|_| ControlError::StoreCorrupt)?;
    Ok(Blake3Digest32::from_stored_bytes(stored))
}

fn ensure_unique_keys(keys: &[ControlKey]) -> Result<(), ControlError> {
    let mut unique = BTreeSet::new();
    if keys.iter().any(|key| !unique.insert(key.as_bytes())) {
        return Err(ControlError::DuplicateMutationKey);
    }
    Ok(())
}

fn ensure_unique_conditions(conditions: &[ControlRecordCondition]) -> Result<(), ControlError> {
    let mut unique = BTreeSet::new();
    if conditions
        .iter()
        .any(|condition| !unique.insert(condition.key().as_bytes()))
    {
        return Err(ControlError::DuplicateMutationKey);
    }
    Ok(())
}

fn ensure_unique_guard_entries(entries: &[(ControlKey, ControlValue)]) -> Result<(), ControlError> {
    let mut unique = BTreeSet::new();
    if entries
        .iter()
        .any(|(key, _)| !unique.insert(key.as_bytes()))
    {
        return Err(ControlError::StoreCorrupt);
    }
    Ok(())
}

const fn class_code(class: ControlRecordClass) -> u64 {
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

const fn class_from_code(code: u64) -> Result<ControlRecordClass, ControlError> {
    match code {
        1 => Ok(ControlRecordClass::Identity),
        2 => Ok(ControlRecordClass::Revision),
        3 => Ok(ControlRecordClass::State),
        4 => Ok(ControlRecordClass::Receipt),
        5 => Ok(ControlRecordClass::Operation),
        6 => Ok(ControlRecordClass::Snapshot),
        7 => Ok(ControlRecordClass::Migration),
        _ => Err(ControlError::StoreCorrupt),
    }
}
