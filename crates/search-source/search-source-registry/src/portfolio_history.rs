//! Immutable reference-portfolio revisions and guarded current pointers.
//!
//! A published portfolio revision is addressed by the exact pair
//! `(portfolio_id, portfolio_revision)` and is never rewritten in place. The
//! previous storage was keyed by `ReferencePortfolioId` alone, so publishing a
//! later revision silently replaced the earlier record and the same immutable
//! `ReferencePortfolioRevision` contract reference could change meaning.
//!
//! This module keeps every revision append-only and moves the only mutable
//! state into a separate guarded current pointer. Same key with identical
//! canonical content replays the exact retained record. Same key with different
//! content refuses, because overwriting an accepted revision would rewrite
//! history that queries still reference. Reclaiming an old revision is
//! explicit retention work and never occurs here.
//!
//! The pointer carries only the exact target revision, its own pointer
//! revision, the expected-current guard and the operation identity. It never
//! carries membership precedence or any other record content, so moving the
//! pointer cannot redefine the revision it targets.

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, PortfolioRevision,
    ReceiptRef, ReferencePortfolioId, SourceMembershipId};

use crate::error::RegistryError;

/// Domain separator for the canonical portfolio-revision content preimage.
const REVISION_DOMAIN: &[u8] = b"eliot-search/portfolio-revision/v1\0";
/// Domain separator for the canonical current-pointer preimage.
const POINTER_DOMAIN: &[u8] = b"eliot-search/portfolio-current-pointer/v1\0";
/// Magic prefix of the frozen canonical portfolio-revision record bytes.
const RECORD_MAGIC: &[u8; 4] = b"ELPR";
/// Frozen canonical portfolio-revision record version.
const RECORD_VERSION: u8 = 1;
/// Canonical current-pointer record version.
const POINTER_VERSION: u8 = 1;

/// Immutable key of one portfolio revision: identity plus exact revision.
///
/// The pair, not the identity, is the storage key. Two revisions of one
/// portfolio are two distinct immutable records.
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
        Self { portfolio_id, portfolio_revision }
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
}

/// Immutable content of one published reference-portfolio revision.
///
/// Ordered membership precedence is record content, not a pointer target, so
/// adding, removing or reordering members yields another revision instead of
/// mutating a prior one. Durability references are deliberately excluded from
/// the content preimage so a replayed receipt cannot redefine a revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImmutablePortfolioRevision {
    /// Ordered membership precedence of this exact revision.
    membership_precedence: Vec<SourceMembershipId>,
}

impl ImmutablePortfolioRevision {
    /// Creates one immutable revision from an explicitly ordered precedence.
    #[must_use]
    pub const fn new(membership_precedence: Vec<SourceMembershipId>) -> Self {
        Self { membership_precedence }
    }

    /// Exact ordered membership precedence of this revision.
    #[must_use]
    pub fn membership_precedence(&self) -> &[SourceMembershipId] {
        &self.membership_precedence
    }

    /// Canonical content preimage of this exact revision.
    ///
    /// The preimage binds a versioned domain, the exact precedence length and
    /// every membership identity in order. Order is part of identity, so a
    /// permutation is a different revision rather than a relabelling.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            REVISION_DOMAIN.len() + 8 + 16 * self.membership_precedence.len(),
        );
        bytes.extend_from_slice(REVISION_DOMAIN);
        bytes.push(RECORD_VERSION);
        bytes.extend_from_slice(&(self.membership_precedence.len() as u64).to_be_bytes());
        for membership_id in &self.membership_precedence {
            bytes.extend_from_slice(membership_id.as_bytes());
        }
        bytes
    }

    /// Canonical content digest of this exact revision.
    ///
    /// The digest covers ordered membership content only. Receipt references,
    /// registry revisions and operation identities never enter revision
    /// identity, so re-issuing durability metadata cannot redefine a revision.
    #[must_use]
    pub fn content_digest(&self) -> Blake3Digest32 {
        Blake3Digest32::from_bytes(*blake3::hash(&self.canonical_bytes()).as_bytes())
    }
}

