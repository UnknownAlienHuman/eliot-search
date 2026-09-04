//! Deterministic bounded research-export planning.
//!
//! This module owns export accounting and truthfulness, not filesystem or
//! network I/O. Callers supply already-validated citations and payloads, then
//! hand the resulting immutable export to a concrete serializer/writer.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroUsize;

use search_contracts::{Blake3Digest32, RequestId};

/// Supported semantic export representation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExportFormat {
    /// Human-readable Markdown with explicit citations.
    Markdown,
    /// Canonical JSON object/array representation.
    CanonicalJson,
    /// One independently bounded canonical JSON value per line.
    JsonLines,
}

/// Whether the originating retrieval accounted for its authoritative scope.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExportCoverage {
    /// Every item in a frozen authoritative denominator was accounted for.
    Complete,
    /// A known denominator has one or more explicit gaps.
    Partial,
    /// No authoritative denominator was established.
    Unknown,
}

/// Finite export limits applied before accepting an item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportLimits {
    max_items: NonZeroUsize,
    max_gaps: NonZeroUsize,
    max_item_bytes: NonZeroUsize,
    max_total_bytes: NonZeroUsize,
}

impl ExportLimits {
    /// Creates non-zero finite limits.
    ///
    /// # Errors
    ///
    /// Returns [`ExportError::InvalidLimits`] when any dimension is zero or an
    /// item ceiling exceeds the complete export ceiling.
    pub fn new(
        max_items: usize,
        max_gaps: usize,
        max_item_bytes: usize,
        max_total_bytes: usize,
    ) -> Result<Self, ExportError> {
        let limits = Self {
            max_items: NonZeroUsize::new(max_items).ok_or(ExportError::InvalidLimits)?,
            max_gaps: NonZeroUsize::new(max_gaps).ok_or(ExportError::InvalidLimits)?,
            max_item_bytes: NonZeroUsize::new(max_item_bytes)
                .ok_or(ExportError::InvalidLimits)?,
            max_total_bytes: NonZeroUsize::new(max_total_bytes)
                .ok_or(ExportError::InvalidLimits)?,
        };
        if limits.max_item_bytes > limits.max_total_bytes {
            return Err(ExportError::InvalidLimits);
        }
        Ok(limits)
    }

    /// Maximum exported evidence items.
    #[must_use]
    pub const fn max_items(self) -> usize {
        self.max_items.get()
    }

    /// Maximum explicit gaps.
    #[must_use]
    pub const fn max_gaps(self) -> usize {
        self.max_gaps.get()
    }

    /// Maximum encoded bytes for one item.
    #[must_use]
    pub const fn max_item_bytes(self) -> usize {
        self.max_item_bytes.get()
    }

    /// Maximum estimated encoded bytes for the full export.
    #[must_use]
    pub const fn max_total_bytes(self) -> usize {
        self.max_total_bytes.get()
    }
}

/// Policy controlling export admission and finalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportPolicy {
    /// Selected representation.
    pub format: ExportFormat,
    /// Require complete authoritative coverage before finalization.
    pub require_complete_coverage: bool,
    /// Reject multiple evidence items with the same stable evidence identity.
    pub reject_duplicate_evidence: bool,
    /// Require at least one evidence item.
    pub require_non_empty: bool,
}

/// Stable identity and bounded accounting for one research item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResearchItem<Identity, Source, Citation, Payload> {
    /// Stable evidence identity used for deterministic ordering/deduplication.
    pub identity: Identity,
    /// Stable source identity or source-revision reference.
    pub source: Source,
    /// Already-validated citation/anchor representation.
    pub citation: Citation,
    /// Digest of exact exported payload bytes.
    pub payload_digest: Blake3Digest32,
    /// Estimated encoded bytes, including item-local framing.
    pub encoded_bytes: usize,
    /// Opaque already-approved export payload.
    pub payload: Payload,
}

/// Explicit omission or uncertainty kept separate from evidence items.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResearchGap<Identity, Reason> {
    /// Stable identity of the missing scope leg/item.
    pub identity: Identity,
    /// Closed caller-owned reason.
    pub reason: Reason,
}

/// Immutable content-free manifest for an export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportManifest {
    /// Originating request identity.
    pub request_id: RequestId,
    /// Selected format.
    pub format: ExportFormat,
    /// Truthful coverage classification.
    pub coverage: ExportCoverage,
    /// Number of exported evidence items.
    pub item_count: usize,
    /// Number of explicit gaps.
    pub gap_count: usize,
    /// Sum of accepted item byte estimates.
    pub estimated_payload_bytes: usize,
    /// Digest of exact canonical export-plan bytes supplied by the caller.
    pub plan_digest: Blake3Digest32,
}

