//! Public materialization product models and content-free receipts.

use crate::assurance::MaterializationAssurance;
use crate::maps::MapBundle;
use crate::normalize::CanonicalRepresentation;
use crate::profile::{MaterializerProfileId, SourceEncoding, ValidatedMaterializerProfile};
use crate::request::{CancellationToken, MaterializationBudget};
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};

/// Materialization execution context: bound profile, finite budgets and
/// cooperative cancellation.
#[derive(Clone, Debug)]
pub struct MaterializationContext<'a> {
    /// Bound validated profile (must match the validated request).
    pub profile: ValidatedMaterializerProfile,
    /// Finite operation budgets.
    pub budget: MaterializationBudget,
    /// Cooperative cancellation token.
    pub cancel: CancellationToken<'a>,
}

/// Content-free surfaced warning. Warnings always accompany loss records;
/// loss without warnings is an assurance violation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterializationWarning {
    /// A leading byte-order mark was stripped and recorded.
    BomStripped,
    /// Bytes were transcoded exactly from the named encoding.
    Transcoded(SourceEncoding),
    /// A counted number of line terminators was normalized and recorded.
    NewlinesNormalized(u64),
}

impl MaterializationWarning {
    /// Stable short name used in receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BomStripped => "bom-stripped",
            Self::Transcoded(_) => "transcoded",
            Self::NewlinesNormalized(_) => "newlines-normalized",
        }
    }
}

/// Content-free resource and operation receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceReceipt {
    /// Exact input bytes consumed.
    pub input_bytes: u64,
    /// Exact canonical output bytes produced.
    pub output_bytes: u64,
    /// Elementary decode/normalize/map steps consumed.
    pub steps_used: u64,
    /// Coordinate segments produced.
    pub segments: u64,
    /// Loss records produced.
    pub loss_records: u64,
}

/// Immutable materialization product with deterministic identity.
#[derive(Clone, Eq, PartialEq)]
pub struct MaterializationProduct {
    pub(super) representation_id: Blake3Digest32,
    pub(super) source_id: OpaqueId,
    pub(super) revision: NonZeroRevision,
    pub(super) profile_id: MaterializerProfileId,
    pub(super) encoding: SourceEncoding,
    pub(super) input_digest: Blake3Digest32,
    pub(super) canonical_digest: Blake3Digest32,
    pub(super) coordinate_digest: Blake3Digest32,
    pub(super) loss_digest: Blake3Digest32,
    pub(super) canonical: CanonicalRepresentation,
    pub(super) maps: MapBundle,
    pub(super) assurance: MaterializationAssurance,
    pub(super) warnings: Vec<MaterializationWarning>,
    pub(super) resource: ResourceReceipt,
}

impl MaterializationProduct {
    /// Deterministic identity over revision, profile, encoding, bytes and maps.
    #[must_use]
    pub const fn representation_id(&self) -> Blake3Digest32 {
        self.representation_id
    }

    /// Materialized source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Materialized retained revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Bound profile identity.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }

    /// Decided source encoding.
    #[must_use]
    pub const fn encoding(&self) -> SourceEncoding {
        self.encoding
    }

    /// Exact input digest bound from the validated request.
    #[must_use]
    pub const fn input_digest(&self) -> Blake3Digest32 {
        self.input_digest
    }

    /// Digest over the canonical text bytes.
    #[must_use]
    pub const fn canonical_digest(&self) -> Blake3Digest32 {
        self.canonical_digest
    }

    /// Digest over the serialized coordinate segments.
    #[must_use]
    pub const fn coordinate_digest(&self) -> Blake3Digest32 {
        self.coordinate_digest
    }

    /// Digest over the serialized loss records.
    #[must_use]
    pub const fn loss_digest(&self) -> Blake3Digest32 {
        self.loss_digest
    }

    /// Canonical text.
    #[must_use]
    pub fn canonical_text(&self) -> &str {
        self.canonical.text()
    }

