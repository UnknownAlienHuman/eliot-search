//! Truthful development-shell readiness and capability projection.

/// Composition-prerequisite readiness for one daemon state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionReadiness {
    pub(crate) configuration_ready: bool,
    pub(crate) runtime_owner_ready: bool,
    pub(crate) control_store_ready: bool,
}

/// Store and channel readiness for one daemon state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreReadiness {
    pub(crate) secret_store_ready: bool,
    pub(crate) endpoint_ready: bool,
    pub(crate) direct_store_ready: bool,
}

/// Capability availability for one daemon state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityAvailability {
    pub(crate) source_backed_search_available: bool,
    /// General search requires an accepted receipt; W1 readiness never
    /// implies it. Composed via `config_composition::derive_readiness`.
    pub(crate) search_available: bool,
    /// Indexed search requires qualified Qdrant, artifacts, and routes plus
    /// accepted receipts; always false without them.
    pub(crate) indexed_search_available: bool,
}

impl CapabilityAvailability {
    /// One-shot stdin scans remain available on every daemon state.
    pub(crate) const fn development_stdin_scan_available(self) -> bool {
        let _ = self;
        true
    }

    /// One-shot file scans remain available on every daemon state.
    pub(crate) const fn development_file_scan_available(self) -> bool {
        let _ = self;
        true
    }
}

/// Truthful capability summary for one daemon composition state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Health {
    pub(crate) composition: CompositionReadiness,
    pub(crate) stores: StoreReadiness,
    pub(crate) capabilities: CapabilityAvailability,
}

impl Health {
    /// Process shell before a data root is opened.
    pub(crate) const SHELL: Self = Self {
        composition: CompositionReadiness {
            configuration_ready: true,
            runtime_owner_ready: false,
            control_store_ready: false,
        },
        stores: StoreReadiness {
            secret_store_ready: false,
            endpoint_ready: true,
            direct_store_ready: false,
        },
        capabilities: CapabilityAvailability {
            source_backed_search_available: false,
            search_available: false,
            indexed_search_available: false,
        },
    };

    /// Derives truthful health from an effective-configuration readiness
    /// report. One-shot development scans remain available; every other
    /// capability comes from the report, never from constants or `cfg!`.
    pub(crate) const fn from_readiness(report: &crate::config_composition::ReadinessReport) -> Self {
        Self {
            composition: CompositionReadiness {
                configuration_ready: report.composition.configuration_ready,
                runtime_owner_ready: report.composition.runtime_owner_ready,
                control_store_ready: report.composition.control_store_ready,
            },
            stores: StoreReadiness {
                secret_store_ready: report.stores.secret_store_ready,
                endpoint_ready: report.stores.endpoint_ready,
                direct_store_ready: report.stores.direct_store_ready,
            },
            capabilities: CapabilityAvailability {
                source_backed_search_available: report
                    .capabilities
                    .source_backed_search_available,
                search_available: report.capabilities.search_available,
                indexed_search_available: report.capabilities.indexed_search_available,
            },
        }
    }

    pub(crate) fn json(self) -> String {
        format!(
            concat!(
                "{{\"status\":\"development_shell\",",
                "\"configuration_ready\":{},",
                "\"runtime_owner_ready\":{},",
                "\"control_store_ready\":{},",
                "\"secret_store_ready\":{},",
                "\"endpoint_ready\":{},",
                "\"direct_store_ready\":{},",
                "\"source_backed_search_available\":{},",
                "\"development_stdin_scan_available\":{},",
                "\"development_file_scan_available\":{},",
                "\"search_available\":{},",
                "\"indexed_search_available\":{}}}"
            ),
            self.composition.configuration_ready,
            self.composition.runtime_owner_ready,
            self.composition.control_store_ready,
            self.stores.secret_store_ready,
            self.stores.endpoint_ready,
            self.stores.direct_store_ready,
            self.capabilities.source_backed_search_available,
            self.capabilities.development_stdin_scan_available(),
            self.capabilities.development_file_scan_available(),
            self.capabilities.search_available,
            self.capabilities.indexed_search_available,
        )
    }
}