/// Deterministically ordered export ready for a concrete serializer/writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResearchExport<Identity, Source, Citation, Payload, GapIdentity, GapReason>
where
    Identity: Ord,
    GapIdentity: Ord,
{
    /// Immutable manifest.
    pub manifest: ExportManifest,
    /// Evidence ordered by stable identity.
    pub items: BTreeMap<Identity, ResearchItem<Identity, Source, Citation, Payload>>,
    /// Gaps ordered by stable identity.
    pub gaps: BTreeMap<GapIdentity, ResearchGap<GapIdentity, GapReason>>,
}

/// Builder lifecycle. Finalized builders cannot be mutated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuilderState {
    /// Items and gaps may be accepted.
    Collecting,
    /// Final export has been produced.
    Finalized,
    /// A contradictory duplicate or accounting condition quarantined the plan.
    Quarantined,
}

/// Bounded deterministic research-export builder.
#[derive(Debug)]
pub struct ResearchExportBuilder<Identity, Source, Citation, Payload, GapIdentity, GapReason>
where
    Identity: Clone + Ord,
    GapIdentity: Clone + Ord,
{
    request_id: RequestId,
    limits: ExportLimits,
    policy: ExportPolicy,
    state: BuilderState,
    items: BTreeMap<Identity, ResearchItem<Identity, Source, Citation, Payload>>,
    gaps: BTreeMap<GapIdentity, ResearchGap<GapIdentity, GapReason>>,
    source_identities: BTreeSet<Blake3Digest32>,
    estimated_payload_bytes: usize,
}

impl<Identity, Source, Citation, Payload, GapIdentity, GapReason>
    ResearchExportBuilder<Identity, Source, Citation, Payload, GapIdentity, GapReason>
where
    Identity: Clone + Ord,
    GapIdentity: Clone + Ord,
{
    /// Creates an empty collecting builder.
    #[must_use]
    pub fn new(request_id: RequestId, limits: ExportLimits, policy: ExportPolicy) -> Self {
        Self {
            request_id,
            limits,
            policy,
            state: BuilderState::Collecting,
            items: BTreeMap::new(),
            gaps: BTreeMap::new(),
            source_identities: BTreeSet::new(),
            estimated_payload_bytes: 0,
        }
    }

    /// Current builder lifecycle.
    #[must_use]
    pub const fn state(&self) -> BuilderState {
        self.state
    }

    /// Number of accepted evidence items.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Number of explicit gaps.
    #[must_use]
    pub fn gap_count(&self) -> usize {
        self.gaps.len()
    }

    /// Adds one evidence item.
    ///
    /// `evidence_identity_digest` is a stable digest of the complete logical
    /// evidence identity, not merely source/path text. It is used only when
    /// duplicate evidence rejection is enabled.
    ///
    /// # Errors
    ///
    /// Rejects non-collecting state, item/byte overflow, identity conflicts and
    /// duplicate evidence. A conflicting identity quarantines the builder.
    pub fn push_item(
        &mut self,
        item: ResearchItem<Identity, Source, Citation, Payload>,
        evidence_identity_digest: Blake3Digest32,
    ) -> Result<(), ExportError> {
        self.require_collecting()?;
        if item.encoded_bytes == 0 || item.encoded_bytes > self.limits.max_item_bytes() {
            return Err(ExportError::ItemTooLarge);
        }
        if self.items.len() >= self.limits.max_items() {
            return Err(ExportError::ItemLimitExceeded);
        }
        let next_total = self
            .estimated_payload_bytes
            .checked_add(item.encoded_bytes)
            .ok_or(ExportError::TotalByteLimitExceeded)?;
        if next_total > self.limits.max_total_bytes() {
            return Err(ExportError::TotalByteLimitExceeded);
        }
        if self.items.contains_key(&item.identity) {
            self.state = BuilderState::Quarantined;
            return Err(ExportError::ItemIdentityConflict);
        }
        if self.policy.reject_duplicate_evidence
            && !self.source_identities.insert(evidence_identity_digest)
        {
            return Err(ExportError::DuplicateEvidence);
        }
        self.estimated_payload_bytes = next_total;
        self.items.insert(item.identity.clone(), item);
        Ok(())
    }

    /// Adds one explicit gap, kept outside the evidence list.
    ///
    /// # Errors
    ///
    /// Rejects non-collecting state, capacity overflow and duplicate gap IDs.
    pub fn push_gap(
        &mut self,
        gap: ResearchGap<GapIdentity, GapReason>,
    ) -> Result<(), ExportError> {
        self.require_collecting()?;
        if self.gaps.len() >= self.limits.max_gaps() {
            return Err(ExportError::GapLimitExceeded);
        }
        if self.gaps.insert(gap.identity.clone(), gap).is_some() {
            self.state = BuilderState::Quarantined;
            return Err(ExportError::GapIdentityConflict);
        }
        Ok(())
    }

    /// Finalizes exactly one immutable export.
    ///
    /// # Errors
    ///
    /// Rejects false complete coverage, policy-incompatible incomplete coverage,
    /// required-empty exports and repeated finalization.
    pub fn finalize(
        &mut self,
        coverage: ExportCoverage,
        plan_digest: Blake3Digest32,
    ) -> Result<ResearchExport<Identity, Source, Citation, Payload, GapIdentity, GapReason>, ExportError>
    {
        self.require_collecting()?;
        if coverage == ExportCoverage::Complete && !self.gaps.is_empty() {
            self.state = BuilderState::Quarantined;
            return Err(ExportError::ContradictoryCoverage);
        }
        if self.policy.require_complete_coverage && coverage != ExportCoverage::Complete {
            return Err(ExportError::IncompleteCoverageDenied);
        }
        if self.policy.require_non_empty && self.items.is_empty() {
            return Err(ExportError::EmptyExportDenied);
        }

        self.state = BuilderState::Finalized;
        let items = std::mem::take(&mut self.items);
        let gaps = std::mem::take(&mut self.gaps);
        Ok(ResearchExport {
            manifest: ExportManifest {
                request_id: self.request_id,
                format: self.policy.format,
                coverage,
                item_count: items.len(),
                gap_count: gaps.len(),
                estimated_payload_bytes: self.estimated_payload_bytes,
                plan_digest,
            },
            items,
            gaps,
        })
    }

    /// Quarantines the builder explicitly.
    pub fn quarantine(&mut self) {
        self.state = BuilderState::Quarantined;
    }

    fn require_collecting(&self) -> Result<(), ExportError> {
        match self.state {
            BuilderState::Collecting => Ok(()),
            BuilderState::Finalized => Err(ExportError::AlreadyFinalized),
            BuilderState::Quarantined => Err(ExportError::Quarantined),
        }
    }
}

