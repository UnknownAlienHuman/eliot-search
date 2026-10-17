use search_config::{
    ConfigError, ConfigFieldDescriptor, ConfigKeyName, ConfigLimits, ConfigOwner,
    ConfigSectionDescriptor, ConfigSectionName, ConfigSourceKind as Source, ConfigValue as Value,
    ConfigValueKind as Kind, ReconfigurationAction as Action, RedactionPolicy, ReloadClass,
    SecretPolicy, SecurityFloor, ValidatedSection, ValueBounds,
};
use search_contracts::{self as c, Blake3Digest32, CanonicalValue as V};
use std::num::NonZeroU64;

const SECTION: &str = "handles";
const OWNER: &str = "search-handles";
const SCHEMA_REVISION: u64 = 1;
const MAX_FINITE_INTEGER: i64 = 16_777_216;
const BOUNDS: ValueBounds = ValueBounds {
    max_text_bytes: 4096,
    max_list_items: 64,
    max_list_item_bytes: 512,
    integer_min: 0,
    integer_max: MAX_FINITE_INTEGER,
};

/// Immutable owner-validated settings for the selected handles section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedHandlesConfig {
    ephemeral_ttl_ms: u64,
    max_ephemeral_per_binding: u32,
    max_durable_per_binding: u32,
    max_expansion_bytes: u64,
    durable_unsaved_allowed: bool,
    section_digest: Blake3Digest32,
}

impl ValidatedHandlesConfig {
    /// Selected finite ephemeral handle lifetime in milliseconds.
    #[must_use]
    pub const fn ephemeral_ttl_ms(&self) -> u64 {
        self.ephemeral_ttl_ms
    }

    /// Selected maximum ephemeral handles per binding.
    #[must_use]
    pub const fn max_ephemeral_per_binding(&self) -> u32 {
        self.max_ephemeral_per_binding
    }

    /// Selected maximum durable handles per binding.
    #[must_use]
    pub const fn max_durable_per_binding(&self) -> u32 {
        self.max_durable_per_binding
    }

    /// Selected maximum bytes returned by one handle expansion.
    #[must_use]
    pub const fn max_expansion_bytes(&self) -> u64 {
        self.max_expansion_bytes
    }

    /// Whether durable handles may target unsaved content; fixed to false.
    #[must_use]
    pub const fn durable_unsaved_allowed(&self) -> bool {
        self.durable_unsaved_allowed
    }
}

/// Returns the complete owner descriptor, including source and reload policy.
pub fn section_descriptor() -> Result<ConfigSectionDescriptor, ConfigError> {
    let fields = [
        field("durable_unsaved_allowed", Value::Boolean(false))?,
        field("ephemeral_ttl_ms", Value::Integer(600_000))?,
        field("max_durable_per_binding", Value::Integer(128))?,
        field("max_ephemeral_per_binding", Value::Integer(256))?,
        field("max_expansion_bytes", Value::Integer(1_048_576))?,
    ];
    ConfigSectionDescriptor::new(
        ConfigSectionName::new(SECTION, ConfigLimits::W1.max_identifier_bytes)?,
        ConfigOwner::new(OWNER, ConfigLimits::W1.max_identifier_bytes)?,
        NonZeroU64::new(SCHEMA_REVISION).ok_or(ConfigError::InvalidDescriptor)?,
        ReloadClass::ApplyLive,
        descriptor_digest(&fields)?,
        SecretPolicy::ForbidPlaintext,
        fields,
        ConfigLimits::W1,
    )
}

fn field(name: &str, default: Value) -> Result<ConfigFieldDescriptor, ConfigError> {
    let (kind, bounds, sources, reset_allowed, floor, action) = match name {
        "durable_unsaved_allowed" => (
            Kind::Boolean,
            BOUNDS,
            vec![],
            false,
            SecurityFloor::Fixed,
            Action::Reject,
        ),
        "ephemeral_ttl_ms" => (
            Kind::Integer,
            ValueBounds {
                integer_min: 1,
                ..BOUNDS
            },
            vec![Source::File],
            false,
            SecurityFloor::IntegerMaximum(MAX_FINITE_INTEGER),
            Action::ApplyLive,
        ),
        "max_durable_per_binding" | "max_ephemeral_per_binding" | "max_expansion_bytes" => (
            Kind::Integer,
            BOUNDS,
            vec![Source::File],
            false,
            SecurityFloor::IntegerMaximum(MAX_FINITE_INTEGER),
            Action::ApplyLive,
        ),
        _ => return Err(ConfigError::UnknownKey),
    };
    ConfigFieldDescriptor::new(
        ConfigKeyName::new(name, ConfigLimits::W1.max_identifier_bytes)?,
        kind,
        default,
        bounds,
        sources,
        reset_allowed,
        floor,
        RedactionPolicy::Public,
        [action],
    )
}

