//! Complete effective snapshots assembled from package-validated sections.

use std::collections::BTreeMap;

use search_contracts::ProfileId;

use crate::section::descriptor_matches;

use crate::{
    ConfigError, ConfigFieldDescriptor, ConfigFingerprint, ConfigKeyName, ConfigLimits,
    ConfigOwner, ConfigRegistry, ConfigSectionField, ConfigSectionName, SectionFingerprintInput,
    ValidatedSection, fingerprint,
};

/// One validated section in the authoritative candidate snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveSection {
    owner: ConfigOwner,
    validated: ValidatedSection,
}

impl EffectiveSection {
    /// Registered owner.
    #[must_use]
    pub const fn owner(&self) -> &ConfigOwner {
        &self.owner
    }

    /// Capability-validated section result.
    #[must_use]
    pub const fn validated(&self) -> &ValidatedSection {
        &self.validated
    }

    /// One effective field.
    #[must_use]
    pub fn field(&self, key: &ConfigKeyName) -> Option<&ConfigSectionField> {
        self.validated.field(key)
    }
}

/// Immutable complete effective configuration candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveConfigSnapshot {
    config_schema_version: u32,
    selected_profile: ProfileId,
    sections: BTreeMap<ConfigSectionName, EffectiveSection>,
    fingerprint: ConfigFingerprint,
}

impl EffectiveConfigSnapshot {
    /// Exact accepted configuration schema version.
    #[must_use]
    pub const fn config_schema_version(&self) -> u32 {
        self.config_schema_version
    }

    /// Selected externally authorized profile.
    #[must_use]
    pub const fn selected_profile(&self) -> &ProfileId {
        &self.selected_profile
    }

    /// Exact effective fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> ConfigFingerprint {
        self.fingerprint
    }

    /// One effective section.
    #[must_use]
    pub fn section(&self, name: &ConfigSectionName) -> Option<&EffectiveSection> {
        self.sections.get(name)
    }

    /// Effective sections in canonical name order.
    #[must_use]
    pub fn sections(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ConfigSectionName, &EffectiveSection)> {
        self.sections.iter()
    }

    /// One field descriptor and value by canonical path components.
    #[must_use]
    pub fn field<'a>(
        &'a self,
        registry: &'a ConfigRegistry,
        section_name: &ConfigSectionName,
        key: &ConfigKeyName,
    ) -> Option<(&'a ConfigFieldDescriptor, &'a ConfigSectionField)> {
        let descriptor = registry.section(section_name)?.field(key)?;
        let value = self.section(section_name)?.field(key)?;
        Some((descriptor, value))
    }
}

