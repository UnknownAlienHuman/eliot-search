//! Explicit live publication guards and deterministic process-test fixtures.

use search_contracts::{
    Blake3Digest32, OwnerEpoch, PublicationGuards,
};

/// Guards observed at one control generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGuardRead {
    guards: PublicationGuards,
    observed_generation: u64,
}

impl LiveGuardRead {
    /// Binds an explicitly observed guard snapshot to its control generation.
    #[must_use]
    pub const fn new(
        guards: PublicationGuards,
        observed_generation: u64,
    ) -> Self {
        Self {
            guards,
            observed_generation,
        }
    }

    /// Observed guard values.
    #[must_use]
    pub const fn guards(&self) -> PublicationGuards {
        self.guards
    }

    /// Control generation at which the guards were observed.
    #[must_use]
    pub const fn observed_generation(&self) -> u64 {
        self.observed_generation
    }
}

/// Deterministic harness guards with one explicit constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeGuards {
    /// Explicit guard values; tests mutate fields directly for rotations.
    pub guards: PublicationGuards,
}

impl FakeGuards {
    /// Returns the single deterministic process-test guard set.
    #[must_use]
    pub fn fresh() -> Self {
        let owner_epoch =
            OwnerEpoch::new(1).expect("fixed harness owner epoch is non-zero");
        Self {
            guards: PublicationGuards {
                owner_epoch,
                source_catalog_generation: 7,
                membership_generation: 5,
                access_generation: 3,
                shadow_generation: 2,
                purge_generation: 2,
                profile_digest: Blake3Digest32::from_bytes([0xA1; 32]),
            },
        }
    }

    /// Guard values held by this harness.
    #[must_use]
    pub const fn guards(&self) -> PublicationGuards {
        self.guards
    }
}
