//! Complete abandonment fences and logical exact-ID retirement records.

use super::compensation::CompensatePointId;
use super::spec::{MAX_RETIRED_IDS, PublisherError};

/// Complete membership fence required to abandon an active commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MembershipFence {
    total: u64,
    fenced: u64,
}

impl MembershipFence {
    /// Builds a complete fence over `total` memberships.
    pub const fn full(total: u64) -> Result<Self, PublisherError> {
        if total == 0 {
            return Err(PublisherError::BudgetExceeded);
        }
        Ok(Self {
            total,
            fenced: total,
        })
    }

    /// Builds a possibly partial fence.
    pub const fn partial(
        fenced: u64,
        total: u64,
    ) -> Result<Self, PublisherError> {
        if total == 0 || fenced > total {
            return Err(PublisherError::BudgetExceeded);
        }
        Ok(Self { total, fenced })
    }

    /// Whether every membership is fenced.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.total > 0 && self.fenced == self.total
    }
}

/// Logical retirement record containing exact point identities.
///
/// This record never deletes or physically reclaims data. A separate exact-ID
/// reclaimer owns that effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetiredManifest {
    ids: Vec<CompensatePointId>,
}

impl RetiredManifest {
    /// Records a finite, non-empty exact retirement set.
    pub fn new(
        ids: Vec<CompensatePointId>,
    ) -> Result<Self, PublisherError> {
        if ids.is_empty() || ids.len() > MAX_RETIRED_IDS {
            return Err(PublisherError::BudgetExceeded);
        }
        Ok(Self { ids })
    }

    /// Exact retired point identities.
    #[must_use]
    pub fn ids(&self) -> &[CompensatePointId] {
        &self.ids
    }
}
