//! Owner-checked secrets configuration metadata; no runtime or backend authority.

use search_config::{
    ConfigError, ConfigFieldDescriptor, ConfigKeyName, ConfigKeyPath, ConfigLimits, ConfigOwner,
    ConfigSectionDescriptor, ConfigSectionName, ConfigSourceKind as Source, ConfigValue as Value,
    ConfigValueKind as Kind, ReconfigurationAction as Action, RedactionPolicy, ReloadClass,
    SecretPolicy, SecretReference, SectionDelta, SecurityFloor, ValidatedSection, ValueBounds,
};
use search_contracts::{self as c, Blake3Digest32, CanonicalValue as V};
use std::{collections::BTreeSet, fmt, num::NonZeroU64};

const SECTION: &str = "secrets";
const OWNER: &str = "search-os-secrets";
const BACKEND: &str = "windows_user_bound";
const BOUNDS: ValueBounds = ValueBounds {
    max_text_bytes: 4096,
    max_list_items: 64,
    max_list_item_bytes: 512,
    integer_min: 0,
    integer_max: 16_777_216,
};

/// Immutable source-admitted settings, without application, lease or key-use proof.
#[derive(Clone, Eq, PartialEq)]
pub struct ValidatedSecretsConfig {
    backend: &'static str,
    lease_ttl_ms: u64,
    rotation_grace_ms: u64,
    qdrant_api_secret_ref: Option<SecretReference>,
    section_digest: Blake3Digest32,
}

impl ValidatedSecretsConfig {
    /// Fixed backend spelling; does not assert platform qualification.
    #[must_use]
    pub const fn backend(&self) -> &'static str {
        self.backend
    }
    /// Source-admitted finite lease duration in milliseconds.
    #[must_use]
    pub const fn lease_ttl_ms(&self) -> u64 {
        self.lease_ttl_ms
    }
    /// Source-admitted finite rotation grace in milliseconds.
    #[must_use]
    pub const fn rotation_grace_ms(&self) -> u64 {
        self.rotation_grace_ms
    }
    /// Optional opaque reference, never resolved by configuration admission.
    #[must_use]
    pub const fn qdrant_api_secret_ref(&self) -> Option<&SecretReference> {
        self.qdrant_api_secret_ref.as_ref()
    }
}

impl fmt::Debug for ValidatedSecretsConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ValidatedSecretsConfig(<metadata; reference redacted>)")
    }
}

/// Complete revision-2 descriptor, including defaults and finite source policies.
pub fn section_descriptor() -> Result<ConfigSectionDescriptor, ConfigError> {
    let fields = [
        field("backend", Value::Text(BACKEND.to_owned()), BOUNDS)?,
        field(
            "lease_ttl_ms",
            Value::Integer(30_000),
            ValueBounds {
                integer_min: 1_000,
                integer_max: 300_000,
                ..BOUNDS
            },
        )?,
        field("qdrant_api_secret_ref", Value::Absent, BOUNDS)?,
        field(
            "rotation_grace_ms",
            Value::Integer(60_000),
            ValueBounds {
                integer_min: 0,
                integer_max: 600_000,
                ..BOUNDS
            },
        )?,
    ];
    ConfigSectionDescriptor::new(
        ConfigSectionName::new(SECTION, ConfigLimits::W1.max_identifier_bytes)?,
        ConfigOwner::new(OWNER, ConfigLimits::W1.max_identifier_bytes)?,
        NonZeroU64::new(2).ok_or(ConfigError::InvalidDescriptor)?,
        ReloadClass::RestartDependency,
        descriptor_digest(&fields)?,
        SecretPolicy::OpaqueReferencesOnly,
        fields,
        ConfigLimits::W1,
    )
}

fn key(value: &str) -> Result<ConfigKeyName, ConfigError> {
    ConfigKeyName::new(value, ConfigLimits::W1.max_identifier_bytes)
}

