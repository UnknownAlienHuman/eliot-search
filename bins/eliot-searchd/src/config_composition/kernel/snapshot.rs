//! Effective snapshot assembly over pure `search-config` mechanics.

use search_config::{
    ConfigDocument, ConfigError, ConfigFingerprint, ConfigLayers,
    ConfigRegistry, EffectiveConfigSnapshot, ValidatedSection,
    assemble_effective, merge_layers, project_section,
};
use search_contracts::ProfileId;

use super::capture::validation_digest;
use super::registry::daemon_registry;
use super::spec::{
    DAEMON_DIRECT_PROFILE, daemon_limits, defaults_source, profile_id,
};

/// Immutable daemon-effective configuration: the authoritative snapshot plus
/// the closed registry that produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveDaemonConfig {
    snapshot: EffectiveConfigSnapshot,
    registry: ConfigRegistry,
}

impl EffectiveDaemonConfig {
    /// Authoritative effective snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &EffectiveConfigSnapshot {
        &self.snapshot
    }

    /// Closed registry that produced the snapshot.
    #[must_use]
    pub const fn registry(&self) -> &ConfigRegistry {
        &self.registry
    }

    /// Exact effective fingerprint for persistence and receipts.
    #[must_use]
    pub const fn fingerprint(&self) -> ConfigFingerprint {
        self.snapshot.fingerprint()
    }

    /// Externally selected profile bound into the snapshot.
    #[must_use]
    pub fn selected_profile(&self) -> ProfileId {
        self.snapshot.selected_profile().clone()
    }
}

/// Assembles the daemon-effective snapshot from defaults and already-captured
/// file, environment and CLI layers. No partial snapshot escapes.
pub fn build_effective(
    file: Option<ConfigDocument>,
    environment: Option<ConfigDocument>,
    cli: Option<ConfigDocument>,
    requested_profile: &str,
    selected_profile: &str,
) -> Result<EffectiveDaemonConfig, ConfigError> {
    let registry = daemon_registry()?;
    let requested = profile_id(requested_profile)?;
    let selected = profile_id(selected_profile)?;
    let merged = merge_layers(
        ConfigLayers {
            defaults: defaults_source()?,
            requested_profile: requested,
            file,
            environment,
            cli,
        },
        &registry,
        daemon_limits(),
    )?;
    let mut validated = Vec::new();
    for (_, descriptor) in registry.sections() {
        let input = project_section(&merged, descriptor)?;
        let digest = validation_digest(&input);
        validated.push(ValidatedSection::new(input, selected.clone(), digest));
    }
    let snapshot =
        assemble_effective(&registry, validated, selected, daemon_limits())?;
    Ok(EffectiveDaemonConfig { snapshot, registry })
}

/// Defaults-only effective snapshot for the W1 shell profile.
pub fn build_effective_defaults() -> Result<EffectiveDaemonConfig, ConfigError> {
    build_effective(
        None,
        None,
        None,
        DAEMON_DIRECT_PROFILE,
        DAEMON_DIRECT_PROFILE,
    )
}