/// One retained immutable portfolio revision record.
///
/// The record stores the exact revision content plus the content-free
/// durability metadata observed when it was published. It never rewrites
/// another revision of the same portfolio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioRevisionRecord {
    /// Exact immutable key of this record.
    pub key: PortfolioRevisionKey,
    /// Immutable revision content.
    pub revision: ImmutablePortfolioRevision,
    /// Registry revision that published this record.
    pub registry_revision: u64,
    /// Content-free durability receipt of this publication.
    pub last_receipt: ReceiptRef,
}

impl PortfolioRevisionRecord {
    /// Canonical content digest of the retained revision.
    #[must_use]
    pub fn content_digest(&self) -> Blake3Digest32 {
        self.revision.content_digest()
    }

    /// Full canonical preimage binding key and content for durable storage.
    ///
    /// The preimage is the frozen wire form of one immutable record. It binds
    /// the exact identity and revision to the ordered content, so a record
    /// decoded from bytes is compared against its own re-encoding.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(160 + self.revision.canonical_bytes().len());
        bytes.extend_from_slice(RECORD_MAGIC);
        bytes.push(RECORD_VERSION);
        bytes.extend_from_slice(self.key.portfolio_id.as_bytes());
        bytes.extend_from_slice(&self.key.portfolio_revision.get().to_be_bytes());
        bytes.extend_from_slice(&self.registry_revision.to_be_bytes());
        let receipt = self.last_receipt.as_str().as_bytes();
        bytes.extend_from_slice(&(receipt.len() as u64).to_be_bytes());
        bytes.extend_from_slice(receipt);
        bytes.extend_from_slice(self.content_digest().as_bytes());
        bytes.extend_from_slice(&self.revision.canonical_bytes());
        bytes
    }
}

/// Guarded current pointer for one portfolio identity.
///
/// The pointer is the only mutable portfolio state. It names an immutable
/// target revision and never carries membership content, so a pointer move
/// cannot redefine the revision it targets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioCurrentPointer {
    /// Exact immutable target revision selected by this pointer.
    pub target: PortfolioRevisionKey,
    /// Monotone pointer revision; a skipped or regressed value conflicts.
    pub pointer_revision: NonZeroRevision,
    /// Operation identity that last moved this pointer.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt of the last pointer move.
    pub last_receipt: ReceiptRef,
}

impl PortfolioCurrentPointer {
    /// Canonical preimage binding target, pointer revision and operation.
    ///
    /// Membership precedence is deliberately absent: the pointer binds the
    /// target revision identity so a pointer cannot be the only copy of the
    /// revision content.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(POINTER_DOMAIN.len() + 96);
        bytes.extend_from_slice(POINTER_DOMAIN);
        bytes.push(POINTER_VERSION);
        bytes.extend_from_slice(self.target.portfolio_id.as_bytes());
        bytes.extend_from_slice(&self.target.portfolio_revision.get().to_be_bytes());
        bytes.extend_from_slice(&self.pointer_revision.get().to_be_bytes());
        bytes.extend_from_slice(self.operation_id.as_str().as_bytes());
        bytes
    }

    /// Canonical digest of this exact pointer state.
    #[must_use]
    pub fn pointer_digest(&self) -> Blake3Digest32 {
        Blake3Digest32::from_bytes(*blake3::hash(&self.canonical_bytes()).as_bytes())
    }
}

/// Append-only immutable portfolio history across portfolio identities.
///
/// Retaining every revision is the point. The total retained record count is
/// bounded by one explicit aggregate ceiling, so history is bounded in total
/// rather than limited to one latest record per identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioHistory {
    /// Every retained immutable revision, ordered by exact key.
    pub revisions: std::collections::BTreeMap<PortfolioRevisionKey, PortfolioRevisionRecord>,
    /// Guarded current pointer, present only after an explicit move.
    pub current: Option<PortfolioCurrentPointer>,
    /// Total retained revisions across every portfolio identity.
    pub total_records: u64,
}

