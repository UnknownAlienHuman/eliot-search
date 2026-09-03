//! Reload classes, independent obligations, and receipt requirements.

/// Ordered minimum reload class declared by a section.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReloadClass {
    /// No material action.
    Noop,
    /// Apply a bounded live mutation.
    ApplyLive,
    /// Install a restrictive barrier before acknowledgement.
    SecurityBarrier,
    /// Restart one dependency owner.
    RestartDependency,
    /// Drain accepted work and restart the owning process.
    DrainAndRestart,
    /// Publish a new collection generation.
    NewCollectionGeneration,
    /// Rebuild derived projections.
    RebuildProjection,
    /// Re-run an explicit acceptance gate.
    GateRequired,
    /// Reject the candidate configuration.
    Reject,
}

/// Independent reconfiguration obligation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReconfigurationAction {
    /// Apply a bounded live mutation.
    ApplyLive,
    /// Install a restrictive security/lifecycle barrier.
    SecurityBarrier,
    /// Restart a dependency owner.
    RestartDependency,
    /// Drain accepted work and restart the owner.
    DrainAndRestart,
    /// Create and validate a new collection generation.
    NewCollectionGeneration,
    /// Rebuild derived projections.
    RebuildProjection,
    /// Execute an explicit control-schema migration.
    MigrateControlSchema,
    /// Re-run an acceptance gate.
    GateRequired,
    /// Reject activation.
    Reject,
}

impl ReloadClass {
    /// Converts the minimum class to its independent obligation.
    #[must_use]
    pub const fn action(self) -> Option<ReconfigurationAction> {
        match self {
            Self::Noop => None,
            Self::ApplyLive => Some(ReconfigurationAction::ApplyLive),
            Self::SecurityBarrier => Some(ReconfigurationAction::SecurityBarrier),
            Self::RestartDependency => Some(ReconfigurationAction::RestartDependency),
            Self::DrainAndRestart => Some(ReconfigurationAction::DrainAndRestart),
            Self::NewCollectionGeneration => Some(ReconfigurationAction::NewCollectionGeneration),
            Self::RebuildProjection => Some(ReconfigurationAction::RebuildProjection),
            Self::GateRequired => Some(ReconfigurationAction::GateRequired),
            Self::Reject => Some(ReconfigurationAction::Reject),
        }
    }
}

/// Receipt class required before a candidate fingerprint may become authoritative.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReceiptKind {
    /// Live application readback.
    LiveApply,
    /// Restrictive barrier installation.
    SecurityBarrier,
    /// Dependency restart and readiness.
    DependencyRestart,
    /// Drain and process restart.
    ProcessRestart,
    /// Collection generation publication.
    CollectionGeneration,
    /// Projection rebuild validation.
    ProjectionRebuild,
    /// Control-schema migration commit and readback.
    ControlMigration,
    /// Gate acceptance.
    GateAcceptance,
}

impl ReconfigurationAction {
    /// Receipt required by this obligation, when execution is possible.
    #[must_use]
    pub const fn receipt(self) -> Option<ReceiptKind> {
        match self {
            Self::ApplyLive => Some(ReceiptKind::LiveApply),
            Self::SecurityBarrier => Some(ReceiptKind::SecurityBarrier),
            Self::RestartDependency => Some(ReceiptKind::DependencyRestart),
            Self::DrainAndRestart => Some(ReceiptKind::ProcessRestart),
            Self::NewCollectionGeneration => Some(ReceiptKind::CollectionGeneration),
            Self::RebuildProjection => Some(ReceiptKind::ProjectionRebuild),
            Self::MigrateControlSchema => Some(ReceiptKind::ControlMigration),
            Self::GateRequired => Some(ReceiptKind::GateAcceptance),
            Self::Reject => None,
        }
    }

    /// Deterministic execution order. Independent obligations remain distinct.
    #[must_use]
    pub const fn order(self) -> u8 {
        match self {
            Self::SecurityBarrier => 0,
            Self::ApplyLive => 1,
            Self::RestartDependency => 2,
            Self::DrainAndRestart => 3,
            Self::MigrateControlSchema => 4,
            Self::NewCollectionGeneration => 5,
            Self::RebuildProjection => 6,
            Self::GateRequired => 7,
            Self::Reject => 8,
        }
    }
}
