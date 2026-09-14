//! Closed daemon-observed configuration registry.

use std::num::NonZeroU64;

use search_config::{
    ConfigError, ConfigFieldDescriptor, ConfigRegistry,
    ConfigSectionDescriptor, ConfigSourceKind, ConfigValue, ConfigValueKind,
    ReconfigurationAction, RedactionPolicy, ReloadClass, SecretPolicy,
    SecurityFloor, register_sections,
};
use search_contracts::Blake3Digest32;

use super::spec::{
    DAEMON_CONFIG_SCHEMA_VERSION, blake3_digest, const_bounds, daemon_limits,
    key_name, owner_name, section_name,
};

/// Deterministic field-registry digest over the exact declared fields.
fn section_registry_digest(
    section: &str,
    owner: &str,
    revision: u64,
    fields: &[(&str, &str)],
) -> Blake3Digest32 {
    let mut encoding = Vec::new();
    encoding.extend_from_slice(b"eliot-searchd/section-registry/v1\0");
    encoding.extend_from_slice(section.as_bytes());
    encoding.push(0);
    encoding.extend_from_slice(owner.as_bytes());
    encoding.push(0);
    encoding.extend_from_slice(&revision.to_be_bytes());
    for (key, kind) in fields {
        encoding.extend_from_slice(key.as_bytes());
        encoding.push(0);
        encoding.extend_from_slice(kind.as_bytes());
        encoding.push(0);
    }
    blake3_digest(&encoding)
}

struct FieldSpec<'a> {
    key: &'a str,
    kind: ConfigValueKind,
    default: ConfigValue,
    allowed: &'a [ConfigSourceKind],
    reset_allowed: bool,
    floor: SecurityFloor,
    redaction: RedactionPolicy,
    actions: &'a [ReconfigurationAction],
}

fn make_field(spec: FieldSpec<'_>) -> Result<ConfigFieldDescriptor, ConfigError> {
    ConfigFieldDescriptor::new(
        key_name(spec.key)?,
        spec.kind,
        spec.default,
        const_bounds(),
        spec.allowed.iter().copied(),
        spec.reset_allowed,
        spec.floor,
        spec.redaction,
        spec.actions.iter().copied(),
    )
}

/// Closed daemon-observed registry.
///
/// Mirrors `config/sections.toml` for the daemon-observed subset. Every
/// reload/obligation class the daemon must gate appears at least once.
pub fn daemon_registry() -> Result<ConfigRegistry, ConfigError> {
    register_sections(
        DAEMON_CONFIG_SCHEMA_VERSION,
        [
            instance_section()?,
            secrets_section()?,
            control_section()?,
            admission_section()?,
            lexical_section()?,
            query_section()?,
            optional_section()?,
        ],
        daemon_limits(),
    )
}

fn instance_section() -> Result<ConfigSectionDescriptor, ConfigError> {
    ConfigSectionDescriptor::new(
        section_name("instance")?,
        owner_name("search-runtime-owner")?,
        NonZeroU64::MIN,
        ReloadClass::DrainAndRestart,
        section_registry_digest(
            "instance",
            "search-runtime-owner",
            1,
            &[("mode", "text"), ("data_root", "text")],
        ),
        SecretPolicy::ForbidPlaintext,
        [
            make_field(FieldSpec {
                key: "mode",
                kind: ConfigValueKind::Text,
                default: ConfigValue::Text("standalone".to_owned()),
                allowed: &[ConfigSourceKind::File, ConfigSourceKind::Cli],
                reset_allowed: true,
                floor: SecurityFloor::None,
                redaction: RedactionPolicy::Public,
                actions: &[ReconfigurationAction::DrainAndRestart],
            })?,
            make_field(FieldSpec {
                key: "data_root",
                kind: ConfigValueKind::Text,
                default: ConfigValue::Absent,
                allowed: &[
                    ConfigSourceKind::File,
                    ConfigSourceKind::Environment,
                    ConfigSourceKind::Cli,
                ],
                reset_allowed: true,
                floor: SecurityFloor::None,
                redaction: RedactionPolicy::PathDigest,
                actions: &[ReconfigurationAction::DrainAndRestart],
            })?,
        ],
        daemon_limits(),
    )
}

fn secrets_section() -> Result<ConfigSectionDescriptor, ConfigError> {
    ConfigSectionDescriptor::new(
        section_name("secrets")?,
        owner_name("search-os-secrets")?,
        NonZeroU64::MIN,
        ReloadClass::RestartDependency,
        section_registry_digest(
            "secrets",
            "search-os-secrets",
            1,
            &[("qdrant_api_secret_ref", "secret")],
        ),
        SecretPolicy::OpaqueReferencesOnly,
        [make_field(FieldSpec {
            key: "qdrant_api_secret_ref",
            kind: ConfigValueKind::SecretReference,
            default: ConfigValue::Absent,
            allowed: &[ConfigSourceKind::File, ConfigSourceKind::Environment],
            reset_allowed: true,
            floor: SecurityFloor::None,
            redaction: RedactionPolicy::Secret,
            actions: &[ReconfigurationAction::RestartDependency],
        })?],
        daemon_limits(),
    )
}