/// Closed research-export failure registry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExportError {
    /// At least one finite limit is invalid.
    InvalidLimits,
    /// One item is empty or exceeds its byte ceiling.
    ItemTooLarge,
    /// Item capacity was exhausted.
    ItemLimitExceeded,
    /// Gap capacity was exhausted.
    GapLimitExceeded,
    /// Complete export byte accounting overflowed or exceeded its ceiling.
    TotalByteLimitExceeded,
    /// Stable item identity was repeated.
    ItemIdentityConflict,
    /// Stable gap identity was repeated.
    GapIdentityConflict,
    /// Duplicate logical evidence was rejected by policy.
    DuplicateEvidence,
    /// Complete coverage was claimed while explicit gaps exist.
    ContradictoryCoverage,
    /// Policy requires complete authoritative coverage.
    IncompleteCoverageDenied,
    /// Policy requires at least one evidence item.
    EmptyExportDenied,
    /// Builder already produced its terminal export.
    AlreadyFinalized,
    /// Builder is quarantined.
    Quarantined,
}

impl ExportError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "RESEARCH_EXPORT_INVALID_LIMITS",
            Self::ItemTooLarge => "RESEARCH_EXPORT_ITEM_TOO_LARGE",
            Self::ItemLimitExceeded => "RESEARCH_EXPORT_ITEM_LIMIT",
            Self::GapLimitExceeded => "RESEARCH_EXPORT_GAP_LIMIT",
            Self::TotalByteLimitExceeded => "RESEARCH_EXPORT_TOTAL_BYTE_LIMIT",
            Self::ItemIdentityConflict => "RESEARCH_EXPORT_ITEM_IDENTITY_CONFLICT",
            Self::GapIdentityConflict => "RESEARCH_EXPORT_GAP_IDENTITY_CONFLICT",
            Self::DuplicateEvidence => "RESEARCH_EXPORT_DUPLICATE_EVIDENCE",
            Self::ContradictoryCoverage => "RESEARCH_EXPORT_CONTRADICTORY_COVERAGE",
            Self::IncompleteCoverageDenied => "RESEARCH_EXPORT_INCOMPLETE_COVERAGE_DENIED",
            Self::EmptyExportDenied => "RESEARCH_EXPORT_EMPTY_DENIED",
            Self::AlreadyFinalized => "RESEARCH_EXPORT_ALREADY_FINALIZED",
            Self::Quarantined => "RESEARCH_EXPORT_QUARANTINED",
        }
    }
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ExportError {}
