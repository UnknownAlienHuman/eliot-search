//! Vendor-neutral exact-ID compensation compatibility contract.
//!
//! The public names are retained for the existing T27 process harness. This
//! module contains no Qdrant SDK, transport, route, point-payload or async
//! runtime type. Consequently it is not itself a production `RealDataPlane`
//! adapter: a live adapter must bind complete route/payload/context inputs at
//! the Qdrant package boundary and map them into this orchestration contract.

use search_contracts::Epoch;

/// Explicit point ID for exact compensation calls.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompensatePointId(pub [u8; 16]);

/// Immutable exact mutation identity for one compensation call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompensateMutation {
    /// Caller-assigned operation identity; retries reuse it byte-identical.
    pub operation_id: u64,
    /// Digest of the exact canonical mutation input.
    pub input_digest: [u8; 32],
}

/// Exact compensation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompensateReceipt {
    /// Explicit IDs the mutation applied to.
    pub affected: Vec<CompensatePointId>,
    /// Whether the receipt came from idempotent replay.
    pub replayed: bool,
}

/// Exact point readback with explicit present and missing sets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompensateReadback {
    /// Requested IDs found with exact payloads.
    pub present: Vec<CompensatePointId>,
    /// Requested IDs with no stored point.
    pub missing: Vec<CompensatePointId>,
}

/// Closed compensation failure aligned with the bridge reason vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompensateError {
    /// The mutation may have committed; exact readback must resolve it.
    Unknown,
    /// The same identity was reused with different canonical input.
    Conflict,
    /// Readback or pre-dispatch state differs from the exact expectation.
    Mismatch,
}

impl CompensateError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unknown => "QDRANT_MUTATION_OUTCOME_UNKNOWN",
            Self::Conflict => "QDRANT_OPERATION_CONFLICT",
            Self::Mismatch => "QDRANT_EXACT_READBACK_MISMATCH",
        }
    }
}

impl core::fmt::Display for CompensateError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for CompensateError {}

/// Exact-ID compensation port retained for the T27 process harness.
///
/// Only explicit IDs are accepted; broad-filter mutation and physical reclaim
/// do not exist on this surface. Mutations may return [`CompensateError::Unknown`]
/// after possible dispatch. Reads must remain definite.
pub trait QdrantCompensate {
    /// Upserts only the named point identities under one immutable mutation ID.
    fn upsert_exact(
        &mut self,
        ids: Vec<CompensatePointId>,
        mutation: CompensateMutation,
    ) -> Result<CompensateReceipt, CompensateError>;

    /// Sets the exclusive upper epoch on only the named point identities.
    fn close_exact(
        &mut self,
        ids: Vec<CompensatePointId>,
        valid_until: Epoch,
        mutation: CompensateMutation,
    ) -> Result<CompensateReceipt, CompensateError>;

    /// Reads exactly the named identities; absence is typed data.
    fn readback_exact(
        &self,
        ids: Vec<CompensatePointId>,
    ) -> Result<CompensateReadback, CompensateError>;
}
