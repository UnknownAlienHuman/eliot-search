//! Private finite typed rows. The shared encoder alone defines exact CBOR bytes.
use crate::{
    ConfigError, ConfigFingerprint, ConfigLimits, ConfigSectionInput, ConfigSectionName,
    ConfigValue, EffectiveSection, ValidatedSection,
};
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText,
    CanonicalValue, ContractError, DigestInputLimit, MAX_CANONICAL_BYTES, MAX_COLLECTION_ITEMS,
    MAX_RAW_BYTES, ProfileId, blake3_canonical_array,
};
use std::collections::BTreeMap;

const SECTION_DOMAIN: &str = "eliot/cbor/config-section/v2";
const SNAPSHOT_DOMAIN: &str = "eliot/cbor/config-snapshot/v2";

// Borrowed logical construction accounting, not another CBOR size/encoding engine.
pub struct IdentityBudget {
    limits: ConfigLimits,
    identifiers: usize,
    values: usize,
    list_items: usize,
    logical: usize,
}

impl IdentityBudget {
    fn new(limits: ConfigLimits) -> Result<Self, ConfigError> {
        Ok(Self {
            limits: limits.validate()?,
            identifiers: 0,
            values: 0,
            list_items: 0,
            logical: 0,
        })
    }

    fn logical(&mut self, bytes: usize) -> Result<(), ConfigError> {
        charge(
            &mut self.logical,
            bytes,
            self.limits.max_canonical_bytes.min(MAX_CANONICAL_BYTES),
        )
        .map_err(|error| match error {
            ConfigError::CapacityExceeded => ConfigError::CanonicalBytesExceeded,
            other => other,
        })
    }

    fn identifier(&mut self, value: &str) -> Result<(), ConfigError> {
        if value.len() > self.limits.max_identifier_bytes.min(MAX_RAW_BYTES) {
            return Err(ConfigError::CapacityExceeded);
        }
        charge(
            &mut self.identifiers,
            value.len(),
            self.limits.max_decoded_identifier_bytes_total,
        )?;
        self.logical(value.len())
    }

    fn payload(&mut self, bytes: usize) -> Result<(), ConfigError> {
        charge(
            &mut self.values,
            bytes,
            self.limits.max_decoded_value_bytes_total,
        )?;
        self.logical(bytes)
    }

    fn text_payload(&mut self, value: &str) -> Result<(), ConfigError> {
        if value.len() > self.limits.max_text_bytes.min(MAX_RAW_BYTES) {
            return Err(ConfigError::CapacityExceeded);
        }
        self.payload(value.len())
    }

    fn value(&mut self, value: &ConfigValue) -> Result<(), ConfigError> {
        match value {
            ConfigValue::Absent => Ok(()),
            ConfigValue::Boolean(_) => self.payload(1),
            ConfigValue::Integer(_) => self.payload(8),
            ConfigValue::Text(value) => self.text_payload(value),
            ConfigValue::SecretReference(value) => self.text_payload(value.as_str()),
            ConfigValue::StringList(values) => {
                if values.len() > self.limits.max_list_items.min(MAX_COLLECTION_ITEMS) {
                    return Err(ConfigError::CapacityExceeded);
                }
                charge(
                    &mut self.list_items,
                    values.len(),
                    self.limits.max_total_list_items,
                )?;
                for value in values {
                    self.text_payload(value)?;
                }
                Ok(())
            }
        }
    }

    // Called before assembly clones names/owners or inserts into its finite map.
    pub fn snapshot_section(
        &mut self,
        name: &ConfigSectionName,
        section: &ValidatedSection,
    ) -> Result<(), ConfigError> {
        self.identifier(name.as_str())?;
        self.identifier(section.owner().as_str())?;
        self.logical(8 + 32 + 32)
    }
}

fn charge(current: &mut usize, bytes: usize, ceiling: usize) -> Result<(), ConfigError> {
    let next = current
        .checked_add(bytes)
        .ok_or(ConfigError::LengthOverflow)?;
    if next > ceiling {
        return Err(ConfigError::CapacityExceeded);
    }
    *current = next;
    Ok(())
}

pub fn snapshot_budget(
    profile: &ProfileId,
    limits: ConfigLimits,
) -> Result<IdentityBudget, ConfigError> {
    let mut budget = IdentityBudget::new(limits)?;
    budget.logical(8)?;
    budget.identifier(profile.as_str())?;
    Ok(budget)
}

fn row_count(count: usize, ceiling: usize) -> Result<usize, ConfigError> {
    if count > ceiling || count >= MAX_COLLECTION_ITEMS {
        return Err(ConfigError::CapacityExceeded);
    }
    count.checked_add(1).ok_or(ConfigError::LengthOverflow)
}

