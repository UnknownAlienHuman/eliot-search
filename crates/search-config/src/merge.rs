//! Deterministic field-level layer composition with provenance.

use std::collections::BTreeMap;

use search_contracts::ProfileId;

use crate::{
    ConfigDocument, ConfigError, ConfigKeyPath, ConfigLimits, ConfigRegistry, ConfigSource,
    ConfigSourceKind, ConfigValue, LayerOperation,
};

/// Exact source and reset provenance for one effective field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldProvenance {
    /// Source that supplied the effective value.
    pub source: ConfigSource,
    /// Whether this source explicitly reset the value to its compiled default.
    pub explicit_reset: bool,
}

/// One merged typed field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergedField {
    /// Validated typed value.
    pub value: ConfigValue,
    /// Exact winning source provenance.
    pub provenance: FieldProvenance,
}

/// Fixed precedence inputs for one merge operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigLayers {
    /// Synthetic identity of capability-owned compiled defaults.
    pub defaults: ConfigSource,
    /// Requested composition ceiling after external gate intersection.
    pub requested_profile: ProfileId,
    /// Optional captured local file.
    pub file: Option<ConfigDocument>,
    /// Optional captured whitelisted environment layer.
    pub environment: Option<ConfigDocument>,
    /// Optional captured typed CLI layer.
    pub cli: Option<ConfigDocument>,
}

/// Complete deterministic merge output before package-owned section validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergedConfig {
    config_schema_version: u32,
    requested_profile: ProfileId,
    fields: BTreeMap<ConfigKeyPath, MergedField>,
}

impl MergedConfig {
    /// Exact configuration schema version.
    #[must_use]
    pub const fn config_schema_version(&self) -> u32 {
        self.config_schema_version
    }

    /// Requested composition ceiling.
    #[must_use]
    pub const fn requested_profile(&self) -> &ProfileId {
        &self.requested_profile
    }

    /// Exact effective field.
    #[must_use]
    pub fn field(&self, path: &ConfigKeyPath) -> Option<&MergedField> {
        self.fields.get(path)
    }

    /// Effective fields in canonical path order.
    #[must_use]
    pub fn fields(&self) -> impl ExactSizeIterator<Item = (&ConfigKeyPath, &MergedField)> {
        self.fields.iter()
    }

