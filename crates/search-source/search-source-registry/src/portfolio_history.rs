//! Immutable reference-portfolio revisions and guarded current pointers.
//!
//! A published portfolio revision is addressed by the exact pair
//! `(portfolio_id, portfolio_revision)` and is never rewritten in place. The
//! previous storage was keyed by `ReferencePortfolioId` alone, so publishing a
//! later revision silently replaced the earlier record and the same immutable
//! `ReferencePortfolioRevision` contract reference could change meaning.
//!
//! This module keeps every revision append-only and moves the only mutable
//! state into guarded current pointers. Pointers are per portfolio identity,
//! so publishing one portfolio never erases another identity's current
//! selection. Same key with identical canonical content replays the exact
//! retained record. Same key with different content refuses, because
//! overwriting an accepted revision would rewrite history that queries still
//! reference. Reclaiming an old revision is explicit retention work and never
//! occurs here.
//!
//! A pointer carries only the exact target revision, its own pointer revision,
//! the expected-current guard and the operation identity. It never carries
//! membership precedence or any other record content, so moving the pointer
//! cannot redefine the revision it targets.

use std::collections::BTreeMap;

use search_contracts::{
    BoundedBytes, BoundedList, BoundedMap, CanonicalDigestDomain, CanonicalKey, CanonicalText,
    CanonicalValue, ContractError, DigestInputLimit, MAX_CANONICAL_BYTES, MAX_MAP_ENTRIES,
    NonZeroRevision, OpaqueId, PortfolioRevision, ReceiptRef, ReferencePortfolioId,
    SourceMembershipId, blake3_canonical,
};

use crate::error::RegistryError;

/// Domain for the canonical portfolio-revision content digest.
const REVISION_DIGEST_DOMAIN: &str = "eliot/cbor/portfolio-revision/v1";
/// Domain for the canonical portfolio current-pointer digest.
const POINTER_DIGEST_DOMAIN: &str = "eliot/cbor/portfolio-current-pointer/v1";
/// Frozen canonical portfolio-revision record version.
const RECORD_VERSION: u8 = 1;
/// Canonical current-pointer record version.
const POINTER_VERSION: u8 = 1;

/// Frozen digest domain, parsed from a compile-time constant.
fn digest_domain(name: &'static str) -> CanonicalDigestDomain {
    CanonicalDigestDomain::parse(name).expect("frozen digest domain")
}

/// Finite preimage ceiling for one canonical digest call.
fn digest_limit() -> DigestInputLimit {
    DigestInputLimit::new(MAX_CANONICAL_BYTES).expect("canonical digest ceiling")
}

/// Bounded canonical text value.
fn canonical_text(value: &str) -> Result<CanonicalValue, ContractError> {
    CanonicalText::new(value).map(CanonicalValue::Text)
}

/// Bounded canonical identity value.
fn canonical_uuid(value: &[u8; 16]) -> Result<CanonicalValue, ContractError> {
    BoundedBytes::new(value.to_vec()).map(CanonicalValue::Bytes)
}

/// Bounded canonical revision value.
fn canonical_u64(value: u64) -> CanonicalValue {
    CanonicalValue::U64(value)
}

/// Bounded canonical list of rows.
fn canonical_list(rows: Vec<CanonicalValue>) -> Result<CanonicalValue, ContractError> {
    BoundedList::new(rows).map(CanonicalValue::Array)
}