/// Validates exact owner identity, shape, bounds, and file-only provenance.
pub fn validate_handles_section(
    section: &ValidatedSection,
) -> Result<ValidatedHandlesConfig, ConfigError> {
    let descriptor = section_descriptor()?;
    let actual = (
        section.section_name(),
        section.owner(),
        section.schema_revision(),
        section.field_registry_digest(),
    );
    let expected = (
        descriptor.name(),
        descriptor.owner(),
        descriptor.schema_revision(),
        descriptor.field_registry_digest(),
    );
    if actual != expected {
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

    Ok(ValidatedHandlesConfig {
        ephemeral_ttl_ms: integer(section, "ephemeral_ttl_ms")?,
        max_ephemeral_per_binding: u32::try_from(integer(section, "max_ephemeral_per_binding")?)
            .map_err(|_| ConfigError::ValueOutOfBounds)?,
        max_durable_per_binding: u32::try_from(integer(section, "max_durable_per_binding")?)
            .map_err(|_| ConfigError::ValueOutOfBounds)?,
        max_expansion_bytes: integer(section, "max_expansion_bytes")?,
        durable_unsaved_allowed: boolean(section, "durable_unsaved_allowed")?,
        section_digest: section.validated_section_digest(),
    })
}

/// Returns the validated section identity retained with the typed settings.
#[must_use]
pub const fn section_digest(config: &ValidatedHandlesConfig) -> Blake3Digest32 {
    config.section_digest
}

fn integer(section: &ValidatedSection, name: &str) -> Result<u64, ConfigError> {
    let key = ConfigKeyName::new(name, ConfigLimits::W1.max_identifier_bytes)?;
    let value = section
        .field(&key)
        .ok_or(ConfigError::MissingSection)?
        .value
        .clone();
    let Value::Integer(value) = value else {
        return Err(ConfigError::TypeMismatch);
    };
    u64::try_from(value).map_err(|_| ConfigError::ValueOutOfBounds)
}

fn boolean(section: &ValidatedSection, name: &str) -> Result<bool, ConfigError> {
    let key = ConfigKeyName::new(name, ConfigLimits::W1.max_identifier_bytes)?;
    let value = section
        .field(&key)
        .ok_or(ConfigError::MissingSection)?
        .value
        .clone();
    let Value::Boolean(value) = value else {
        return Err(ConfigError::TypeMismatch);
    };
    Ok(value)
}

fn descriptor_digest(fields: &[ConfigFieldDescriptor; 5]) -> Result<Blake3Digest32, ConfigError> {
    let mut rows = Vec::with_capacity(fields.len());
    for field in fields {
        let kind = match field.kind() {
            Kind::Boolean => "boolean",
            Kind::Integer => "integer",
            _ => return Err(ConfigError::InvalidDescriptor),
        };
        let default = match field.default() {
            Value::Boolean(value) => V::Bool(*value),
            Value::Integer(value) => {
                V::U64(u64::try_from(*value).map_err(|_| ConfigError::InvalidDescriptor)?)
            }
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
            SecurityFloor::IntegerMaximum(_) => "integer-maximum",
            _ => return Err(ConfigError::InvalidDescriptor),
        };
        let redaction = match field.redaction() {
            RedactionPolicy::Public => "public",
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
        V::U64(SCHEMA_REVISION),
        text("apply-live")?,
        text("forbid-plaintext")?,
        array(rows)?,
    ])?;
    let domain = c::CanonicalDigestDomain::parse("eliot/cbor/handles-section-descriptor/v1")
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
