//! T20 mandatory pre-retrieval admission and live barrier.
//!
//! Query and expansion paths must enter through this module before provider
//! dispatch and before every result/handle/continuation emission. Policy state
//! remains owned by `search-control-redb`; this module performs no persistence.

use std::collections::BTreeSet;

use search_access::{
    AccessCheckpoint, AccessError, LiveSecurityState, PreRetrievalPlan,
    PreRetrievalRequest, RequestSecurityFence, classify_contaminated_legs,
    compile_pre_retrieval, recheck_live_access,
};

/// Live Qdrant scoring/IDF/count parity is deferred to T24, not claimed here.
pub const QDRANT_LIVE_PARITY_DEFERRED_TO: &str = "T24";

/// Maximum legs carried in one admission summary.
pub const MAX_ADMISSION_LEGS: usize = 64;

/// Pre-retrieval admission refused without a provider dispatch.
pub const ACCESS_PRE_RETRIEVAL_DENIED: &str = "ACCESS_PRE_RETRIEVAL_DENIED";
/// Live barrier fired at an emission/expansion checkpoint.
pub const ACCESS_LIVE_BARRIER_DENIED: &str = "ACCESS_LIVE_BARRIER_DENIED";
/// A local token or process identity was offered as authority.
pub const ACCESS_LOCAL_TOKEN_NOT_AUTHORITY: &str = "ACCESS_LOCAL_TOKEN_NOT_AUTHORITY";

/// Typed gate denial: stable daemon reason plus the kernel access code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessGateDenial {
    /// Daemon `ACCESS_*` reason code.
    pub reason: &'static str,
    /// Underlying `search-access` reason code.
    pub access_code: &'static str,
}

/// Content-free admission summary for one gated request.
///
/// Carries counts, generations and predicate digests only: no source bytes,
/// no grant material and no reusable authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreRetrievalAdmission {
    /// Number of admitted safe legs.
    pub leg_count: usize,
    /// Live security generation bound into the admission permit.
    pub live_generation: u64,
    /// Live snapshot digest bound into the admission permit.
    pub snapshot_digest: [u8; 32],
    /// Unified retrieval/IDF predicate digests, one per eligibility plan.
    pub predicate_digests: Vec<[u8; 32]>,
}

/// Admits one query/expand request through the mandatory pre-retrieval gate.
///
/// The kernel compiles grant, policy fence, scope, safe legs and the live
/// barrier in one fixed order; any failure denies before any provider call.
/// Fails closed with [`ACCESS_PRE_RETRIEVAL_DENIED`] on any kernel denial.
pub fn admit_pre_retrieval(
    request: PreRetrievalRequest<'_>,
) -> Result<PreRetrievalAdmission, AccessGateDenial> {
    let plan = compile_pre_retrieval(request).map_err(|error| AccessGateDenial {
        reason: ACCESS_PRE_RETRIEVAL_DENIED,
        access_code: error.code(),
    })?;
    admission_summary(&plan).ok_or(AccessGateDenial {
        reason: ACCESS_PRE_RETRIEVAL_DENIED,
        access_code: AccessError::RetrievalLegBudgetExceeded.code(),
    })
}

/// Rechecks the live barrier immediately before result emission.
///
/// Must be called with the admitted memberships and the current live state;
/// any new denial or purge fails with [`ACCESS_LIVE_BARRIER_DENIED`].
pub fn recheck_before_emission(
    memberships: &BTreeSet<search_contracts::SourceMembershipId>,
    planned_generation: u64,
    live: &LiveSecurityState,
) -> Result<(), AccessGateDenial> {
    recheck_barrier(
        memberships,
        planned_generation,
        live,
        AccessCheckpoint::BeforeResultEmission,
    )
}

/// Rechecks the live barrier before every handle/continuation expansion.
///
/// Expansion reauthorizes live state; handle possession alone admits nothing.
pub fn recheck_before_expansion(
    memberships: &BTreeSet<search_contracts::SourceMembershipId>,
    planned_generation: u64,
    live: &LiveSecurityState,
    continuation: bool,
) -> Result<(), AccessGateDenial> {
    let checkpoint = if continuation {
        AccessCheckpoint::ContinuationExpansion
    } else {
        AccessCheckpoint::HandleExpansion
    };
    recheck_barrier(memberships, planned_generation, live, checkpoint)
}

/// Discards every rank leg influenced by a live fence move.
///
/// Returns the exact contaminated leg IDs for whole-leg discard and replan.
/// An empty set means the in-flight work is unaffected.
#[must_use]
pub fn discard_contaminated_legs(
    execution: &[search_access::LegSecurityPopulation],
    previous: &LiveSecurityState,
    current: &LiveSecurityState,
) -> BTreeSet<usize> {
    match classify_contaminated_legs(execution, previous, current) {
        search_access::ContaminationDecision::Clean => BTreeSet::new(),
        search_access::ContaminationDecision::DiscardLegs(legs) => legs,
    }
}

/// Rejects a local token or process identity offered as authority.
///
/// Authenticating a local token establishes no per-request allowed scope.
/// Always denies; callers must present a server-validated grant instead.
#[must_use]
pub const fn deny_local_token_as_authority() -> AccessGateDenial {
    AccessGateDenial {
        reason: ACCESS_LOCAL_TOKEN_NOT_AUTHORITY,
        access_code: AccessError::LocalIdentityNotAuthority.code(),
    }
}

fn recheck_barrier(
    memberships: &BTreeSet<search_contracts::SourceMembershipId>,
    planned_generation: u64,
    live: &LiveSecurityState,
    checkpoint: AccessCheckpoint,
) -> Result<(), AccessGateDenial> {
    let fence = RequestSecurityFence {
        planned_generation,
        memberships: memberships.clone(),
    };
    recheck_live_access(&fence, live, checkpoint)
        .map(|_| ())
        .map_err(|error| AccessGateDenial {
            reason: ACCESS_LIVE_BARRIER_DENIED,
            access_code: error.code(),
        })
}

fn admission_summary(plan: &PreRetrievalPlan) -> Option<PreRetrievalAdmission> {
    if plan.legs.len() > MAX_ADMISSION_LEGS {
        return None;
    }
    Some(PreRetrievalAdmission {
        leg_count: plan.legs.len(),
        live_generation: plan.permit.live_generation,
        snapshot_digest: *plan.permit.live_snapshot_digest.as_bytes(),
        predicate_digests: plan
            .predicates
            .iter()
            .map(|predicates| predicates.retrieval_digest.0)
            .collect(),
    })
}