    /// Canonical representation with its offset-change record.
    #[must_use]
    pub const fn canonical(&self) -> &CanonicalRepresentation {
        &self.canonical
    }

    /// Coordinate and loss maps.
    #[must_use]
    pub const fn maps(&self) -> &MapBundle {
        &self.maps
    }

    /// Derived assurance bound.
    #[must_use]
    pub const fn assurance(&self) -> MaterializationAssurance {
        self.assurance
    }

    /// Surfaced content-free warnings.
    #[must_use]
    pub fn warnings(&self) -> &[MaterializationWarning] {
        &self.warnings
    }

    /// Content-free resource receipt.
    #[must_use]
    pub const fn resource_receipt(&self) -> ResourceReceipt {
        self.resource
    }
}

impl core::fmt::Debug for MaterializationProduct {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MaterializationProduct")
            .field("representation_id", &self.representation_id)
            .field("source_id", &self.source_id)
            .field("revision", &self.revision)
            .field("profile_id", &self.profile_id)
            .field("encoding", &self.encoding)
            .field(
                "canonical",
                &format_args!("<{} bytes>", self.resource.output_bytes),
            )
            .field("assurance", &self.assurance)
            .field("warnings", &self.warnings)
            .field("resource", &self.resource)
            .finish_non_exhaustive()
    }
}

/// Deterministic canonical bytes for descriptors, maps and warnings.
///
/// Source content travels by digest only: technical receipts never embed
/// content or paths.
#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalMaterializationBytes {
    pub(super) bytes: Vec<u8>,
}

impl CanonicalMaterializationBytes {
    /// Canonical bytes borrowed without copying.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Canonical byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Reports whether the canonical bytes are empty (never for valid output).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl core::fmt::Debug for CanonicalMaterializationBytes {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CanonicalMaterializationBytes")
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .finish()
    }
}

/// Content-free verification receipt: the output belongs to the exact source
/// revision and profile. It cannot prove current filesystem state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationVerificationReceipt {
    pub(super) representation_id: Blake3Digest32,
    pub(super) profile_id: MaterializerProfileId,
    pub(super) coordinate_segments: u64,
    pub(super) loss_records: u64,
    pub(super) assurance: MaterializationAssurance,
}

impl MaterializationVerificationReceipt {
    /// Verified representation identity.
    #[must_use]
    pub const fn representation_id(&self) -> Blake3Digest32 {
        self.representation_id
    }

    /// Verified profile identity.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }

    /// Validated coordinate segment count.
    #[must_use]
    pub const fn coordinate_segments(&self) -> u64 {
        self.coordinate_segments
    }

    /// Validated loss record count.
    #[must_use]
    pub const fn loss_records(&self) -> u64 {
        self.loss_records
    }

    /// Verified assurance bound.
    #[must_use]
    pub const fn assurance(&self) -> MaterializationAssurance {
        self.assurance
    }
}

/// Content-addressed immutable publication plan.
///
/// Prepared for the revision/preparation owner. This plan does not write CAS
/// or control state and does not claim admission; unknown artifact-write
/// outcomes remain the caller's operation and readback responsibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationAdmissionPlan {
    pub(super) representation_id: Blake3Digest32,
    pub(super) canonical_digest: Blake3Digest32,
    pub(super) profile_id: MaterializerProfileId,
    pub(super) canonical_bytes_len: u64,
    pub(super) operation_id: OpaqueId,
}

impl MaterializationAdmissionPlan {
    /// Planned representation identity.
    #[must_use]
    pub const fn representation_id(&self) -> Blake3Digest32 {
        self.representation_id
    }

    /// Planned canonical content digest.
    #[must_use]
    pub const fn canonical_digest(&self) -> Blake3Digest32 {
        self.canonical_digest
    }

    /// Planned profile identity.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }

    /// Planned canonical byte length.
    #[must_use]
    pub const fn canonical_bytes_len(&self) -> u64 {
        self.canonical_bytes_len
    }

    /// Owning operation identity.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }
}
