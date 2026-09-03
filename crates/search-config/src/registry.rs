//! Closed section and field registration.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use search_contracts::Blake3Digest32;

use crate::{
    ConfigError, ConfigKeyName, ConfigKeyPath, ConfigLimits, ConfigOwner, ConfigSectionName,
    ConfigSourceKind, ConfigValue, ConfigValueKind, DocumentValue, ReconfigurationAction,
    RedactionPolicy, ReloadClass, SecretReference, SecurityFloor, ValueBounds,
};

/// Section-wide secret handling policy.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SecretPolicy {
    /// No secret-bearing field is legal in this section.
    ForbidPlaintext,
    /// Secret fields contain opaque references only.
    OpaqueReferencesOnly,
}

/// Closed descriptor for one capability-owned field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigFieldDescriptor {
    key: ConfigKeyName,
    kind: ConfigValueKind,
    default: ConfigValue,
    bounds: ValueBounds,
    allowed_sources: BTreeSet<ConfigSourceKind>,
    reset_allowed: bool,
    security_floor: SecurityFloor,
    redaction: RedactionPolicy,
    actions: BTreeSet<ReconfigurationAction>,
}

impl ConfigFieldDescriptor {
    /// Creates a validated field descriptor.
    ///
    /// # Errors
    ///
    /// Rejects invalid bounds/defaults, default/source misuse, an empty override
    /// policy for a non-fixed field, and inconsistent secret/redaction policy.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: ConfigKeyName,
        kind: ConfigValueKind,
        default: ConfigValue,
        bounds: ValueBounds,
        allowed_sources: impl IntoIterator<Item = ConfigSourceKind>,
        reset_allowed: bool,
        security_floor: SecurityFloor,
        redaction: RedactionPolicy,
        actions: impl IntoIterator<Item = ReconfigurationAction>,
    ) -> Result<Self, ConfigError> {
        let bounds = bounds.validate()?;
        let allowed_sources = allowed_sources.into_iter().collect::<BTreeSet<_>>();
        if allowed_sources.contains(&ConfigSourceKind::Defaults) {
            return Err(ConfigError::InvalidDescriptor);
        }
        if kind == ConfigValueKind::SecretReference && redaction != RedactionPolicy::Secret {
            return Err(ConfigError::InvalidDescriptor);
        }
        if kind != ConfigValueKind::SecretReference && redaction == RedactionPolicy::Secret {
            return Err(ConfigError::InvalidDescriptor);
        }
        let descriptor = Self {
            key,
            kind,
            default,
            bounds,
            allowed_sources,
            reset_allowed,
            security_floor,
            redaction,
            actions: actions.into_iter().collect(),
        };
        descriptor.validate_typed(&descriptor.default)?;
        descriptor.validate_security_floor(&descriptor.default, &descriptor.default)?;
        Ok(descriptor)
    }

    /// Canonical field key.
    #[must_use]
    pub const fn key(&self) -> &ConfigKeyName {
        &self.key
    }

    /// Closed field kind.
    #[must_use]
    pub const fn kind(&self) -> ConfigValueKind {
        self.kind
    }

    /// Capability-owned compiled default.
    #[must_use]
    pub const fn default(&self) -> &ConfigValue {
        &self.default
    }

    /// Finite field bounds.
    #[must_use]
    pub const fn bounds(&self) -> ValueBounds {
        self.bounds
    }

    /// Whether a captured source may override this field.
    #[must_use]
    pub fn allows_source(&self, source: ConfigSourceKind) -> bool {
        self.allowed_sources.contains(&source)
    }

    /// Whether an explicit reset marker is accepted.
    #[must_use]
    pub const fn reset_allowed(&self) -> bool {
        self.reset_allowed
    }

    /// Directional security floor.
    #[must_use]
    pub const fn security_floor(&self) -> SecurityFloor {
        self.security_floor
    }

    /// Ordinary diagnostic disclosure policy.
    #[must_use]
    pub const fn redaction(&self) -> RedactionPolicy {
        self.redaction
    }

    /// Independent field obligations in canonical order.
    #[must_use]
    pub fn actions(&self) -> impl ExactSizeIterator<Item = ReconfigurationAction> + '_ {
        self.actions.iter().copied()
    }

    /// Converts and validates a parser-level value.
    ///
    /// # Errors
    ///
    /// Returns distinct type, bounds, and plaintext-secret failures.
    pub fn validate_document_value(
        &self,
        value: &DocumentValue,
    ) -> Result<ConfigValue, ConfigError> {
        let typed = match (self.kind, value) {
            (ConfigValueKind::Boolean, DocumentValue::Boolean(value)) => {
                ConfigValue::Boolean(*value)
            }
            (ConfigValueKind::Integer, DocumentValue::Integer(value)) => {
                ConfigValue::Integer(*value)
            }
            (ConfigValueKind::Text, DocumentValue::Text(value)) => ConfigValue::Text(value.clone()),
            (ConfigValueKind::SecretReference, DocumentValue::Text(value)) => {
                ConfigValue::SecretReference(SecretReference::new(
                    value.clone(),
                    self.bounds.max_text_bytes,
                )?)
            }
            (ConfigValueKind::StringList, DocumentValue::StringList(values)) => {
                ConfigValue::StringList(values.clone())
            }
            _ => return Err(ConfigError::TypeMismatch),
        };
        self.validate_typed(&typed)?;
        Ok(typed)
    }

    /// Validates an already typed value.
    ///
    /// # Errors
    ///
    /// Returns distinct kind and bound failures.
    pub fn validate_typed(&self, value: &ConfigValue) -> Result<(), ConfigError> {
        if matches!(value, ConfigValue::Absent) {
            return if matches!(self.default, ConfigValue::Absent) {
                Ok(())
            } else {
                Err(ConfigError::TypeMismatch)
            };
        }
        if value.kind() != Some(self.kind) {
            return Err(ConfigError::TypeMismatch);
        }
        match value {
            ConfigValue::Absent | ConfigValue::Boolean(_) => Ok(()),
            ConfigValue::Integer(value)
                if *value >= self.bounds.integer_min && *value <= self.bounds.integer_max =>
            {
                Ok(())
            }
            ConfigValue::Integer(_) => Err(ConfigError::ValueOutOfBounds),
            ConfigValue::Text(value) => {
                if value.is_empty() || value.len() > self.bounds.max_text_bytes {
                    Err(ConfigError::ValueOutOfBounds)
                } else {
                    Ok(())
                }
            }
            ConfigValue::SecretReference(value) => {
                if value.as_str().len() > self.bounds.max_text_bytes {
                    Err(ConfigError::ValueOutOfBounds)
                } else {
                    Ok(())
                }
            }
            ConfigValue::StringList(values) => {
                if values.len() > self.bounds.max_list_items {
                    return Err(ConfigError::ValueOutOfBounds);
                }
                for value in values {
                    if value.is_empty() || value.len() > self.bounds.max_list_item_bytes {
                        return Err(ConfigError::ValueOutOfBounds);
                    }
                }
                Ok(())
            }
        }
    }

    /// Verifies a directional security floor relative to the prior value.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::SecurityFloorViolation`] on weakening.
    pub fn validate_security_floor(
        &self,
        before: &ConfigValue,
        after: &ConfigValue,
    ) -> Result<(), ConfigError> {
        self.validate_typed(before)?;
        self.validate_typed(after)?;
        let valid = match self.security_floor {
            SecurityFloor::None => true,
            SecurityFloor::Fixed => before == after,
            SecurityFloor::BooleanMayOnlyRestrict => {
                matches!(
                    (before, after),
                    (ConfigValue::Boolean(true), ConfigValue::Boolean(_))
                        | (ConfigValue::Boolean(false), ConfigValue::Boolean(false))
                )
            }
            SecurityFloor::IntegerMinimum(floor) => {
                matches!(after, ConfigValue::Integer(value) if *value >= floor)
            }
            SecurityFloor::IntegerMaximum(ceiling) => {
                matches!(after, ConfigValue::Integer(value) if *value <= ceiling)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ConfigError::SecurityFloorViolation)
        }
    }

    /// Returns whether a material change is directionally restrictive.
    #[must_use]
    pub fn is_restrictive_change(&self, before: &ConfigValue, after: &ConfigValue) -> bool {
        if before == after {
            return false;
        }
        if self
            .actions
            .contains(&ReconfigurationAction::SecurityBarrier)
        {
            return true;
        }
        match (self.security_floor, before, after) {
            (
                SecurityFloor::BooleanMayOnlyRestrict,
                ConfigValue::Boolean(true),
                ConfigValue::Boolean(false),
            ) => true,
            (
                SecurityFloor::IntegerMinimum(_),
                ConfigValue::Integer(before),
                ConfigValue::Integer(after),
            ) => after > before,
            (
                SecurityFloor::IntegerMaximum(_),
                ConfigValue::Integer(before),
                ConfigValue::Integer(after),
            ) => after < before,
            _ => false,
        }
    }
}

