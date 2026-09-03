//! Exact environment-key mapping without environment acquisition.

use crate::{
    ConfigError, ConfigKeyName, ConfigKeyPath, ConfigRegistry, ConfigSectionName, ConfigSourceKind,
    ConfigValueKind,
};

const ENVIRONMENT_PREFIX: &str = "ELIOT_SEARCH__";

/// Validates one captured environment variable name against the closed registry.
///
/// Accepted names have the exact `ELIOT_SEARCH__SECTION__KEY` form, use ASCII
/// upper-case tokens, refer to a registered scalar field, and are explicitly
/// allowed by that field's override policy.
///
/// # Errors
///
/// Unknown prefixed variables, malformed names, list fields, and non-whitelisted
/// fields all fail closed as [`ConfigError::InvalidEnvironmentKey`].
pub fn validate_environment_key(
    name: &str,
    registry: &ConfigRegistry,
) -> Result<ConfigKeyPath, ConfigError> {
    let suffix = name
        .strip_prefix(ENVIRONMENT_PREFIX)
        .ok_or(ConfigError::InvalidEnvironmentKey)?;
    if suffix.is_empty()
        || suffix
            .bytes()
            .any(|byte| !byte.is_ascii_uppercase() && !byte.is_ascii_digit() && byte != b'_')
    {
        return Err(ConfigError::InvalidEnvironmentKey);
    }
    let mut parts = suffix.split("__");
    let section = parts.next().ok_or(ConfigError::InvalidEnvironmentKey)?;
    let key = parts.next().ok_or(ConfigError::InvalidEnvironmentKey)?;
    if section.is_empty() || key.is_empty() || parts.next().is_some() {
        return Err(ConfigError::InvalidEnvironmentKey);
    }

    let section = ConfigSectionName::new(section.to_ascii_lowercase(), 128)
        .map_err(|_| ConfigError::InvalidEnvironmentKey)?;
    let key = ConfigKeyName::new(key.to_ascii_lowercase(), 128)
        .map_err(|_| ConfigError::InvalidEnvironmentKey)?;
    let path = ConfigKeyPath::new(section, key);
    let descriptor = registry
        .field(&path)
        .ok_or(ConfigError::InvalidEnvironmentKey)?;
    if descriptor.kind() == ConfigValueKind::StringList
        || !descriptor.allows_source(ConfigSourceKind::Environment)
    {
        return Err(ConfigError::InvalidEnvironmentKey);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use search_contracts::Blake3Digest32;

    use super::validate_environment_key;
    use crate::{
        ConfigError, ConfigFieldDescriptor, ConfigKeyName, ConfigLimits, ConfigOwner,
        ConfigSectionDescriptor, ConfigSectionName, ConfigSourceKind, ConfigValue, ConfigValueKind,
        RedactionPolicy, ReloadClass, SecretPolicy, SecurityFloor, ValueBounds, register_sections,
    };

    fn registry() -> crate::ConfigRegistry {
        let field = ConfigFieldDescriptor::new(
            ConfigKeyName::new("max_in_flight", 64).expect("key"),
            ConfigValueKind::Integer,
            ConfigValue::Integer(4),
            ValueBounds {
                max_text_bytes: 64,
                max_list_items: 4,
                max_list_item_bytes: 64,
                integer_min: 1,
                integer_max: 32,
            },
            [ConfigSourceKind::Environment],
            false,
            SecurityFloor::None,
            RedactionPolicy::Public,
            [crate::ReconfigurationAction::ApplyLive],
        )
        .expect("field");
        let section = ConfigSectionDescriptor::new(
            ConfigSectionName::new("protocol", 64).expect("section"),
            ConfigOwner::new("search-provider-protocol", 64).expect("owner"),
            NonZeroU64::new(1).expect("revision"),
            ReloadClass::ApplyLive,
            Blake3Digest32::from_bytes([1; 32]),
            SecretPolicy::ForbidPlaintext,
            [field],
            ConfigLimits::W1,
        )
        .expect("section");
        register_sections(1, [section], ConfigLimits::W1).expect("registry")
    }

    #[test]
    fn exact_prefixed_scalar_mapping_is_accepted() {
        let path = validate_environment_key("ELIOT_SEARCH__PROTOCOL__MAX_IN_FLIGHT", &registry())
            .expect("mapping");
        assert_eq!(path.to_string(), "protocol.max_in_flight");
    }

    #[test]
    fn unknown_or_lowercase_names_fail_closed() {
        assert_eq!(
            validate_environment_key("ELIOT_SEARCH__PROTOCOL__TYPO", &registry()),
            Err(ConfigError::InvalidEnvironmentKey)
        );
        assert_eq!(
            validate_environment_key("ELIOT_SEARCH__protocol__MAX_IN_FLIGHT", &registry()),
            Err(ConfigError::InvalidEnvironmentKey)
        );
    }
}