/// Builds one bounded canonical object from validated entries.
fn canonical_object(
    entries: Vec<(&'static str, CanonicalValue)>,
) -> Result<CanonicalValue, ContractError> {
    BoundedMap::<CanonicalKey, CanonicalValue, MAX_MAP_ENTRIES>::from_entries(
        entries
            .into_iter()
            .map(|(key, value)| CanonicalKey::new_non_empty(key).map(|key| (key, value)))
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map(CanonicalValue::Object)
}

/// Immutable key of one portfolio revision: identity plus exact revision.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortfolioRevisionKey {
    /// Stable portfolio identity.
    pub portfolio_id: ReferencePortfolioId,
    /// Exact immutable portfolio revision.
    pub portfolio_revision: PortfolioRevision,
}

impl PortfolioRevisionKey {
    /// Builds one exact revision key from a validated identity and revision.
    #[must_use]
    pub const fn new(
        portfolio_id: ReferencePortfolioId,
        portfolio_revision: PortfolioRevision,
    ) -> Self {
        Self {
            portfolio_id,
            portfolio_revision,
        }
    }

    /// Stable portfolio identity of this key.
    #[must_use]
    pub const fn portfolio_id(&self) -> ReferencePortfolioId {
        self.portfolio_id
    }

    /// Exact immutable portfolio revision of this key.
    #[must_use]
    pub const fn portfolio_revision(&self) -> PortfolioRevision {
        self.portfolio_revision
    }

    /// Canonical value binding the exact identity and revision.
    fn canonical_value(&self) -> Result<CanonicalValue, ContractError> {
        canonical_object(vec![
            (
                "portfolio_id",
                canonical_uuid(self.portfolio_id.as_bytes())?,
            ),
            (
                "portfolio_revision",
                canonical_u64(self.portfolio_revision.get()),
            ),
        ])
    }
}

/// Immutable content of one published reference-portfolio revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImmutablePortfolioRevision {
    membership_precedence: Vec<SourceMembershipId>,
}

impl ImmutablePortfolioRevision {
    /// Creates one immutable revision from an explicitly ordered precedence.
    #[must_use]
    pub const fn new(membership_precedence: Vec<SourceMembershipId>) -> Self {
        Self {
            membership_precedence,
        }
    }

    /// Exact ordered membership precedence of this revision.
    #[must_use]
    pub fn membership_precedence(&self) -> &[SourceMembershipId] {
        &self.membership_precedence
    }

    /// Canonical value of this exact revision content.
    ///
    /// Order is part of identity, so a permutation is a different revision
    /// rather than a relabelling. Durability references are excluded.
    fn canonical_value(&self) -> Result<CanonicalValue, ContractError> {
        let mut rows = Vec::with_capacity(self.membership_precedence.len());
        for membership_id in &self.membership_precedence {
            rows.push(canonical_uuid(membership_id.as_bytes())?);
        }
        canonical_object(vec![
            ("record_version", canonical_u64(u64::from(RECORD_VERSION))),
            ("membership_precedence", canonical_list(rows)?),
        ])
    }

    /// Canonical content digest through the accepted digest owner.
    ///
    /// The digest covers ordered membership content only. Receipt
    /// references, registry revisions and operation identities never enter
    /// revision identity, so re-issuing durability metadata cannot redefine
    /// a revision. An over-ceiling preimage yields no digest.
    ///
    /// # Errors
    /// Returns the underlying canonical encode or bound failure.
    pub fn content_digest(&self) -> Result<search_contracts::Blake3Digest32, ContractError> {
        blake3_canonical(
            &digest_domain(REVISION_DIGEST_DOMAIN),
            &self.canonical_value()?,
            digest_limit(),
        )
    }
}

/// Append-only immutable portfolio history across portfolio identities.
///
/// Retaining every revision is the point. The retained record count is
/// bounded by one explicit aggregate ceiling, so history is bounded in total
/// rather than limited to one latest record per identity.
///
/// Current pointers are stored per portfolio identity. Publishing or moving
/// one identity never erases another identity current selection, and an
/// exact operation replay returns the historical publication receipt and
/// revisions even after another identity has published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioHistory {
    revisions: BTreeMap<PortfolioRevisionKey, PortfolioRevisionRecord>,
    current: BTreeMap<ReferencePortfolioId, PortfolioCurrentPointer>,
}