impl PortfolioHistory {
    /// Creates an empty history with no current pointer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            revisions: std::collections::BTreeMap::new(),
            current: None,
            total_records: 0,
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

    /// Exact current target revision key, if a pointer exists.
    ///
    /// The returned key always resolves to a retained record; a pointer naming
    /// an absent revision is refused by construction rather than returned.
    #[must_use]
    pub fn current_key(&self) -> Option<PortfolioRevisionKey> {
        self.current.as_ref().map(|pointer| pointer.target)
    }

    /// Exact current immutable revision, resolved through the guard.
    #[must_use]
    pub fn current_revision(&self) -> Option<&PortfolioRevisionRecord> {
        self.current_key()
            .as_ref()
            .and_then(|key| self.revisions.get(key))
    }

    /// Publishes one immutable revision, or replays an identical retained one.
    ///
    /// Same key with identical content replays the exact retained record. Same
    /// key with different content refuses, because an accepted revision is
    /// immutable. The total retained record ceiling is aggregate, not per
    /// identity, so a portfolio with many revisions cannot exhaust history
    /// while another identity holds none.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::ReferencePortfolioInvalid`] for a conflicting
    /// retained revision and [`RegistryError::CapacityExceeded`] when the
    /// aggregate retained-record ceiling is already reached.
    pub fn publish_revision(
        &mut self,
        key: PortfolioRevisionKey,
        revision: ImmutablePortfolioRevision,
        registry_revision: u64,
        receipt: &ReceiptRef,
        total_limit: usize,
    ) -> Result<PortfolioRevisionRecord, RegistryError> {
        if let Some(retained) = self.revisions.get(&key) {
            return if retained.revision == revision {
                Ok(retained.clone())
            } else {
                Err(RegistryError::ReferencePortfolioInvalid)
            };
        }
        if self.revisions.len() >= total_limit {
            return Err(RegistryError::CapacityExceeded);
        }
        let record = PortfolioRevisionRecord {
            key,
            revision,
            registry_revision,
            last_receipt: receipt.clone(),
        };
        self.revisions.insert(key, record.clone());
        self.total_records = self.revisions.len() as u64;
        Ok(record)
    }

    /// Moves the guarded current pointer under an exact expected-current guard.
    ///
    /// A stale expected-current, a skipped or regressed pointer revision, or a
    /// competing move conflicts. A move never mutates the target revision and
    /// never deletes a prior revision, so an older pointer move leaves the
    /// previous target permanently readable.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::ReferencePortfolioInvalid`] when the target
    /// revision is not retained, [`RegistryError::ReferenceScopeEmpty`] when
    /// the expected-current guard does not match the retained pointer,
    /// [`RegistryError::MembershipGenerationMismatch`] for a non-advancing
    /// pointer revision and [`RegistryError::OperationConflict`] when the
    /// same operation identity already moved this pointer elsewhere.
    pub fn move_current(
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
        match (&self.current, expected_current) {
            (Some(retained), Some(expected)) if retained.target != expected => {
                return Err(RegistryError::ReferenceScopeEmpty);
            }
            (None, Some(_)) => return Err(RegistryError::ReferenceScopeEmpty),
            (Some(_) | None, None) | (Some(_), Some(_)) => {}
        }
        let pointer = PortfolioCurrentPointer {
            target: key,
            pointer_revision,
            operation_id: operation_id.clone(),
            last_receipt: receipt.clone(),
        };
        if let Some(retained) = &self.current {
            if retained.pointer_revision >= pointer_revision {
                return Err(RegistryError::MembershipGenerationMismatch);
            }
            if retained.operation_id == pointer.operation_id
                && retained.target != pointer.target
            {
                return Err(RegistryError::OperationConflict);
            }
        }
        self.current = Some(pointer.clone());
        Ok(pointer)
    }
}

impl Default for PortfolioHistory {
    fn default() -> Self {
        Self::new()
    }
}

