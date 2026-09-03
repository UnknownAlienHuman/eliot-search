//! Capability-owned section projection and validation handoff values.

use std::collections::BTreeMap;
use std::num::NonZeroU64;

use search_contracts::{Blake3Digest32, ProfileId};

use crate::{
    ConfigError, ConfigKeyName, ConfigKeyPath, ConfigOwner, ConfigSectionDescriptor,
    ConfigSectionName, ConfigValue, FieldProvenance, MergedConfig,
};

/// One projected field delivered only to its registered capability owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSectionField {
    /// Typed effective value.
    pub value: ConfigValue,
    /// Exact source and reset provenance.
    pub provenance: FieldProvenance,
}

/// Complete immutable input for one package-owned section validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSectionInput {
    section_name: ConfigSectionName,
    owner: ConfigOwner,
    schema_revision: NonZeroU64,
    field_registry_digest: Blake3Digest32,
    requested_profile: ProfileId,
    fields: BTreeMap<ConfigKeyName, ConfigSectionField>,
}

impl ConfigSectionInput {
    /// Canonical section name.
    #[must_use]
    pub const fn section_name(&self) -> &ConfigSectionName {
        &self.section_name
    }

    /// Registered capability owner.
    #[must_use]
    pub const fn owner(&self) -> &ConfigOwner {
        &self.owner
    }

    /// Descriptor schema revision.
    #[must_use]
    pub const fn schema_revision(&self) -> NonZeroU64 {
        self.schema_revision
    }

    /// Exact field-registry digest.
    #[must_use]
    pub const fn field_registry_digest(&self) -> Blake3Digest32 {
        self.field_registry_digest
    }

    /// Requested profile ceiling.
    #[must_use]
    pub const fn requested_profile(&self) -> &ProfileId {
        &self.requested_profile
    }

    /// One projected field.
    #[must_use]
    pub fn field(&self, key: &ConfigKeyName) -> Option<&ConfigSectionField> {
        self.fields.get(key)
    }

    /// Projected fields in canonical key order.
    #[must_use]
    pub fn fields(&self) -> impl ExactSizeIterator<Item = (&ConfigKeyName, &ConfigSectionField)> {
        self.fields.iter()
    }

    /// Number of projected fields.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns whether no field was projected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// Projects only fields owned by the supplied section descriptor.
///
/// # Errors
///
/// Returns [`ConfigError::MissingSection`] if a registered field is absent from
/// the complete merged snapshot.
pub fn project_section(
    merged: &MergedConfig,
    descriptor: &ConfigSectionDescriptor,
) -> Result<ConfigSectionInput, ConfigError> {
    let mut fields = BTreeMap::new();
    for (key, _field_descriptor) in descriptor.fields() {
        let path = ConfigKeyPath::new(descriptor.name().clone(), key.clone());
        let merged_field = merged.field(&path).ok_or(ConfigError::MissingSection)?;
        fields.insert(
            key.clone(),
            ConfigSectionField {
                value: merged_field.value.clone(),
                provenance: merged_field.provenance.clone(),
            },
        );
    }
    Ok(ConfigSectionInput {
        section_name: descriptor.name().clone(),
        owner: descriptor.owner().clone(),
        schema_revision: descriptor.schema_revision(),
        field_registry_digest: descriptor.field_registry_digest(),
        requested_profile: merged.requested_profile().clone(),
        fields,
    })
}

/// Result returned by a capability-owned section validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedSection {
    section_name: ConfigSectionName,
    owner: ConfigOwner,
    schema_revision: NonZeroU64,
    field_registry_digest: Blake3Digest32,
    selected_profile: ProfileId,
    validation_digest: Blake3Digest32,
    fields: BTreeMap<ConfigKeyName, ConfigSectionField>,
}

impl ValidatedSection {
    /// Binds a package-validated digest to the exact projected descriptor input.
    #[must_use]
    pub fn new(
        input: ConfigSectionInput,
        selected_profile: ProfileId,
        validation_digest: Blake3Digest32,
    ) -> Self {
        Self {
            section_name: input.section_name,
            owner: input.owner,
            schema_revision: input.schema_revision,
            field_registry_digest: input.field_registry_digest,
            selected_profile,
            validation_digest,
            fields: input.fields,
        }
    }

    /// Canonical section name.
    #[must_use]
    pub const fn section_name(&self) -> &ConfigSectionName {
        &self.section_name
    }

    /// Registered capability owner.
    #[must_use]
    pub const fn owner(&self) -> &ConfigOwner {
        &self.owner
    }

    /// Descriptor schema revision used by validation.
    #[must_use]
    pub const fn schema_revision(&self) -> NonZeroU64 {
        self.schema_revision
    }

    /// Field-registry digest used by validation.
    #[must_use]
    pub const fn field_registry_digest(&self) -> Blake3Digest32 {
        self.field_registry_digest
    }

    /// Selected authorized profile used by validation.
    #[must_use]
    pub const fn selected_profile(&self) -> &ProfileId {
        &self.selected_profile
    }

    /// Capability-owned digest of the complete validated section.
    #[must_use]
    pub const fn validated_section_digest(&self) -> Blake3Digest32 {
        self.validation_digest
    }

    /// One validated field.
    #[must_use]
    pub fn field(&self, key: &ConfigKeyName) -> Option<&ConfigSectionField> {
        self.fields.get(key)
    }

    /// Validated fields in canonical key order.
    #[must_use]
    pub fn fields(&self) -> impl ExactSizeIterator<Item = (&ConfigKeyName, &ConfigSectionField)> {
        self.fields.iter()
    }
}

pub(crate) fn descriptor_matches(
    section: &ValidatedSection,
    descriptor: &ConfigSectionDescriptor,
) -> bool {
    section.section_name == *descriptor.name()
        && section.owner == *descriptor.owner()
        && section.schema_revision == descriptor.schema_revision()
        && section.field_registry_digest == descriptor.field_registry_digest()
        && section.fields.len() == descriptor.fields().len()
        && descriptor.fields().all(|(key, field)| {
            section
                .fields
                .get(key)
                .is_some_and(|value| field.validate_typed(&value.value).is_ok())
        })
}