/// Requires exactly one current package-validated result for every registered section.
///
/// # Errors
///
/// Rejects unknown, missing, duplicate, stale-descriptor, wrong-profile, or
/// invalid-field section results. A partial snapshot is never returned.
pub fn assemble_effective(
    registry: &ConfigRegistry,
    validated_sections: impl IntoIterator<Item = ValidatedSection>,
    selected_profile: ProfileId,
    limits: ConfigLimits,
) -> Result<EffectiveConfigSnapshot, ConfigError> {
    let limits = limits.validate()?;
    let mut sections = BTreeMap::new();
    for validated in validated_sections {
        if sections.len() >= limits.max_sections {
            return Err(ConfigError::CapacityExceeded);
        }
        let descriptor = registry
            .section(validated.section_name())
            .ok_or(ConfigError::UnknownSection)?;
        if validated.selected_profile() != &selected_profile {
            return Err(ConfigError::ProfileNotAuthorized);
        }
        if !descriptor_matches(&validated, descriptor) {
            return Err(ConfigError::StaleDescriptor);
        }
        let name = validated.section_name().clone();
        let owner = descriptor.owner().clone();
        if sections
            .insert(name, EffectiveSection { owner, validated })
            .is_some()
        {
            return Err(ConfigError::DuplicateValidatedSection);
        }
    }

    if sections.len() != registry.len()
        || registry
            .sections()
            .any(|(name, _)| !sections.contains_key(name))
    {
        return Err(ConfigError::MissingSection);
    }

    let fingerprint = fingerprint(
        registry.config_schema_version(),
        &selected_profile,
        sections
            .iter()
            .map(|(name, section)| SectionFingerprintInput {
                section_name: name,
                schema_revision: section.validated.schema_revision(),
                field_registry_digest: section.validated.field_registry_digest(),
                validated_section_digest: section.validated.validated_section_digest(),
            }),
        limits.max_canonical_bytes,
    )?;

    Ok(EffectiveConfigSnapshot {
        config_schema_version: registry.config_schema_version(),
        selected_profile,
        sections,
        fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use search_contracts::{Blake3Digest32, ProfileId};

    use super::assemble_effective;
    use crate::{
        ConfigFieldDescriptor, ConfigKeyName, ConfigLayers, ConfigLimits, ConfigOwner,
        ConfigSectionDescriptor, ConfigSectionName, ConfigSource, ConfigSourceKind,
        ConfigSourceRef, ConfigValue, ConfigValueKind, RedactionPolicy, ReloadClass, SecretPolicy,
        SecurityFloor, ValidatedSection, ValueBounds, merge_layers, project_section,
        register_sections,
    };

    fn fixture() -> (crate::ConfigRegistry, crate::MergedConfig) {
        let field = ConfigFieldDescriptor::new(
            ConfigKeyName::new("enabled", 64).expect("key"),
            ConfigValueKind::Boolean,
            ConfigValue::Boolean(false),
            ValueBounds {
                max_text_bytes: 64,
                max_list_items: 4,
                max_list_item_bytes: 64,
                integer_min: 0,
                integer_max: 10,
            },
            [ConfigSourceKind::File],
            false,
            SecurityFloor::None,
            RedactionPolicy::Public,
            [crate::ReconfigurationAction::ApplyLive],
        )
        .expect("field");
        let section = ConfigSectionDescriptor::new(
            ConfigSectionName::new("query", 64).expect("section"),
            ConfigOwner::new("search-query-planner", 64).expect("owner"),
            NonZeroU64::new(1).expect("revision"),
            ReloadClass::ApplyLive,
            Blake3Digest32::from_bytes([4; 32]),
            SecretPolicy::ForbidPlaintext,
            [field],
            ConfigLimits::W1,
        )
        .expect("section");
        let registry = register_sections(1, [section], ConfigLimits::W1).expect("registry");
        let merged = merge_layers(
            ConfigLayers {
                defaults: ConfigSource {
                    kind: ConfigSourceKind::Defaults,
                    source_ref: ConfigSourceRef::new("defaults", 64).expect("source"),
                    source_digest: Blake3Digest32::from_bytes([1; 32]),
                },
                requested_profile: ProfileId::new("direct").expect("profile"),
                file: None,
                environment: None,
                cli: None,
            },
            &registry,
            ConfigLimits::W1,
        )
        .expect("merged");
        (registry, merged)
    }

    #[test]
    fn missing_section_never_publishes_snapshot() {
        let (registry, _) = fixture();
        assert_eq!(
            assemble_effective(
                &registry,
                [],
                ProfileId::new("direct").expect("profile"),
                ConfigLimits::W1,
            ),
            Err(crate::ConfigError::MissingSection)
        );
    }

    #[test]
    fn same_validated_inputs_produce_same_fingerprint() {
        let (registry, merged) = fixture();
        let descriptor = registry.sections().next().expect("section").1;
        let input = project_section(&merged, descriptor).expect("projection");
        let validated = ValidatedSection::new(
            input,
            ProfileId::new("direct").expect("profile"),
            Blake3Digest32::from_bytes([5; 32]),
        );
        let left = assemble_effective(
            &registry,
            [validated.clone()],
            ProfileId::new("direct").expect("profile"),
            ConfigLimits::W1,
        )
        .expect("snapshot");
        let right = assemble_effective(
            &registry,
            [validated],
            ProfileId::new("direct").expect("profile"),
            ConfigLimits::W1,
        )
        .expect("snapshot");
        assert_eq!(left.fingerprint(), right.fingerprint());
    }
}
