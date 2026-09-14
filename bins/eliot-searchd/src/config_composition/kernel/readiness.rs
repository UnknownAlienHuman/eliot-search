//! Truthful dependency/capability readiness and read-only status projection.

use search_config::{
    ConfigFingerprint, ConfigValue, EffectiveConfigSnapshot, redacted_view,
};

use super::registry::daemon_registry;
use super::snapshot::EffectiveDaemonConfig;
use super::spec::{
    CONTROL_NOT_READY, DAEMON_CONFIG_SCHEMA_VERSION, DIRECT_NOT_READY,
    INDEXED_NOT_ACCEPTED, OPTIONAL_GATE_REQUIRED, QUARANTINED_BLOCKER,
    SEARCH_NOT_ACCEPTED, daemon_limits, key_name, section_name,
};

/// Verified dependency state observed by the daemon composition root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DependencyState {
    /// Owner and quarantine fence.
    pub owner: OwnerFence,
    /// Verified store fence.
    pub stores: StoreVerification,
    /// Live service fence.
    pub services: ServiceFence,
}

/// Owner and quarantine fence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerFence {
    /// Owner guard is held for the canonical root.
    pub runtime_owner_ready: bool,
    /// Persistent quarantine marker is armed.
    pub quarantined: bool,
}

/// Verified store fence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoreVerification {
    /// Control journal or redb mapping verified under the owner guard.
    pub control_store_verified: bool,
    /// Direct store opened and verified under the owner guard.
    pub direct_store_verified: bool,
    /// OS secret backend probed live for the current incarnation.
    pub secret_backend_verified: bool,
}

/// Live service fence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceFence {
    /// Exact qualified Qdrant process and data plane are live.
    pub qdrant_available: bool,
    /// Exact control adapter required for search is constructed.
    pub control_adapter_available: bool,
}

/// Externally accepted receipts. Presence alone never activates a capability.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcceptedReceipts {
    /// General search acceptance.
    pub search_accepted: bool,
    /// Indexed search acceptance with qualified artifacts and routes.
    pub indexed_accepted: bool,
    /// Optional-profile gate acceptance.
    pub optional_gate_accepted: bool,
}

/// Composition readiness fence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionStatus {
    /// Effective configuration is assembled.
    pub configuration_ready: bool,
    /// Owner guard held.
    pub runtime_owner_ready: bool,
    /// Control verified and not quarantined.
    pub control_store_ready: bool,
}

/// Store readiness fence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreStatus {
    /// Secret backend probed live, never `cfg!(windows)`.
    pub secret_store_ready: bool,
    /// Shell endpoint exists.
    pub endpoint_ready: bool,
    /// Direct store verified and not quarantined.
    pub direct_store_ready: bool,
}

/// Capability readiness fence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityStatus {
    /// DIRECT source-backed search over verified immutable revisions.
    pub source_backed_search_available: bool,
    /// General search only with an accepted receipt.
    pub search_available: bool,
    /// Indexed search only with qualified Qdrant, artifacts and routes.
    pub indexed_search_available: bool,
}

/// Truthful readiness derived from effective config, dependencies and receipts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessReport {
    /// Composition fence.
    pub composition: CompositionStatus,
    /// Store fence.
    pub stores: StoreStatus,
    /// Capability fence.
    pub capabilities: CapabilityStatus,
    /// Closed blocker codes, never paths or secrets.
    pub blockers: Vec<&'static str>,
    /// Effective fingerprint for receipts.
    pub fingerprint: ConfigFingerprint,
}

fn boolean_field(
    snapshot: &EffectiveConfigSnapshot,
    section: &str,
    key: &str,
) -> Option<bool> {
    let registry = daemon_registry().ok()?;
    let section_name = section_name(section).ok()?;
    let key_name = key_name(key).ok()?;
    let effective = snapshot.section(&section_name)?.field(&key_name)?;
    registry.section(&section_name)?.field(&key_name)?;
    match &effective.value {
        ConfigValue::Boolean(value) => Some(*value),
        _ => None,
    }
}

