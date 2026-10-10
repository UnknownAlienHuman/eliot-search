//! Private shared admission budget for captured configuration documents.

use std::collections::BTreeMap;

use search_contracts::{MAX_PROFILE_ID_BYTES, ProfileId};

use super::ConfigDocument;
use crate::{
    ConfigError, ConfigKeyName, ConfigKeyPath, ConfigLimits, ConfigSectionName, ConfigSource,
    DocumentValue, LayerOperation,
};

/// The two text populations have distinct aggregate accounting.
#[derive(Clone, Copy)]
pub(super) enum TextPopulation {
    Profile,
    Value,
}

/// Counts only admitted decoded data, before owner allocation or collection growth.
pub(super) struct DocumentBuilder {
    limits: ConfigLimits,
    entries: BTreeMap<ConfigKeyPath, LayerOperation>,
    sections: BTreeMap<ConfigSectionName, usize>,
    identifier_bytes: usize,
    value_bytes: usize,
    list_items: usize,
}

impl DocumentBuilder {
    pub(super) const fn new(limits: ConfigLimits) -> Self {
        Self {
            limits,
            entries: BTreeMap::new(),
            sections: BTreeMap::new(),
            identifier_bytes: 0,
            value_bytes: 0,
            list_items: 0,
        }
    }

    pub(super) fn declare_table(&mut self, name: &str) -> Result<(), ConfigError> {
        if self.section_fields(name).is_some() {
            return Err(ConfigError::DuplicateTable);
        }
        if self.sections.len() >= self.limits.max_sections {
            return Err(ConfigError::CapacityExceeded);
        }
        self.check_identifier(name)?;
        self.charge_identifiers(name.len())?;
        let name = ConfigSectionName::new(name, self.limits.max_identifier_bytes)?;
        self.sections.insert(name, 0);
        Ok(())
    }

    // At most max_sections names are visited. No lookup token is cloned/allocated.
    fn section_fields(&self, section: &str) -> Option<usize> {
        self.sections
            .iter()
            .find_map(|(name, fields)| (name.as_str() == section).then_some(*fields))
    }

    const fn check_identifier(&self, name: &str) -> Result<(), ConfigError> {
        if name.len() > self.limits.max_identifier_bytes {
            return Err(ConfigError::CapacityExceeded);
        }
        Ok(())
    }

    fn charge_identifiers(&mut self, bytes: usize) -> Result<(), ConfigError> {
        self.identifier_bytes = accumulate(
            self.identifier_bytes,
            bytes,
            self.limits.max_decoded_identifier_bytes_total,
        )?;
        Ok(())
    }

    // Both entry paths reserve precisely the same retained identifier population.
    fn reserve_entry(&mut self, section: &str, key: &str) -> Result<(), ConfigError> {
        if self.entries.len() >= self.limits.max_entries_per_layer {
            return Err(ConfigError::CapacityExceeded);
        }
        let fields = self.section_fields(section);
        if fields.unwrap_or(0) >= self.limits.max_fields_per_section
            || (fields.is_none() && self.sections.len() >= self.limits.max_sections)
        {
            return Err(ConfigError::CapacityExceeded);
        }
        self.check_identifier(section)?;
        self.check_identifier(key)?;
        let bytes = section
            .len()
            .checked_add(key.len())
            .and_then(|bytes| bytes.checked_add(if fields.is_none() { section.len() } else { 0 }))
            .ok_or(ConfigError::LengthOverflow)?;
        self.charge_identifiers(bytes)
    }

    pub(super) fn parsed_path(
        &mut self,
        section: &str,
        key: &str,
    ) -> Result<ConfigKeyPath, ConfigError> {
        self.reserve_entry(section, key)?;
        let path = ConfigKeyPath::new(
            ConfigSectionName::new(section, self.limits.max_identifier_bytes)?,
            ConfigKeyName::new(key, self.limits.max_identifier_bytes)?,
        );
        if self.entries.contains_key(&path) {
            return Err(ConfigError::DuplicateKey);
        }
        Ok(path)
    }

