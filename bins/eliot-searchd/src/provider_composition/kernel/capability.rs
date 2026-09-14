//! Readiness-derived provider capability negotiation and operation gating.

use super::spec::{
    MAX_BLOCKERS, PROVIDER_EXPAND_UNAVAILABLE, PROVIDER_INGEST_UNAVAILABLE,
    PROVIDER_QUERY_UNAVAILABLE, ProviderOperation,
};

/// Plain readiness evidence for capability negotiation.
///
/// Built by the daemon adapter from the T12 `ReadinessReport`
/// (`source_backed_search_available`, `search_available`,
/// `indexed_search_available`, `blockers`); tests construct it literally.
/// It carries availability flags and blocker codes only — no paths, secrets
/// or content.
#[derive(Clone, Debug)]
pub struct CapabilityEvidence {
    /// DIRECT source-backed search over verified immutable revisions.
    pub source_backed_search_available: bool,
    /// General search with an accepted receipt.
    pub search_available: bool,
    /// Indexed search with qualified artifacts and routes.
    pub indexed_search_available: bool,
    /// Closed blocker codes, at most [`MAX_BLOCKERS`].
    pub blockers: Vec<&'static str>,
}

impl CapabilityEvidence {
    /// Builds evidence with a bounded blocker list (excess is never silent:
    /// construction fails instead of truncating).
    pub fn from_parts(
        source_backed_search_available: bool,
        search_available: bool,
        indexed_search_available: bool,
        blockers: Vec<&'static str>,
    ) -> Result<Self, &'static str> {
        if blockers.len() > MAX_BLOCKERS {
            return Err("PROVIDER_EVIDENCE_TOO_LARGE");
        }
        Ok(Self {
            source_backed_search_available,
            search_available,
            indexed_search_available,
            blockers,
        })
    }
}

/// Binding-visible capability snapshot for one connection.
///
/// One boolean per closed provider operation by construction; the lint
/// allowance below is intentional (a capability matrix is a bool bag, and
/// collapsing it into bitflags would hide the per-operation mapping).
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProviderCapabilities {
    /// Local health envelope routing.
    pub health_available: bool,
    /// Local status diagnostics.
    pub status_available: bool,
    /// Local version envelope routing.
    pub version_available: bool,
    /// Local shutdown envelope routing.
    pub shutdown_available: bool,
    /// Connection-local cancellation.
    pub cancel_available: bool,
    /// Content admission (needs search acceptance).
    pub ingest_available: bool,
    /// Recipe query (needs search acceptance).
    pub query_available: bool,
    /// Handle expansion, gated by search acceptance.
    pub expand_available: bool,
    /// Indexed search, gated by indexed acceptance plus qualified routes.
    pub indexed_available: bool,
    /// Closed blocker codes explaining every unavailable capability.
    pub blockers: Vec<&'static str>,
}

/// Negotiates binding-visible capabilities from T12 readiness evidence.
///
/// Local shell operations are always available; recipes follow the accepted
/// receipts. Availability grants no authority — it only decides between
/// routing and explicit unavailable.
#[must_use]
pub fn negotiate_capabilities(evidence: &CapabilityEvidence) -> ProviderCapabilities {
    let recipes_available = evidence.search_available && evidence.source_backed_search_available;
    ProviderCapabilities {
        health_available: true,
        status_available: true,
        version_available: true,
        shutdown_available: true,
        cancel_available: true,
        ingest_available: recipes_available,
        query_available: recipes_available,
        expand_available: recipes_available,
        indexed_available: evidence.indexed_search_available
            && evidence.source_backed_search_available,
        blockers: evidence.blockers.clone(),
    }
}

/// Denial for a gated operation: a typed reason plus the exact blockers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDenial {
    /// Stable `PROVIDER_*_UNAVAILABLE` reason code.
    pub reason: &'static str,
    /// Closed blocker codes from the capability evidence.
    pub blockers: Vec<&'static str>,
}

/// Gates one operation against negotiated capabilities.
///
/// Envelope-only operations pass here (their admission is envelope-driven);
/// recipes fail with explicit unavailable instead of empty success.
pub fn gate_operation(
    operation: ProviderOperation,
    capabilities: &ProviderCapabilities,
) -> Result<(), ProviderDenial> {
    let deny = |reason: &'static str| {
        Err(ProviderDenial {
            reason,
            blockers: capabilities.blockers.clone(),
        })
    };
    match operation {
        ProviderOperation::Health
        | ProviderOperation::Status
        | ProviderOperation::Version
        | ProviderOperation::Cancel
        | ProviderOperation::Shutdown => Ok(()),
        ProviderOperation::Ingest if capabilities.ingest_available => Ok(()),
        ProviderOperation::Ingest => deny(PROVIDER_INGEST_UNAVAILABLE),
        ProviderOperation::Query if capabilities.query_available => Ok(()),
        ProviderOperation::Query => deny(PROVIDER_QUERY_UNAVAILABLE),
        ProviderOperation::Expand if capabilities.expand_available => Ok(()),
        ProviderOperation::Expand => deny(PROVIDER_EXPAND_UNAVAILABLE),
    }
}
