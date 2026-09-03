//! Stable effective-configuration differences.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ConfigError, ConfigFingerprint, ConfigKeyPath, ConfigOwner, ConfigRegistry, ConfigSectionName,
    EffectiveConfigSnapshot, ReconfigurationAction,
};

/// Stable changes for one capability-owned section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionDelta {
    /// Canonical section name.
    pub section_name: ConfigSectionName,
    /// Registered capability owner.
    pub owner: ConfigOwner,
    /// Changed field paths in canonical order.
    pub changed_key_paths: BTreeSet<ConfigKeyPath>,
    /// Independent obligations contributed by this section.
    pub required_actions: BTreeSet<ReconfigurationAction>,
    /// Whether at least one change is directionally restrictive.
    pub restrictive: bool,
}

/// Stable complete delta between two effective snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigDelta {
    /// Authoritative old fingerprint.
    pub old_fingerprint: ConfigFingerprint,
    /// Candidate fingerprint.
    pub candidate_fingerprint: ConfigFingerprint,
    /// Whether the selected profile identity changed.
    pub profile_changed: bool,
    /// Global obligations not owned by one field.
    pub global_actions: BTreeSet<ReconfigurationAction>,
    /// Changed sections in canonical order.
    pub sections: BTreeMap<ConfigSectionName, SectionDelta>,
}

impl ConfigDelta {
    /// Returns whether the snapshots are materially identical.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.profile_changed && self.global_actions.is_empty() && self.sections.is_empty()
    }

    /// Unions every independent obligation without reducing it to a scalar.
    #[must_use]
    pub fn required_actions(&self) -> BTreeSet<ReconfigurationAction> {
        let mut actions = self.global_actions.clone();
        for section in self.sections.values() {
            actions.extend(section.required_actions.iter().copied());
        }
        actions
    }

    /// Unions affected capability owners.
    #[must_use]
    pub fn affected_capabilities(&self) -> BTreeSet<ConfigOwner> {
        self.sections
            .values()
            .map(|section| section.owner.clone())
            .collect()
    }
}

/// Produces a stable key-level delta under the current closed registry.
///
/// # Errors
///
/// Rejects schema mismatches, missing sections/fields, and snapshots whose
/// descriptor-bound values are no longer present in the registry.
pub fn diff(
    old: &EffectiveConfigSnapshot,
    candidate: &EffectiveConfigSnapshot,
    registry: &ConfigRegistry,
) -> Result<ConfigDelta, ConfigError> {
    if old.config_schema_version() != registry.config_schema_version()
        || candidate.config_schema_version() != registry.config_schema_version()
    {
        return Err(ConfigError::UnsupportedSchemaVersion);
    }

    let profile_changed = old.selected_profile() != candidate.selected_profile();
    let global_actions = if profile_changed {
        BTreeSet::from([ReconfigurationAction::GateRequired])
    } else {
        BTreeSet::new()
    };
    let mut sections = BTreeMap::new();

    for (section_name, descriptor) in registry.sections() {
        let old_section = old
            .section(section_name)
            .ok_or(ConfigError::MissingSection)?;
        let new_section = candidate
            .section(section_name)
            .ok_or(ConfigError::MissingSection)?;
        if old_section.owner() != descriptor.owner() || new_section.owner() != descriptor.owner() {
            return Err(ConfigError::StaleDescriptor);
        }

        let mut changed_key_paths = BTreeSet::new();
        let mut required_actions = BTreeSet::new();
        let mut restrictive = false;
        for (key, field_descriptor) in descriptor.fields() {
            let old_field = old_section.field(key).ok_or(ConfigError::MissingSection)?;
            let new_field = new_section.field(key).ok_or(ConfigError::MissingSection)?;
            if old_field.value == new_field.value {
                continue;
            }
            changed_key_paths.insert(ConfigKeyPath::new(section_name.clone(), key.clone()));
            if let Some(action) = descriptor.minimum_reload().action() {
                required_actions.insert(action);
            }
            required_actions.extend(field_descriptor.actions());
            restrictive |=
                field_descriptor.is_restrictive_change(&old_field.value, &new_field.value);
        }

        if profile_changed {
            required_actions.insert(ReconfigurationAction::GateRequired);
        }
        if !changed_key_paths.is_empty() || profile_changed {
            sections.insert(
                section_name.clone(),
                SectionDelta {
                    section_name: section_name.clone(),
                    owner: descriptor.owner().clone(),
                    changed_key_paths,
                    required_actions,
                    restrictive,
                },
            );
        }
    }

    Ok(ConfigDelta {
        old_fingerprint: old.fingerprint(),
        candidate_fingerprint: candidate.fingerprint(),
        profile_changed,
        global_actions,
        sections,
    })
}