/// Derives readiness without feature-presence or health constants.
#[must_use]
pub fn derive_readiness(
    effective: &EffectiveDaemonConfig,
    dependencies: DependencyState,
    accepted: AcceptedReceipts,
) -> ReadinessReport {
    let snapshot = effective.snapshot();
    let control_ready =
        dependencies.stores.control_store_verified && !dependencies.owner.quarantined;
    let direct_ready =
        dependencies.stores.direct_store_verified && !dependencies.owner.quarantined;
    let source_backed = direct_ready && control_ready;
    let optional_semantic =
        boolean_field(snapshot, "optional_profiles", "semantic").unwrap_or(false);
    let optional_blocked = optional_semantic && !accepted.optional_gate_accepted;
    let mut blockers = Vec::new();
    if dependencies.owner.quarantined {
        blockers.push(QUARANTINED_BLOCKER);
    }
    if !dependencies.stores.control_store_verified {
        blockers.push(CONTROL_NOT_READY);
    }
    if !dependencies.stores.direct_store_verified {
        blockers.push(DIRECT_NOT_READY);
    }
    if !accepted.search_accepted {
        blockers.push(SEARCH_NOT_ACCEPTED);
    }
    if !accepted.indexed_accepted || !dependencies.services.qdrant_available {
        blockers.push(INDEXED_NOT_ACCEPTED);
    }
    if !dependencies.services.control_adapter_available {
        blockers.push(CONTROL_NOT_READY);
    }
    if optional_blocked {
        blockers.push(OPTIONAL_GATE_REQUIRED);
    }
    let search_available = accepted.search_accepted
        && source_backed
        && dependencies.services.control_adapter_available
        && !optional_blocked
        && !dependencies.owner.quarantined;
    let indexed_search_available = accepted.indexed_accepted
        && accepted.search_accepted
        && source_backed
        && dependencies.services.qdrant_available
        && dependencies.services.control_adapter_available
        && !optional_blocked
        && !dependencies.owner.quarantined;
    ReadinessReport {
        composition: CompositionStatus {
            configuration_ready: true,
            runtime_owner_ready: dependencies.owner.runtime_owner_ready,
            control_store_ready: control_ready,
        },
        stores: StoreStatus {
            secret_store_ready: dependencies.stores.secret_backend_verified,
            endpoint_ready: true,
            direct_store_ready: direct_ready,
        },
        capabilities: CapabilityStatus {
            source_backed_search_available: source_backed,
            search_available,
            indexed_search_available,
        },
        blockers,
        fingerprint: snapshot.fingerprint(),
    }
}

/// Read-only effective-configuration status without paths or secrets.
#[must_use]
pub fn config_status_json(
    effective: &EffectiveDaemonConfig,
    report: &ReadinessReport,
) -> String {
    let view = redacted_view(
        effective.snapshot(),
        effective.registry(),
        search_config::DisclosureLevel::Ordinary,
        daemon_limits(),
    );
    let fingerprint_hex = crate::sha256::hex(report.fingerprint.as_bytes());
    let mut blockers = String::from("[");
    for (index, blocker) in report.blockers.iter().enumerate() {
        if index > 0 {
            blockers.push(',');
        }
        blockers.push('"');
        blockers.push_str(blocker);
        blockers.push('"');
    }
    blockers.push(']');
    format!(
        concat!(
            "{{\"event\":\"effective_config_status\",",
            "\"schema\":\"eliot.effective-config-status-v1\",",
            "\"config_schema_version\":{},",
            "\"selected_profile\":\"{}\",",
            "\"config_fingerprint\":\"{}\",",
            "\"configuration_ready\":{},",
            "\"runtime_owner_ready\":{},",
            "\"control_store_ready\":{},",
            "\"secret_store_ready\":{},",
            "\"direct_store_ready\":{},",
            "\"source_backed_search_available\":{},",
            "\"search_available\":{},",
            "\"indexed_search_available\":{},",
            "\"blockers\":{},",
            "\"redacted_entries\":{},",
            "\"omitted_entries\":{},",
            "\"read_only\":true}}"
        ),
        DAEMON_CONFIG_SCHEMA_VERSION,
        effective.selected_profile(),
        fingerprint_hex,
        report.composition.configuration_ready,
        report.composition.runtime_owner_ready,
        report.composition.control_store_ready,
        report.stores.secret_store_ready,
        report.stores.direct_store_ready,
        report.capabilities.source_backed_search_available,
        report.capabilities.search_available,
        report.capabilities.indexed_search_available,
        blockers,
        view.entries.len(),
        view.omitted_entries,
    )
}

/// Shell dependency state before a data-root owner is acquired.
#[must_use]
pub const fn shell_dependencies() -> DependencyState {
    DependencyState {
        owner: OwnerFence {
            runtime_owner_ready: false,
            quarantined: false,
        },
        stores: StoreVerification {
            control_store_verified: false,
            direct_store_verified: false,
            secret_backend_verified: false,
        },
        services: ServiceFence {
            qdrant_available: false,
            control_adapter_available: false,
        },
    }
}

/// Direct-store dependency state after owner-fenced open and verification.
#[must_use]
pub const fn direct_dependencies() -> DependencyState {
    DependencyState {
        owner: OwnerFence {
            runtime_owner_ready: true,
            quarantined: false,
        },
        stores: StoreVerification {
            control_store_verified: true,
            direct_store_verified: true,
            secret_backend_verified: false,
        },
        services: ServiceFence {
            qdrant_available: false,
            control_adapter_available: true,
        },
    }
}