/// Closed descriptor for one capability-owned section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSectionDescriptor {
    name: ConfigSectionName,
    owner: ConfigOwner,
    schema_revision: NonZeroU64,
    minimum_reload: ReloadClass,
    field_registry_digest: Blake3Digest32,
    secret_policy: SecretPolicy,
    fields: BTreeMap<ConfigKeyName, ConfigFieldDescriptor>,
}

impl ConfigSectionDescriptor {
    /// Creates a finite, collision-free section descriptor.
    ///
    /// # Errors
    ///
    /// Rejects empty/oversize fields, duplicate keys, and secret-policy conflicts.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: ConfigSectionName,
        owner: ConfigOwner,
        schema_revision: NonZeroU64,
        minimum_reload: ReloadClass,
        field_registry_digest: Blake3Digest32,
        secret_policy: SecretPolicy,
        fields: impl IntoIterator<Item = ConfigFieldDescriptor>,
        limits: ConfigLimits,
    ) -> Result<Self, ConfigError> {
        let limits = limits.validate()?;
        let mut registered = BTreeMap::new();
        for field in fields {
            if registered.len() >= limits.max_fields_per_section {
                return Err(ConfigError::CapacityExceeded);
            }
            if secret_policy == SecretPolicy::ForbidPlaintext
                && field.kind == ConfigValueKind::SecretReference
            {
                return Err(ConfigError::InvalidDescriptor);
            }
            if registered.insert(field.key.clone(), field).is_some() {
                return Err(ConfigError::FieldConflict);
            }
        }
        if registered.is_empty() {
            return Err(ConfigError::InvalidDescriptor);
        }
        Ok(Self {
            name,
            owner,
            schema_revision,
            minimum_reload,
            field_registry_digest,
            secret_policy,
            fields: registered,
        })
    }

    /// Canonical section name.
    #[must_use]
    pub const fn name(&self) -> &ConfigSectionName {
        &self.name
    }

    /// Owning package or capability.
    #[must_use]
    pub const fn owner(&self) -> &ConfigOwner {
        &self.owner
    }

    /// Descriptor schema revision.
    #[must_use]
    pub const fn schema_revision(&self) -> NonZeroU64 {
        self.schema_revision
    }

    /// Section minimum reload class.
    #[must_use]
    pub const fn minimum_reload(&self) -> ReloadClass {
        self.minimum_reload
    }

    /// Digest of the capability-owned field registry.
    #[must_use]
    pub const fn field_registry_digest(&self) -> Blake3Digest32 {
        self.field_registry_digest
    }

    /// Section-wide secret policy.
    #[must_use]
    pub const fn secret_policy(&self) -> SecretPolicy {
        self.secret_policy
    }

    /// Exact field descriptor.
    #[must_use]
    pub fn field(&self, key: &ConfigKeyName) -> Option<&ConfigFieldDescriptor> {
        self.fields.get(key)
    }

    /// Fields in canonical key order.
    #[must_use]
    pub fn fields(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ConfigKeyName, &ConfigFieldDescriptor)> {
        self.fields.iter()
    }
}