    pub(super) fn push_captured(
        &mut self,
        path: ConfigKeyPath,
        operation: LayerOperation,
    ) -> Result<(), ConfigError> {
        if self.entries.contains_key(&path) {
            return Err(ConfigError::DuplicateKey);
        }
        self.reserve_entry(path.section().as_str(), path.key().as_str())?;
        if let LayerOperation::Set(value) = &operation {
            match value {
                DocumentValue::Boolean(_) => self.charge_scalar(1)?,
                DocumentValue::Integer(_) => self.charge_scalar(8)?,
                DocumentValue::Text(value) => {
                    self.charge_text(value.len(), TextPopulation::Value)?;
                }
                DocumentValue::StringList(values) => {
                    // Caller owns this Vec/its strings. Admission charges precede config growth.
                    for (index, value) in values.iter().enumerate() {
                        self.charge_list_item(index)?;
                        self.charge_text(value.len(), TextPopulation::Value)?;
                    }
                }
            }
        }
        self.commit_entry(path, operation)
    }

    /// Only already reserved paths/values can reach this private insertion seam.
    pub(super) fn commit_entry(
        &mut self,
        path: ConfigKeyPath,
        operation: LayerOperation,
    ) -> Result<(), ConfigError> {
        if self.entries.contains_key(&path) {
            return Err(ConfigError::DuplicateKey);
        }
        if let Some(fields) = self.sections.get_mut(path.section()) {
            *fields = fields.checked_add(1).ok_or(ConfigError::LengthOverflow)?;
        } else {
            // reserve_entry charged this extra retained name before its clone.
            self.sections.insert(path.section().clone(), 1);
        }
        self.entries.insert(path, operation);
        Ok(())
    }

    pub(super) fn charge_scalar(&mut self, bytes: usize) -> Result<(), ConfigError> {
        self.value_bytes = accumulate(
            self.value_bytes,
            bytes,
            self.limits.max_decoded_value_bytes_total,
        )?;
        Ok(())
    }

    pub(super) fn charge_text(
        &mut self,
        bytes: usize,
        population: TextPopulation,
    ) -> Result<(), ConfigError> {
        if bytes > self.limits.max_text_bytes {
            return Err(ConfigError::CapacityExceeded);
        }
        match population {
            TextPopulation::Value => self.charge_scalar(bytes),
            TextPopulation::Profile => {
                if bytes > MAX_PROFILE_ID_BYTES {
                    return Err(ConfigError::InvalidIdentifier);
                }
                self.charge_identifiers(bytes)
            }
        }
    }

    pub(super) fn charge_list_item(&mut self, current_list_len: usize) -> Result<(), ConfigError> {
        if current_list_len >= self.limits.max_list_items {
            return Err(ConfigError::CapacityExceeded);
        }
        self.list_items = accumulate(self.list_items, 1, self.limits.max_total_list_items)?;
        Ok(())
    }

    pub(super) fn finish(
        self,
        schema_version: u32,
        requested_profile: Option<ProfileId>,
        source: ConfigSource,
    ) -> ConfigDocument {
        ConfigDocument {
            schema_version,
            requested_profile,
            source,
            entries: self.entries,
            declared_sections: self.sections,
        }
    }
}

fn accumulate(total: usize, bytes: usize, maximum: usize) -> Result<usize, ConfigError> {
    let next = total
        .checked_add(bytes)
        .ok_or(ConfigError::LengthOverflow)?;
    if next > maximum {
        return Err(ConfigError::CapacityExceeded);
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::accumulate;
    use crate::ConfigError;

    #[test]
    fn accumulation_never_wraps() {
        assert_eq!(
            accumulate(usize::MAX, 1, usize::MAX),
            Err(ConfigError::LengthOverflow)
        );
        assert_eq!(accumulate(1, 1, 1), Err(ConfigError::CapacityExceeded));
        assert_eq!(accumulate(1, 1, 2), Ok(2));
    }
}