impl PortfolioHistory {
    /// Creates an empty history with no current pointers.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            revisions: BTreeMap::new(),
            current: BTreeMap::new(),
        }
    }

    /// Number of retained immutable revisions across every identity.
    #[must_use]
    pub fn len(&self) -> usize {
        self.revisions.len()
    }

    /// Whether any immutable revision is retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.revisions.is_empty()
    }

    /// Exact historical read of one immutable revision, if retained.
    #[must_use]
    pub fn revision(&self, key: &PortfolioRevisionKey) -> Option<&PortfolioRevisionRecord> {
        self.revisions.get(key)
    }

    /// Every retained revision, ordered by exact key, for bounded reads.
    pub fn revisions(&self) -> impl Iterator<Item = &PortfolioRevisionRecord> {
        self.revisions.values()
    }

    /// Current pointer of one exact portfolio identity, if any.
    #[must_use]
    pub fn current_pointer(
        &self,
        portfolio_id: ReferencePortfolioId,
    ) -> Option<&PortfolioCurrentPointer> {
        self.current.get(&portfolio_id)
    }

    /// Exact current target revision of one portfolio identity, if any.
    #[must_use]
    pub fn current_key(&self, portfolio_id: ReferencePortfolioId) -> Option<PortfolioRevisionKey> {
        self.current
            .get(&portfolio_id)
            .map(PortfolioCurrentPointer::target)
    }

    /// Exact current immutable revision of one portfolio identity, if any.
    #[must_use]
    pub fn current_revision(
        &self,
        portfolio_id: ReferencePortfolioId,
    ) -> Option<&PortfolioRevisionRecord> {
        self.current_key(portfolio_id)
            .as_ref()
            .and_then(|key| self.revisions.get(key))
    }

    /// Publishes one immutable revision, or replays an identical retained one.
    ///
    /// Same key with identical content replays the exact retained record. Same
    /// key with different content refuses, because an accepted revision is
    /// immutable. The aggregate record ceiling is not per identity, so one
    /// portfolio with many revisions cannot exhaust history while another
    /// identity holds none.
    ///
    /// # Errors
    /// Returns [`RegistryError::ReferencePortfolioInvalid`] for a conflicting
    /// retained revision and [`RegistryError::CapacityExceeded`] when the
    /// aggregate retained-record ceiling is already reached.
    #[cfg(test)]
    pub(crate) fn publish_revision(
        &mut self,
        key: PortfolioRevisionKey,
        revision: ImmutablePortfolioRevision,
        registry_revision: u64,
        receipt: &ReceiptRef,
        total_limit: usize,
        operation_id: &OpaqueId,
    ) -> Result<PortfolioRevisionRecord, RegistryError> {
        let record = self.prepare_revision(
            key,
            revision,
            registry_revision,
            receipt,
            total_limit,
            operation_id,
        )?;
        self.revisions.insert(key, record.clone());
        Ok(record)
    }

    fn prepare_revision(
        &self,
        key: PortfolioRevisionKey,
        revision: ImmutablePortfolioRevision,
        registry_revision: u64,
        receipt: &ReceiptRef,
        total_limit: usize,
        operation_id: &OpaqueId,
    ) -> Result<PortfolioRevisionRecord, RegistryError> {
        if let Some(retained) = self.revisions.get(&key) {
            return if retained.revision == revision {
                Ok(retained.clone())
            } else {
                Err(RegistryError::ReferencePortfolioInvalid)
            };
        }
        if key.portfolio_revision.get() == 0 || registry_revision == 0 {
            return Err(RegistryError::ReferencePortfolioInvalid);
        }
        revision
            .content_digest()
            .map_err(|_| RegistryError::ReferencePortfolioInvalid)?;
        if self.revisions.len() >= total_limit {
            return Err(RegistryError::CapacityExceeded);
        }
        let record = PortfolioRevisionRecord {
            key,
            revision,
            registry_revision,
            last_receipt: receipt.clone(),
            operation_id: operation_id.clone(),
        };
        Ok(record)
    }

    /// Moves the guarded current pointer of one exact portfolio identity.
    ///
    /// A stale expected-current, a skipped or regressed pointer revision, or a
    /// competing move conflicts. A move never mutates the target revision and
    /// never deletes a prior revision, so an older pointer move leaves the
    /// previous target permanently readable. Other identities are untouched.
    ///
    /// # Errors
    /// Returns [`RegistryError::ReferencePortfolioInvalid`] when the target
    /// revision is not retained, [`RegistryError::ReferenceScopeEmpty`] when
    /// an expected-current guard does not match the retained pointer,
    /// [`RegistryError::MembershipGenerationMismatch`] for a non-advancing
    /// pointer revision and [`RegistryError::OperationConflict`] when the
    /// same operation identity already moved this identity elsewhere.
    #[cfg(test)]
    pub(crate) fn move_current(
        &mut self,
        key: PortfolioRevisionKey,
        pointer_revision: NonZeroRevision,
        expected_current: Option<PortfolioRevisionKey>,
        operation_id: &OpaqueId,
        receipt: &ReceiptRef,
    ) -> Result<PortfolioCurrentPointer, RegistryError> {
        if !self.revisions.contains_key(&key) {
            return Err(RegistryError::ReferencePortfolioInvalid);
        }
        let pointer = self.prepare_pointer(
            key,
            pointer_revision,
            expected_current,
            operation_id,
            receipt,
        )?;
        self.current.insert(key.portfolio_id, pointer.clone());
        Ok(pointer)
    }

    fn prepare_pointer(
        &self,
        key: PortfolioRevisionKey,
        pointer_revision: NonZeroRevision,
        expected_current: Option<PortfolioRevisionKey>,
        operation_id: &OpaqueId,
        receipt: &ReceiptRef,
    ) -> Result<PortfolioCurrentPointer, RegistryError> {
        let retained = self.current.get(&key.portfolio_id);
        match (retained, expected_current) {
            (Some(retained), Some(expected)) => {
                if retained.target != expected {
                    return Err(RegistryError::ReferenceScopeEmpty);
                }
                if retained.pointer_revision.checked_next().ok() != Some(pointer_revision) {
                    return Err(RegistryError::MembershipGenerationMismatch);
                }
                if retained.operation_id == *operation_id && retained.target != key {
                    return Err(RegistryError::OperationConflict);
                }
            }
            (None, Some(_)) => return Err(RegistryError::ReferenceScopeEmpty),
            (Some(_), None) => return Err(RegistryError::ReferenceScopeEmpty),
            (None, None) if pointer_revision.get() == 1 => {}
            (None, None) => return Err(RegistryError::MembershipGenerationMismatch),
        }
        let pointer = PortfolioCurrentPointer {
            target: key,
            pointer_revision,
            operation_id: operation_id.clone(),
            last_receipt: receipt.clone(),
        };
        pointer
            .pointer_digest()
            .map_err(|_| RegistryError::ReferencePortfolioInvalid)?;
        Ok(pointer)
    }

    /// Prepares bounded record and pointer values before receipt persistence.
    pub(crate) fn prepare_publication(
        &self,
        key: PortfolioRevisionKey,
        revision: ImmutablePortfolioRevision,
        registry_revision: u64,
        receipt: &ReceiptRef,
        operation_id: &OpaqueId,
        total_limit: usize,
        expected_current: Option<PortfolioRevisionKey>,
    ) -> Result<PreparedPortfolioPublication, RegistryError> {
        let record = self.prepare_revision(
            key,
            revision,
            registry_revision,
            receipt,
            total_limit,
            operation_id,
        )?;
        let next = self
            .current_pointer(key.portfolio_id)
            .map_or_else(
                || NonZeroRevision::new(1),
                |pointer| pointer.pointer_revision().checked_next(),
            )
            .map_err(|_| RegistryError::ContractExhausted)?;
        let pointer = self.prepare_pointer(key, next, expected_current, operation_id, receipt)?;
        Ok(PreparedPortfolioPublication { record, pointer })
    }

    /// Installs values whose complete validation preceded receipt dispatch.
    pub(crate) fn install(&mut self, prepared: PreparedPortfolioPublication) {
        self.current
            .insert(prepared.pointer.target.portfolio_id, prepared.pointer);
        self.revisions.insert(prepared.record.key, prepared.record);
    }

    /// Guarded current pointers in identity order.
    pub fn current_pointers(
        &self,
    ) -> impl Iterator<Item = (&ReferencePortfolioId, &PortfolioCurrentPointer)> {
        self.current.iter()
    }

    /// Derived current view of every identity that has a current pointer.
    ///
    /// The returned entries are projections of the immutable revision named
    /// by each identity pointer. They are never storage authority, so
    /// mutating a returned entry cannot change which revisions are retained.
    #[must_use]
    pub fn current_view(&self) -> BTreeMap<ReferencePortfolioId, PortfolioCurrentView> {
        self.current
            .iter()
            .filter_map(|(portfolio_id, pointer)| {
                let record = self.revisions.get(&pointer.target)?;
                Some((
                    *portfolio_id,
                    PortfolioCurrentView {
                        portfolio_id: record.key.portfolio_id,
                        portfolio_revision: record.key.portfolio_revision,
                        membership_precedence: record.revision.membership_precedence().to_vec(),
                        registry_revision: record.registry_revision,
                        last_receipt: record.last_receipt.clone(),
                    },
                ))
            })
            .collect()
    }
}