    /// Number of effective fields.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns whether no effective field exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// Applies `defaults < file < environment < CLI` under exact field allowlists.
///
/// # Errors
///
/// Rejects source-kind mismatches, version/profile mismatches, unknown sections
/// or keys, forbidden overrides/resets, type/bound failures, plaintext secret
/// attempts, and directional security-floor weakening. No partial result escapes.
pub fn merge_layers(
    layers: ConfigLayers,
    registry: &ConfigRegistry,
    limits: ConfigLimits,
) -> Result<MergedConfig, ConfigError> {
    let limits = limits.validate()?;
    if layers.defaults.kind != ConfigSourceKind::Defaults {
        return Err(ConfigError::SourceKindMismatch);
    }

    let mut fields = BTreeMap::new();
    for (section_name, section) in registry.sections() {
        for (key, descriptor) in section.fields() {
            if fields.len()
                >= limits
                    .max_sections
                    .saturating_mul(limits.max_fields_per_section)
            {
                return Err(ConfigError::CapacityExceeded);
            }
            fields.insert(
                ConfigKeyPath::new(section_name.clone(), key.clone()),
                MergedField {
                    value: descriptor.default().clone(),
                    provenance: FieldProvenance {
                        source: layers.defaults.clone(),
                        explicit_reset: false,
                    },
                },
            );
        }
    }

    let documents = [
        (ConfigSourceKind::File, layers.file.as_ref()),
        (ConfigSourceKind::Environment, layers.environment.as_ref()),
        (ConfigSourceKind::Cli, layers.cli.as_ref()),
    ];
    for (expected_kind, document) in documents {
        let Some(document) = document else {
            continue;
        };
        validate_document_identity(document, expected_kind, registry, &layers.requested_profile)?;
        apply_document(document, registry, &mut fields)?;
    }

    Ok(MergedConfig {
        config_schema_version: registry.config_schema_version(),
        requested_profile: layers.requested_profile,
        fields,
    })
}

fn validate_document_identity(
    document: &ConfigDocument,
    expected_kind: ConfigSourceKind,
    registry: &ConfigRegistry,
    requested_profile: &ProfileId,
) -> Result<(), ConfigError> {
    if document.source().kind != expected_kind {
        return Err(ConfigError::SourceKindMismatch);
    }
    if document.schema_version() != registry.config_schema_version() {
        return Err(ConfigError::UnsupportedSchemaVersion);
    }
    match (expected_kind, document.requested_profile()) {
        (ConfigSourceKind::File, Some(profile)) if profile == requested_profile => {}
        (ConfigSourceKind::File, _) => return Err(ConfigError::ProfileNotAuthorized),
        (_, None) => {}
        (_, Some(_)) => return Err(ConfigError::SourceKindMismatch),
    }
    for section in document.declared_sections() {
        if registry.section(section).is_none() {
            return Err(ConfigError::UnknownSection);
        }
    }
    Ok(())
}

fn apply_document(
    document: &ConfigDocument,
    registry: &ConfigRegistry,
    fields: &mut BTreeMap<ConfigKeyPath, MergedField>,
) -> Result<(), ConfigError> {
    for (path, operation) in document.entries() {
        let section = registry
            .section(path.section())
            .ok_or(ConfigError::UnknownSection)?;
        let descriptor = section.field(path.key()).ok_or(ConfigError::UnknownKey)?;
        let source_kind = document.source().kind;
        let current = fields.get(path).ok_or(ConfigError::UnknownKey)?;
        let (candidate, explicit_reset) = match operation {
            LayerOperation::Set(value) => (descriptor.validate_document_value(value)?, false),
            LayerOperation::Reset => {
                if !descriptor.reset_allowed() {
                    return Err(ConfigError::ResetNotAllowed);
                }
                (descriptor.default().clone(), true)
            }
        };
        if !descriptor.allows_source(source_kind) {
            return Err(ConfigError::OverrideNotAllowed);
        }
        descriptor.validate_security_floor(&current.value, &candidate)?;
        fields.insert(
            path.clone(),
            MergedField {
                value: candidate,
                provenance: FieldProvenance {
                    source: document.source().clone(),
                    explicit_reset,
                },
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use search_contracts::{Blake3Digest32, ProfileId};

    use super::{ConfigLayers, merge_layers};
    use crate::{
        ConfigDocument, ConfigError, ConfigFieldDescriptor, ConfigKeyName, ConfigKeyPath,
        ConfigLimits, ConfigOwner, ConfigSectionDescriptor, ConfigSectionName, ConfigSource,
        ConfigSourceKind, ConfigSourceRef, ConfigValue, ConfigValueKind, DocumentValue,
        LayerOperation, RedactionPolicy, ReloadClass, SecretPolicy, SecurityFloor, ValueBounds,
        register_sections,
    };

    fn source(kind: ConfigSourceKind, marker: u8) -> ConfigSource {
        ConfigSource {
            kind,
            source_ref: ConfigSourceRef::new(format!("source-{marker}"), 64).expect("source"),
            source_digest: Blake3Digest32::from_bytes([marker; 32]),
        }
    }

    fn registry(allow_environment: bool) -> crate::ConfigRegistry {
        let mut sources = vec![ConfigSourceKind::File, ConfigSourceKind::Cli];
        if allow_environment {
            sources.push(ConfigSourceKind::Environment);
        }
        let field = ConfigFieldDescriptor::new(
            ConfigKeyName::new("limit", 64).expect("key"),
            ConfigValueKind::Integer,
            ConfigValue::Integer(1),
            ValueBounds {
                max_text_bytes: 64,
                max_list_items: 4,
                max_list_item_bytes: 64,
                integer_min: 0,
                integer_max: 10,
            },
            sources,
            true,
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
            Blake3Digest32::from_bytes([9; 32]),
            SecretPolicy::ForbidPlaintext,
            [field],
            ConfigLimits::W1,
        )
        .expect("section");
        register_sections(1, [section], ConfigLimits::W1).expect("registry")
    }

    fn path() -> ConfigKeyPath {
        ConfigKeyPath::new(
            ConfigSectionName::new("query", 64).expect("section"),
            ConfigKeyName::new("limit", 64).expect("key"),
        )
    }

    #[test]
    fn higher_precedence_wins_deterministically() {
        let registry = registry(true);
        let environment = ConfigDocument::from_entries(
            1,
            None,
            source(ConfigSourceKind::Environment, 2),
            [(path(), LayerOperation::Set(DocumentValue::Integer(5)))],
            ConfigLimits::W1,
        )
        .expect("environment");
        let cli = ConfigDocument::from_entries(
            1,
            None,
            source(ConfigSourceKind::Cli, 3),
            [(path(), LayerOperation::Set(DocumentValue::Integer(7)))],
            ConfigLimits::W1,
        )
        .expect("cli");
        let merged = merge_layers(
            ConfigLayers {
                defaults: source(ConfigSourceKind::Defaults, 0),
                requested_profile: ProfileId::new("direct").expect("profile"),
                file: None,
                environment: Some(environment),
                cli: Some(cli),
            },
            &registry,
            ConfigLimits::W1,
        )
        .expect("merged");
        assert_eq!(
            merged.field(&path()).expect("field").value,
            ConfigValue::Integer(7)
        );
    }

    #[test]
    fn override_allowlist_is_enforced() {
        let registry = registry(false);
        let environment = ConfigDocument::from_entries(
            1,
            None,
            source(ConfigSourceKind::Environment, 2),
            [(path(), LayerOperation::Set(DocumentValue::Integer(5)))],
            ConfigLimits::W1,
        )
        .expect("environment");
        assert_eq!(
            merge_layers(
                ConfigLayers {
                    defaults: source(ConfigSourceKind::Defaults, 0),
                    requested_profile: ProfileId::new("direct").expect("profile"),
                    file: None,
                    environment: Some(environment),
                    cli: None,
                },
                &registry,
                ConfigLimits::W1,
            ),
            Err(ConfigError::OverrideNotAllowed)
        );
    }
}
