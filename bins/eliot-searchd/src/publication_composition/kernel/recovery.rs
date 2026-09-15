//! Durable crash observations and single-head recovery decisions.

/// Durable crash-observation head for one publication reservation.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryHead {
    /// A durable intent marker exists for the reservation.
    pub intent_durable: bool,
    /// The control commit for the reservation was observed.
    pub control_committed: bool,
    /// The committed snapshot was published after the control commit.
    pub snapshot_published: bool,
    /// Exact external readback verified compensation effects.
    pub qdrant_verified: bool,
    /// The journal slot quarantined and denies normal access.
    pub quarantined: bool,
}

/// Single crash-recovery outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryDecision {
    /// No pending work or verified durable work: resume proposing.
    Continue,
    /// Control committed but snapshot publication is incomplete.
    PublishSnapshot,
    /// Durable intent lacks verified exact compensation.
    CompensateExact,
    /// Quarantined state denies every normal path.
    Blocked,
}

impl RecoveryDecision {
    /// Maps one durable observation to exactly one recovery outcome.
    #[must_use]
    pub const fn decide(head: RecoveryHead) -> Self {
        if head.quarantined {
            return Self::Blocked;
        }
        if head.control_committed && !head.snapshot_published {
            return Self::PublishSnapshot;
        }
        if head.control_committed {
            return Self::Continue;
        }
        if head.intent_durable && !head.qdrant_verified {
            return Self::CompensateExact;
        }
        Self::Continue
    }
}
