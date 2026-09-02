//! Shared access-checkpoint support types.

/// Security/currentness checkpoint in the query pipeline.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessCheckpoint {
    /// Before any candidate, count, IDF, or trace influence.
    BeforeInfluence,
    /// Before exact retained-revision readback.
    BeforeReadback,
    /// Immediately before result emission.
    BeforeEmission,
    /// Immediately before handle or continuation expansion.
    BeforeExpansion,
}

/// Closed permit classification returned by access revalidation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SecurityPermitState {
    /// Current exact state permits the operation.
    Permit,
    /// Current exact state denies the operation.
    Deny,
    /// State changed after earlier influence and the leg is contaminated.
    Contaminated,
    /// Authoritative state cannot be established.
    Unknown,
}