fn control_section() -> Result<ConfigSectionDescriptor, ConfigError> {
    ConfigSectionDescriptor::new(
        section_name("control")?,
        owner_name("search-control-redb")?,
        NonZeroU64::MIN,
        ReloadClass::RestartDependency,
        section_registry_digest(
            "control",
            "search-control-redb",
            1,
            &[("durability", "text"), ("migration_epoch", "integer")],
        ),
        SecretPolicy::ForbidPlaintext,
        [
            make_field(FieldSpec {
                key: "durability",
                kind: ConfigValueKind::Text,
                default: ConfigValue::Text("fsync_atomic".to_owned()),
                allowed: &[ConfigSourceKind::File],
                reset_allowed: false,
                floor: SecurityFloor::Fixed,
                redaction: RedactionPolicy::Public,
                actions: &[ReconfigurationAction::Reject],
            })?,
            make_field(FieldSpec {
                key: "migration_epoch",
                kind: ConfigValueKind::Integer,
                default: ConfigValue::Integer(1),
                allowed: &[ConfigSourceKind::File],
                reset_allowed: false,
                floor: SecurityFloor::IntegerMinimum(1),
                redaction: RedactionPolicy::Public,
                actions: &[ReconfigurationAction::MigrateControlSchema],
            })?,
        ],
        daemon_limits(),
    )
}

fn admission_section() -> Result<ConfigSectionDescriptor, ConfigError> {
    ConfigSectionDescriptor::new(
        section_name("source_admission")?,
        owner_name("search-source-admission")?,
        NonZeroU64::MIN,
        ReloadClass::SecurityBarrier,
        section_registry_digest(
            "source_admission",
            "search-source-admission",
            1,
            &[("allow_generated", "boolean")],
        ),
        SecretPolicy::ForbidPlaintext,
        [make_field(FieldSpec {
            key: "allow_generated",
            kind: ConfigValueKind::Boolean,
            default: ConfigValue::Boolean(true),
            allowed: &[ConfigSourceKind::File],
            reset_allowed: true,
            floor: SecurityFloor::BooleanMayOnlyRestrict,
            redaction: RedactionPolicy::Public,
            actions: &[ReconfigurationAction::SecurityBarrier],
        })?],
        daemon_limits(),
    )
}

fn lexical_section() -> Result<ConfigSectionDescriptor, ConfigError> {
    ConfigSectionDescriptor::new(
        section_name("lexical")?,
        owner_name("search-lexical")?,
        NonZeroU64::MIN,
        ReloadClass::NewCollectionGeneration,
        section_registry_digest(
            "lexical",
            "search-lexical",
            1,
            &[("profile_id", "text")],
        ),
        SecretPolicy::ForbidPlaintext,
        [make_field(FieldSpec {
            key: "profile_id",
            kind: ConfigValueKind::Text,
            default: ConfigValue::Text("baseline-v1".to_owned()),
            allowed: &[ConfigSourceKind::File],
            reset_allowed: true,
            floor: SecurityFloor::None,
            redaction: RedactionPolicy::Public,
            actions: &[
                ReconfigurationAction::NewCollectionGeneration,
                ReconfigurationAction::RebuildProjection,
            ],
        })?],
        daemon_limits(),
    )
}

fn query_section() -> Result<ConfigSectionDescriptor, ConfigError> {
    ConfigSectionDescriptor::new(
        section_name("query")?,
        owner_name("search-query-planner")?,
        NonZeroU64::MIN,
        ReloadClass::ApplyLive,
        section_registry_digest(
            "query",
            "search-query-planner",
            1,
            &[("limit", "integer")],
        ),
        SecretPolicy::ForbidPlaintext,
        [make_field(FieldSpec {
            key: "limit",
            kind: ConfigValueKind::Integer,
            default: ConfigValue::Integer(10),
            allowed: &[
                ConfigSourceKind::File,
                ConfigSourceKind::Environment,
                ConfigSourceKind::Cli,
            ],
            reset_allowed: true,
            floor: SecurityFloor::None,
            redaction: RedactionPolicy::Public,
            actions: &[ReconfigurationAction::ApplyLive],
        })?],
        daemon_limits(),
    )
}

fn optional_section() -> Result<ConfigSectionDescriptor, ConfigError> {
    ConfigSectionDescriptor::new(
        section_name("optional_profiles")?,
        owner_name("eliot-searchd")?,
        NonZeroU64::MIN,
        ReloadClass::GateRequired,
        section_registry_digest(
            "optional_profiles",
            "eliot-searchd",
            1,
            &[("semantic", "boolean")],
        ),
        SecretPolicy::OpaqueReferencesOnly,
        [make_field(FieldSpec {
            key: "semantic",
            kind: ConfigValueKind::Boolean,
            default: ConfigValue::Boolean(false),
            allowed: &[ConfigSourceKind::File],
            reset_allowed: true,
            floor: SecurityFloor::None,
            redaction: RedactionPolicy::Public,
            actions: &[ReconfigurationAction::GateRequired],
        })?],
        daemon_limits(),
    )
}