fn field(
    name: &str,
    default: Value,
    bounds: ValueBounds,
) -> Result<ConfigFieldDescriptor, ConfigError> {
    let (kind, sources, reset, floor, redaction, action) = match name {
        "backend" => (
            Kind::Text,
            vec![Source::File],
            false,
            SecurityFloor::Fixed,
            RedactionPolicy::Public,
            Action::RestartDependency,
        ),
        "lease_ttl_ms" | "rotation_grace_ms" => (
            Kind::Integer,
            vec![Source::File],
            false,
            SecurityFloor::None,
            RedactionPolicy::Public,
            Action::ApplyLive,
        ),
        "qdrant_api_secret_ref" => (
            Kind::SecretReference,
            vec![Source::File, Source::Environment],
            true,
            SecurityFloor::None,
            RedactionPolicy::Secret,
            Action::RestartDependency,
        ),
        _ => return Err(ConfigError::UnknownKey),
    };
    ConfigFieldDescriptor::new(
        key(name)?,
        kind,
        default,
        bounds,
        sources,
        reset,
        floor,
        redaction,
        [action],
    )
}

/// Checks exact descriptor/shape, scalar bounds, fixed backend and source/reset provenance.
/// The input digest remains equality metadata, never runtime application evidence.
pub fn validate_secrets_section(
    section: &ValidatedSection,
) -> Result<ValidatedSecretsConfig, ConfigError> {
    let descriptor = section_descriptor()?;
    if section.section_name() != descriptor.name()
        || section.owner() != descriptor.owner()
        || section.schema_revision() != descriptor.schema_revision()
        || section.field_registry_digest() != descriptor.field_registry_digest()
    {
        return Err(ConfigError::StaleDescriptor);
    }
    for (key, value) in section.fields() {
        let field = descriptor.field(key).ok_or(ConfigError::UnknownKey)?;
        field.validate_typed(&value.value)?;
        field.validate_security_floor(field.default(), &value.value)?;
        let provenance = &value.provenance;
        if provenance.explicit_reset && !field.reset_allowed() {
            return Err(ConfigError::ResetNotAllowed);
        }
        if provenance.source.kind == Source::Defaults {
            if provenance.explicit_reset || value.value != *field.default() {
                return Err(ConfigError::OverrideNotAllowed);
            }
        } else if !field.allows_source(provenance.source.kind) {
            return Err(ConfigError::OverrideNotAllowed);
        }
        if provenance.explicit_reset && value.value != *field.default() {
            return Err(ConfigError::InvalidDescriptor);
        }
    }
    if section.fields().len() != descriptor.fields().len() {
        return Err(ConfigError::MissingSection);
    }
    let qdrant_api_secret_ref = match value(section, "qdrant_api_secret_ref")? {
        Value::Absent => None,
        Value::SecretReference(reference) => Some(reference.clone()),
        _ => return Err(ConfigError::TypeMismatch),
    };
    Ok(ValidatedSecretsConfig {
        backend: BACKEND,
        lease_ttl_ms: number(section, "lease_ttl_ms")?,
        rotation_grace_ms: number(section, "rotation_grace_ms")?,
        qdrant_api_secret_ref,
        section_digest: section.validated_section_digest(),
    })
}

fn value<'a>(section: &'a ValidatedSection, name: &str) -> Result<&'a Value, ConfigError> {
    section
        .field(&key(name)?)
        .map(|field| &field.value)
        .ok_or(ConfigError::MissingSection)
}

fn number(section: &ValidatedSection, name: &str) -> Result<u64, ConfigError> {
    let Value::Integer(value) = value(section, name)? else {
        return Err(ConfigError::TypeMismatch);
    };
    u64::try_from(*value).map_err(|_| ConfigError::ValueOutOfBounds)
}

/// Config-owned equality/provenance digest retained after owner validation.
#[must_use]
pub const fn section_digest(config: &ValidatedSecretsConfig) -> Blake3Digest32 {
    config.section_digest
}

/// Plans field directions and retains the section restart floor for every value change.
/// Provenance-only changes require no action; no execution receipt is issued.
pub fn plan_section_change(
    old: &ValidatedSecretsConfig,
    new: &ValidatedSecretsConfig,
) -> Result<SectionDelta, ConfigError> {
    let descriptor = section_descriptor()?;
    let mut delta = SectionDelta {
        section_name: descriptor.name().clone(),
        owner: descriptor.owner().clone(),
        changed_key_paths: BTreeSet::new(),
        required_actions: BTreeSet::new(),
        restrictive: false,
    };
    let changes = [
        (
            "lease_ttl_ms",
            old.lease_ttl_ms != new.lease_ttl_ms,
            if new.lease_ttl_ms < old.lease_ttl_ms {
                Action::ApplyLive
            } else {
                Action::RestartDependency
            },
        ),
        (
            "rotation_grace_ms",
            old.rotation_grace_ms != new.rotation_grace_ms,
            Action::ApplyLive,
        ),
        (
            "qdrant_api_secret_ref",
            old.qdrant_api_secret_ref != new.qdrant_api_secret_ref,
            Action::RestartDependency,
        ),
    ];
    for (name, changed, action) in changes {
        if changed {
            delta
                .changed_key_paths
                .insert(ConfigKeyPath::new(descriptor.name().clone(), key(name)?));
            delta.required_actions.insert(action);
            delta
                .required_actions
                .extend(descriptor.minimum_reload().action());
        }
    }
    delta.restrictive = new.lease_ttl_ms < old.lease_ttl_ms;
    Ok(delta)
}

