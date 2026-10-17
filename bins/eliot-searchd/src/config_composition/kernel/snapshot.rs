//! Effective snapshot assembly over pure `search-config` mechanics.

use search_config::{
    ConfigDocument, ConfigError, ConfigFingerprint, ConfigLayers, ConfigRegistry,
    EffectiveConfigSnapshot, ValidatedSection, assemble_effective, merge_layers, project_section,
};
use search_contracts::ProfileId;
use search_handles::ValidatedHandlesConfig;
use search_os_secrets::ValidatedSecretsConfig;

use super::registry::daemon_registry;
use super::spec::{DAEMON_DIRECT_PROFILE, daemon_limits, defaults_source, profile_id};

/// Immutable daemon-effective configuration: the authoritative snapshot plus
/// the closed registry that produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveDaemonConfig {
    snapshot: EffectiveConfigSnapshot,
    registry: ConfigRegistry,
    secrets_settings: ValidatedSecretsConfig,
    handles_settings: ValidatedHandlesConfig,
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

    /// Owner-checked immutable settings; no runtime application or key authority.
    #[must_use]
    pub const fn secrets_settings(&self) -> &ValidatedSecretsConfig {
        &self.secrets_settings
    }

    /// Owner-validated settings selected from this same effective snapshot.
    #[must_use]
    pub const fn handles_settings(&self) -> &ValidatedHandlesConfig {
        &self.handles_settings
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
    let mut secrets_settings = None;
    let mut handles_settings = None;
    for (_, descriptor) in registry.sections() {
        let input = project_section(&merged, descriptor)?;
        let section = ValidatedSection::new(input, selected.clone(), daemon_limits())?;
        match section.section_name().as_str() {
            "secrets" => {
                secrets_settings = Some(search_os_secrets::validate_secrets_section(&section)?);
            }
            "handles" => {
                handles_settings = Some(search_handles::validate_handles_section(&section)?);
            }
            _ => {}
        }
        validated.push(section);
    }
    let secrets_settings = secrets_settings.ok_or(ConfigError::MissingSection)?;
    let handles_settings = handles_settings.ok_or(ConfigError::MissingSection)?;
    let snapshot = assemble_effective(&registry, validated, selected, daemon_limits())?;
    Ok(EffectiveDaemonConfig {
        snapshot,
        registry,
        secrets_settings,
        handles_settings,
    })
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

#[cfg(test)]
#[path = "configtests.rs"]
mod configtests;