pub(crate) struct PreparedPortfolioPublication {
    pub(crate) record: PortfolioRevisionRecord,
    pointer: PortfolioCurrentPointer,
}

impl Default for PortfolioHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Derived current view of one portfolio identity.
///
/// This is a projection of the immutable revision named by the current
/// pointer, not storage. Mutating a returned view cannot change which
/// revisions are retained or which pointer is current.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioCurrentView {
    /// Stable portfolio identity.
    pub portfolio_id: ReferencePortfolioId,
    /// Exact revision named by the current pointer.
    pub portfolio_revision: PortfolioRevision,
    /// Ordered membership precedence of that exact revision.
    pub membership_precedence: Vec<SourceMembershipId>,
    /// Registry revision that published the named revision.
    pub registry_revision: u64,
    /// Content-free durability receipt of that publication.
    pub last_receipt: ReceiptRef,
}

/// One retained immutable portfolio revision record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioRevisionRecord {
    key: PortfolioRevisionKey,
    revision: ImmutablePortfolioRevision,
    registry_revision: u64,
    last_receipt: ReceiptRef,
    operation_id: OpaqueId,
}

impl PortfolioRevisionRecord {
    /// Original operation that committed this immutable publication.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }
    /// Exact immutable key of this record.
    #[must_use]
    pub const fn key(&self) -> PortfolioRevisionKey {
        self.key
    }