// Closed owner profile: complete descriptor metadata in canonical key order.
// The domain versions the field directions as well as the serialized policies.
fn descriptor_digest(fields: &[ConfigFieldDescriptor; 4]) -> Result<Blake3Digest32, ConfigError> {
    let mut rows = Vec::new();
    for field in fields {
        let kind = match field.kind() {
            Kind::Integer => "integer",
            Kind::Text => "text",
            Kind::SecretReference => "secret-reference",
            _ => return Err(ConfigError::InvalidDescriptor),
        };
        let default = match field.default() {
            Value::Absent => V::Null,
            Value::Integer(value) => {
                V::U64(u64::try_from(*value).map_err(|_| ConfigError::InvalidDescriptor)?)
            }
            Value::Text(value) => text(value)?,
            _ => return Err(ConfigError::InvalidDescriptor),
        };
        let bounds = field.bounds();
        let sources = [Source::File, Source::Environment, Source::Cli]
            .into_iter()
            .filter(|source| field.allows_source(*source))
            .map(|source| text(source.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        let floor = match field.security_floor() {
            SecurityFloor::None => "none",
            SecurityFloor::Fixed => "fixed",
            _ => return Err(ConfigError::InvalidDescriptor),
        };
        let redaction = match field.redaction() {
            RedactionPolicy::Public => "public",
            RedactionPolicy::Secret => "secret",
            _ => return Err(ConfigError::InvalidDescriptor),
        };
        rows.push(array(vec![
            text(field.key().as_str())?,
            text(kind)?,
            default,
            array(vec![
                unsigned(bounds.max_text_bytes)?,
                unsigned(bounds.max_list_items)?,
                unsigned(bounds.max_list_item_bytes)?,
                V::U64(
                    u64::try_from(bounds.integer_min)
                        .map_err(|_| ConfigError::InvalidDescriptor)?,
                ),
                V::U64(
                    u64::try_from(bounds.integer_max)
                        .map_err(|_| ConfigError::InvalidDescriptor)?,
                ),
            ])?,
            array(sources)?,
            V::Bool(field.reset_allowed()),
            text(floor)?,
            text(redaction)?,
            array(
                field
                    .actions()
                    .map(|action| V::U64(u64::from(action.order())))
                    .collect(),
            )?,
        ])?);
    }
    let metadata = array(vec![
        text(SECTION)?,
        text(OWNER)?,
        V::U64(2),
        text("restart-dependency")?,
        text("opaque-references-only")?,
        array(rows)?,
    ])?;
    let domain = c::CanonicalDigestDomain::parse("eliot/cbor/secrets-section-descriptor/v2")
        .map_err(canonical_error)?;
    let limit =
        c::DigestInputLimit::new(ConfigLimits::W1.max_canonical_bytes).map_err(canonical_error)?;
    c::blake3_canonical(&domain, &metadata, limit).map_err(canonical_error)
}

fn text(value: &str) -> Result<V, ConfigError> {
    c::CanonicalText::new(value.to_owned())
        .map(V::Text)
        .map_err(canonical_error)
}
fn array(values: Vec<V>) -> Result<V, ConfigError> {
    c::BoundedList::new(values)
        .map(V::Array)
        .map_err(canonical_error)
}
fn unsigned(value: usize) -> Result<V, ConfigError> {
    u64::try_from(value)
        .map(V::U64)
        .map_err(|_| ConfigError::LengthOverflow)
}
const fn canonical_error(_: c::ContractError) -> ConfigError {
    ConfigError::CanonicalBytesExceeded
}

#[cfg(test)]
#[path = "configuration/tests.rs"]
mod tests;