/// Finite closed configuration registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigRegistry {
    config_schema_version: u32,
    sections: BTreeMap<ConfigSectionName, ConfigSectionDescriptor>,
}

impl ConfigRegistry {
    /// Exact accepted configuration schema version.
    #[must_use]
    pub const fn config_schema_version(&self) -> u32 {
        self.config_schema_version
    }

    /// Exact section descriptor.
    #[must_use]
    pub fn section(&self, name: &ConfigSectionName) -> Option<&ConfigSectionDescriptor> {
        self.sections.get(name)
    }

    /// Exact field descriptor.
    #[must_use]
    pub fn field(&self, path: &ConfigKeyPath) -> Option<&ConfigFieldDescriptor> {
        self.section(path.section())?.field(path.key())
    }

    /// Sections in canonical name order.
    #[must_use]
    pub fn sections(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ConfigSectionName, &ConfigSectionDescriptor)> {
        self.sections.iter()
    }

    /// Number of registered sections.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sections.len()
    }

    /// Returns whether the registry has no sections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

/// Registers a complete closed section set independently of input order.
///
/// # Errors
///
/// Rejects zero schema version, empty/oversize registry, and duplicate section names.
pub fn register_sections(
    config_schema_version: u32,
    descriptors: impl IntoIterator<Item = ConfigSectionDescriptor>,
    limits: ConfigLimits,
) -> Result<ConfigRegistry, ConfigError> {
    let limits = limits.validate()?;
    if config_schema_version == 0 {
        return Err(ConfigError::UnsupportedSchemaVersion);
    }
    let mut sections = BTreeMap::new();
    for descriptor in descriptors {
        if sections.len() >= limits.max_sections {
            return Err(ConfigError::CapacityExceeded);
        }
        if sections
            .insert(descriptor.name.clone(), descriptor)
            .is_some()
        {
            return Err(ConfigError::SectionConflict);
        }
    }
    if sections.is_empty() {
        return Err(ConfigError::InvalidDescriptor);
    }
    Ok(ConfigRegistry {
        config_schema_version,
        sections,
    })
}