    /// Exact ordered membership precedence of this revision.
    #[must_use]
    pub fn membership_precedence(&self) -> &[SourceMembershipId] {
        self.revision.membership_precedence()
    }

    /// Stable portfolio identity of this record.
    #[must_use]
    pub const fn portfolio_id(&self) -> ReferencePortfolioId {
        self.key.portfolio_id
    }

    /// Exact immutable revision of this record.
    #[must_use]
    pub const fn portfolio_revision(&self) -> PortfolioRevision {
        self.key.portfolio_revision
    }

    /// Registry revision that published this record.
    #[must_use]
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }

    /// Content-free durability receipt of this publication.
    #[must_use]
    pub const fn last_receipt(&self) -> &ReceiptRef {
        &self.last_receipt
    }

    /// Canonical content digest of the retained revision.
    ///
    /// # Errors
    /// Returns the underlying canonical encode or bound failure.
    pub fn content_digest(&self) -> Result<search_contracts::Blake3Digest32, ContractError> {
        self.revision.content_digest()
    }
}

/// Guarded current pointer for one portfolio identity.
///
/// The pointer is the only mutable portfolio state. It names an immutable
/// target revision and never carries membership content, so a pointer move
/// cannot redefine the revision it targets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioCurrentPointer {
    target: PortfolioRevisionKey,
    pointer_revision: NonZeroRevision,
    operation_id: OpaqueId,
    last_receipt: ReceiptRef,
}

impl PortfolioCurrentPointer {
    /// Exact immutable target revision selected by this pointer.
    #[must_use]
    pub const fn target(&self) -> PortfolioRevisionKey {
        self.target
    }

    /// Monotone pointer revision; a skipped or regressed value conflicts.
    #[must_use]
    pub const fn pointer_revision(&self) -> NonZeroRevision {
        self.pointer_revision
    }

    /// Operation identity that last moved this pointer.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }

    /// Content-free durability receipt of the last pointer move.
    #[must_use]
    pub const fn last_receipt(&self) -> &ReceiptRef {
        &self.last_receipt
    }

    /// Canonical value binding target, pointer revision and operation.
    fn canonical_value(&self) -> Result<CanonicalValue, ContractError> {
        canonical_object(vec![
            ("pointer_version", canonical_u64(u64::from(POINTER_VERSION))),
            ("target", self.target.canonical_value()?),
            (
                "pointer_revision",
                canonical_u64(self.pointer_revision.get()),
            ),
            ("operation", canonical_text(self.operation_id.as_str())?),
        ])
    }

    /// Canonical digest of this exact pointer state.
    ///
    /// # Errors
    /// Returns the underlying canonical encode or bound failure.
    pub fn pointer_digest(&self) -> Result<search_contracts::Blake3Digest32, ContractError> {
        blake3_canonical(
            &digest_domain(POINTER_DIGEST_DOMAIN),
            &self.canonical_value()?,
            digest_limit(),
        )
    }
}