pub fn section_digest(
    input: &ConfigSectionInput,
    selected_profile: &ProfileId,
    limits: ConfigLimits,
) -> Result<Blake3Digest32, ConfigError> {
    let mut budget = IdentityBudget::new(limits)?;
    let count = row_count(
        input.len(),
        limits
            .max_fields_per_section
            .min(limits.max_entries_per_layer),
    )?;
    for value in [
        input.section_name().as_str(),
        input.owner().as_str(),
        input.requested_profile().as_str(),
        selected_profile.as_str(),
    ] {
        budget.identifier(value)?;
    }
    budget.logical(8 + 32)?;
    for (key, field) in input.fields() {
        budget.identifier(key.as_str())?;
        budget.identifier(field.provenance.source.source_ref.as_str())?;
        budget.logical(8 + 8 + 32 + 1)?;
        budget.value(&field.value)?;
    }
    let header = (|| {
        row(vec![
            text(input.section_name().as_str())?,
            text(input.owner().as_str())?,
            CanonicalValue::U64(input.schema_revision().get()),
            digest_value(input.field_registry_digest())?,
            text(input.requested_profile().as_str())?,
            text(selected_profile.as_str())?,
        ])
    })();
    let rows = std::iter::once(header).chain(input.fields().map(|(key, field)| {
        row(vec![
            text(key.as_str())?,
            CanonicalValue::U64(value_kind(&field.value)),
            value_payload(&field.value)?,
            CanonicalValue::U64(u64::from(field.provenance.source.kind.precedence())),
            text(field.provenance.source.source_ref.as_str())?,
            digest_value(field.provenance.source.source_digest)?,
            CanonicalValue::Bool(field.provenance.explicit_reset),
        ])
    }));
    compute(SECTION_DOMAIN, count, rows, limits)
}

pub fn snapshot_fingerprint(
    schema: u32,
    profile: &ProfileId,
    sections: &BTreeMap<ConfigSectionName, EffectiveSection>,
    limits: ConfigLimits,
) -> Result<ConfigFingerprint, ConfigError> {
    let mut budget = snapshot_budget(profile, limits)?;
    let count = row_count(sections.len(), limits.max_sections)?;
    for (name, section) in sections {
        budget.snapshot_section(name, section.validated())?;
    }
    let header = (|| {
        row(vec![
            CanonicalValue::U64(u64::from(schema)),
            text(profile.as_str())?,
        ])
    })();
    let rows = std::iter::once(header).chain(sections.iter().map(|(name, section)| {
        let validated = section.validated();
        row(vec![
            text(name.as_str())?,
            text(validated.owner().as_str())?,
            CanonicalValue::U64(validated.schema_revision().get()),
            digest_value(validated.field_registry_digest())?,
            digest_value(validated.validated_section_digest())?,
        ])
    }));
    compute(SNAPSHOT_DOMAIN, count, rows, limits).map(ConfigFingerprint::canonical_v2)
}

fn compute(
    domain: &'static str,
    count: usize,
    rows: impl IntoIterator<Item = Result<CanonicalValue, ContractError>>,
    limits: ConfigLimits,
) -> Result<Blake3Digest32, ConfigError> {
    let domain = CanonicalDigestDomain::parse(domain).map_err(canonical_error)?;
    let limit = DigestInputLimit::new(limits.max_canonical_bytes.min(MAX_CANONICAL_BYTES))
        .map_err(canonical_error)?;
    blake3_canonical_array(&domain, count, rows, limit).map_err(canonical_error)
}

// Config errors carry neither value text nor caller-provided source identifiers.
const fn canonical_error(_: ContractError) -> ConfigError {
    ConfigError::CanonicalBytesExceeded
}

fn text(value: &str) -> Result<CanonicalValue, ContractError> {
    CanonicalText::new(value.to_owned()).map(CanonicalValue::Text)
}

fn digest_value(value: Blake3Digest32) -> Result<CanonicalValue, ContractError> {
    BoundedBytes::new(value.as_bytes().to_vec()).map(CanonicalValue::Bytes)
}

fn row(values: Vec<CanonicalValue>) -> Result<CanonicalValue, ContractError> {
    BoundedList::new(values).map(CanonicalValue::Array)
}

const fn value_kind(value: &ConfigValue) -> u64 {
    match value {
        ConfigValue::Absent => 0,
        ConfigValue::Boolean(_) => 1,
        ConfigValue::Integer(_) => 2,
        ConfigValue::Text(_) => 3,
        ConfigValue::SecretReference(_) => 4,
        ConfigValue::StringList(_) => 5,
    }
}

fn value_payload(value: &ConfigValue) -> Result<CanonicalValue, ContractError> {
    match value {
        ConfigValue::Absent => Ok(CanonicalValue::Null),
        ConfigValue::Boolean(value) => Ok(CanonicalValue::Bool(*value)),
        ConfigValue::Integer(value) if *value < 0 => Ok(CanonicalValue::I64(*value)),
        ConfigValue::Integer(value) => Ok(CanonicalValue::U64(value.unsigned_abs())),
        ConfigValue::Text(value) => text(value),
        ConfigValue::SecretReference(value) => text(value.as_str()),
        ConfigValue::StringList(values) => row(values
            .iter()
            .map(|value| text(value))
            .collect::<Result<Vec<_>, _>>()?),
    }
}
