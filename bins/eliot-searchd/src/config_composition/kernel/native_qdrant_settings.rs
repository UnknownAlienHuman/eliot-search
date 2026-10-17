//! Requested `[qdrant_process]` metadata over the existing typed config snapshot.
//! No artifact reads, secret resolution, process effects or readiness are implied.

use super::{
    registry::section_registry_digest,
    spec::{const_bounds, daemon_limits, key_name, owner_name, section_name},
};
use search_config::{
    ConfigError, ConfigFieldDescriptor, ConfigSectionDescriptor, ConfigSourceKind as Source,
    ConfigValue as Value, ConfigValueKind as Kind, EffectiveConfigSnapshot,
    ReconfigurationAction as Action, RedactionPolicy as Redaction, ReloadClass, SecretPolicy,
    SecretReference, SecurityFloor, ValueBounds,
};
use search_contracts::{BoundedText, Sha256Digest32};
use search_os_secrets::ValidatedSecretsConfig;
use std::{
    fmt,
    num::NonZeroU64,
    path::{Component, Path, PathBuf, Prefix},
};

const SECTION: &str = "qdrant_process";
const OWNER: &str = "search-qdrant-supervisor";

/// Shape-checked requested settings, never operational or release-qualified state.
#[derive(Clone, Eq, PartialEq)]
pub struct NativeQdrantSettings {
    /// Explicit requested enablement, with profile defaults applied to the snapshot.
    pub enabled: bool,
    /// Requested local absolute executable locator; not an opened artifact identity.
    pub artifact_path: Option<PathBuf>,
    /// Requested exact digest; not an observed hash.
    pub expected_sha256: Option<Sha256Digest32>,
    /// Requested exact version; not an observed version.
    pub expected_version: Option<BoundedText<128>>,
    /// Existing selected secret reference, or one explicit exact reference.
    pub api_secret_ref: Option<SecretReference>,
    /// Requested port; zero delegates selection to the supervisor. Binding stays loopback-only.
    pub port: u16,
    /// Original finite next-start budget in milliseconds.
    pub startup_timeout_ms: u64,
    /// Finite health request budget in milliseconds.
    pub health_timeout_ms: u64,
    /// Requested restart count, including zero for no restart.
    pub restart_max_attempts: u8,
    /// Finite restart accounting window in milliseconds.
    pub restart_window_ms: u64,
}

impl fmt::Debug for NativeQdrantSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeQdrantSettings")
            .field("enabled", &self.enabled)
            .field("identity", &"<requested; path and reference redacted>")
            .finish_non_exhaustive()
    }
}

fn identity_fields(profile: &str) -> Result<Vec<ConfigFieldDescriptor>, ConfigError> {
    let indexed = matches!(
        profile,
        "lexical" | "code" | "semantic_optional" | "document_optional"
    );
    Ok(vec![
        field(
            "enabled",
            Kind::Boolean,
            Value::Boolean(indexed),
            &[Source::File, Source::Cli],
            Redaction::Public,
            const_bounds(),
            Action::RestartDependency,
        )?,
        field(
            "artifact_path",
            Kind::Text,
            Value::Absent,
            &[Source::File],
            Redaction::PathDigest,
            const_bounds(),
            Action::RestartDependency,
        )?,
        field(
            "expected_sha256",
            Kind::Text,
            Value::Absent,
            &[Source::File],
            Redaction::Public,
            ValueBounds {
                max_text_bytes: 64,
                ..const_bounds()
            },
            Action::RestartDependency,
        )?,
        field(
            "expected_version",
            Kind::Text,
            Value::Absent,
            &[Source::File],
            Redaction::Public,
            ValueBounds {
                max_text_bytes: 128,
                ..const_bounds()
            },
            Action::RestartDependency,
        )?,
        field(
            "api_secret_ref",
            Kind::SecretReference,
            Value::Absent,
            &[Source::File, Source::Environment],
            Redaction::Secret,
            const_bounds(),
            Action::RestartDependency,
        )?,
    ])
}

pub(super) fn section_descriptor(profile: &str) -> Result<ConfigSectionDescriptor, ConfigError> {
    let mut fields = identity_fields(profile)?;
    fields.extend([
        integer("port", 0, 0, i64::from(u16::MAX), Action::RestartDependency)?,
        // Same next-start interval supported by the existing supervisor launch owner.
        integer(
            "startup_timeout_ms",
            30_000,
            1_000,
            600_000,
            Action::ApplyLive,
        )?,
        integer("health_timeout_ms", 2_000, 1, 600_000, Action::ApplyLive)?,
        integer("restart_max_attempts", 3, 0, 10, Action::ApplyLive)?,
        integer(
            "restart_window_ms",
            300_000,
            1,
            const_bounds().integer_max,
            Action::ApplyLive,
        )?,
        field(
            "auto_download",
            Kind::Boolean,
            Value::Boolean(false),
            &[],
            Redaction::Public,
            const_bounds(),
            Action::Reject,
        )?,
        field(
            "auto_upgrade",
            Kind::Boolean,
            Value::Boolean(false),
            &[],
            Redaction::Public,
            const_bounds(),
            Action::Reject,
        )?,
    ]);
    let kinds: Vec<_> = fields
        .iter()
        .map(|field| {
            (
                field.key().as_str(),
                match field.kind() {
                    Kind::Boolean => "boolean",
                    Kind::Integer => "integer",
                    Kind::Text => "text",
                    Kind::SecretReference => "secret",
                    Kind::StringList => "string-list",
                },
            )
        })
        .collect();
    ConfigSectionDescriptor::new(
        section_name(SECTION)?,
        owner_name(OWNER)?,
        NonZeroU64::MIN,
        ReloadClass::RestartDependency,
        section_registry_digest(SECTION, OWNER, 1, &kinds),
        SecretPolicy::OpaqueReferencesOnly,
        fields,
        daemon_limits(),
    )
}

#[allow(clippy::too_many_arguments)]
fn field(
    name: &str,
    kind: Kind,
    default: Value,
    sources: &[Source],
    redaction: Redaction,
    bounds: ValueBounds,
    action: Action,
) -> Result<ConfigFieldDescriptor, ConfigError> {
    let fixed = matches!(name, "auto_download" | "auto_upgrade");
    ConfigFieldDescriptor::new(
        key_name(name)?,
        kind,
        default,
        bounds,
        sources.iter().copied(),
        false,
        if fixed {
            SecurityFloor::Fixed
        } else {
            SecurityFloor::None
        },
        redaction,
        [action],
    )
}

fn integer(
    name: &str,
    default: i64,
    min: i64,
    max: i64,
    action: Action,
) -> Result<ConfigFieldDescriptor, ConfigError> {
    field(
        name,
        Kind::Integer,
        Value::Integer(default),
        &[Source::File],
        Redaction::Public,
        ValueBounds {
            integer_min: min,
            integer_max: max,
            ..const_bounds()
        },
        action,
    )
}

impl NativeQdrantSettings {
    pub(super) fn from_snapshot(
        snapshot: &EffectiveConfigSnapshot,
        secrets: &ValidatedSecretsConfig,
    ) -> Result<Self, ConfigError> {
        let section = snapshot
            .section(&section_name(SECTION)?)
            .ok_or(ConfigError::MissingSection)?;
        let value = |name| -> Result<&Value, ConfigError> {
            section
                .field(&key_name(name)?)
                .map(|field| &field.value)
                .ok_or(ConfigError::MissingSection)
        };
        let number = |name| -> Result<u64, ConfigError> {
            let Value::Integer(value) = value(name)? else {
                return Err(ConfigError::TypeMismatch);
            };
            u64::try_from(*value).map_err(|_| ConfigError::ValueOutOfBounds)
        };
        let text = |name| -> Result<Option<&str>, ConfigError> {
            match value(name)? {
                Value::Absent => Ok(None),
                Value::Text(value) => Ok(Some(value)),
                _ => Err(ConfigError::TypeMismatch),
            }
        };
        let Value::Boolean(enabled) = value("enabled")? else {
            return Err(ConfigError::TypeMismatch);
        };
        let artifact_path = text("artifact_path")?
            .map(local_artifact_path)
            .transpose()?;
        let expected_sha256 = text("expected_sha256")?
            .map(|value| {
                Sha256Digest32::parse_hex(value).map_err(|_| ConfigError::ValueOutOfBounds)
            })
            .transpose()?;
        let expected_version = text("expected_version")?
            .map(|value| {
                if value.trim() != value || value.chars().any(char::is_control) {
                    return Err(ConfigError::ValueOutOfBounds);
                }
                BoundedText::new(value).map_err(|_| ConfigError::ValueOutOfBounds)
            })
            .transpose()?;
        let explicit = match value("api_secret_ref")? {
            Value::Absent => None,
            Value::SecretReference(value) => Some(value),
            _ => return Err(ConfigError::TypeMismatch),
        };
        let selected = secrets.qdrant_api_secret_ref();
        if explicit
            .zip(selected)
            .is_some_and(|(left, right)| left != right)
        {
            return Err(ConfigError::InvalidDescriptor);
        }
        let api_secret_ref = selected.or(explicit).cloned();
        if *enabled
            && (artifact_path.is_none()
                || expected_sha256.is_none()
                || expected_version.is_none()
                || api_secret_ref.is_none())
        {
            return Err(ConfigError::MissingSection);
        }
        Ok(Self {
            enabled: *enabled,
            artifact_path,
            expected_sha256,
            expected_version,
            api_secret_ref,
            port: u16::try_from(number("port")?).map_err(|_| ConfigError::ValueOutOfBounds)?,
            startup_timeout_ms: number("startup_timeout_ms")?,
            health_timeout_ms: number("health_timeout_ms")?,
            restart_max_attempts: u8::try_from(number("restart_max_attempts")?)
                .map_err(|_| ConfigError::ValueOutOfBounds)?,
            restart_window_ms: number("restart_window_ms")?,
        })
    }
}

fn local_artifact_path(text: &str) -> Result<PathBuf, ConfigError> {
    let path = Path::new(text);
    if !path.is_absolute()
        || text.chars().any(char::is_control)
        || text.starts_with("//")
        || path.components().any(|component| match component {
            Component::ParentDir => true,
            Component::Prefix(prefix) => {
                !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
            }
            _ => false,
        })
    {
        return Err(ConfigError::ValueOutOfBounds);
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::super::{
        build_effective, build_effective_defaults, capture_cli_document,
        capture_environment_document, capture_file_document,
    };
    use super::*;

    fn identity() -> String {
        format!(
            "artifact_path='C:/synthetic-only/qdrant.exe'\nexpected_sha256='{}'\nexpected_version='fixture-1'\n",
            "ab".repeat(32)
        )
    }

    fn configured(extra: &str) -> Result<super::super::EffectiveDaemonConfig, ConfigError> {
        let file = capture_file_document(
            format!("schema_version=2\nprofile='direct'\n{extra}").as_bytes(),
            "pure-qdrant-settings-fixture",
        )?;
        build_effective(Some(file), None, None, "direct", "direct")
    }

    #[test]
    fn requested_defaults_and_exact_identity_are_metadata_only() {
        let default = build_effective_defaults().expect("direct requested defaults");
        let settings = default.native_qdrant_settings();
        assert!(!settings.enabled);
        assert!(
            settings.artifact_path.is_none()
                && settings.expected_sha256.is_none()
                && settings.expected_version.is_none()
                && settings.api_secret_ref.is_none()
        );
        assert_eq!(
            (
                settings.port,
                settings.startup_timeout_ms,
                settings.health_timeout_ms,
                settings.restart_max_attempts,
                settings.restart_window_ms
            ),
            (0, 30_000, 2_000, 3, 300_000)
        );
        let section = default
            .registry()
            .section(&section_name(SECTION).unwrap())
            .unwrap();
        assert_eq!(section.owner().as_str(), OWNER);
        assert_eq!(section.fields().len(), 12);
        for name in ["auto_download", "auto_upgrade"] {
            let field = section.field(&key_name(name).unwrap()).unwrap();
            assert_eq!(field.default(), &Value::Boolean(false));
            assert_eq!(field.security_floor(), SecurityFloor::Fixed);
            assert!(!field.allows_source(Source::File));
        }
        let config = configured(&format!("[qdrant_process]\nenabled=true\n{}api_secret_ref='secret://synthetic/exact'\nport=6334\nrestart_max_attempts=0\n", identity()))
            .expect("requested configuration with one explicit reference");
        let settings = config.native_qdrant_settings();
        assert!(settings.enabled);
        assert_eq!(
            settings.artifact_path.as_deref(),
            Some(Path::new("C:/synthetic-only/qdrant.exe"))
        );
        assert_eq!(settings.expected_sha256.unwrap().as_bytes(), &[0xab; 32]);
        assert_eq!(
            settings.expected_version.as_ref().unwrap().as_str(),
            "fixture-1"
        );
        assert_eq!(
            settings.api_secret_ref.as_ref().unwrap().as_str(),
            "secret://synthetic/exact"
        );
        assert_eq!((settings.port, settings.restart_max_attempts), (6334, 0));
        let debug = format!("{settings:?}");
        assert!(!debug.contains("synthetic-only") && !debug.contains("synthetic/exact"));

        let file = capture_file_document(format!("schema_version=2\nprofile='lexical'\n[qdrant_process]\n{}api_secret_ref='secret://synthetic/exact'\n", identity()).as_bytes(), "indexed-requested-defaults").unwrap();
        let lexical = build_effective(Some(file), None, None, "lexical", "lexical").unwrap();
        assert!(lexical.native_qdrant_settings().enabled);
        assert_eq!(
            lexical
                .snapshot()
                .section(&section_name(SECTION).unwrap())
                .unwrap()
                .field(&key_name("enabled").unwrap())
                .unwrap()
                .value,
            Value::Boolean(true)
        );
    }

    #[test]
    fn selected_secret_and_explicit_reference_cannot_diverge() {
        let input = format!(
            "[secrets]\nqdrant_api_secret_ref='secret://synthetic/selected'\n[qdrant_process]\nenabled=true\n{}",
            identity()
        );
        let inherited =
            configured(&input).expect("existing selected reference supplies the request");
        assert_eq!(
            inherited.native_qdrant_settings().api_secret_ref.as_ref(),
            inherited.secrets_settings().qdrant_api_secret_ref()
        );
        assert!(
            configured(&format!(
                "{input}api_secret_ref='secret://synthetic/selected'\n"
            ))
            .is_ok()
        );
        assert_eq!(
            configured(&format!(
                "{input}api_secret_ref='secret://synthetic/other'\n"
            )),
            Err(ConfigError::InvalidDescriptor)
        );
        let file = capture_file_document(
            format!(
                "schema_version=2\nprofile='direct'\n[qdrant_process]\nenabled=true\n{}",
                identity()
            )
            .as_bytes(),
            "pure-env-ref-fixture",
        )
        .unwrap();
        let environment = capture_environment_document(&[(
            "ELIOT_SEARCH__QDRANT_PROCESS__API_SECRET_REF",
            "secret://synthetic/from-env",
        )])
        .unwrap();
        let effective = build_effective(Some(file), environment, None, "direct", "direct").unwrap();
        assert_eq!(
            effective
                .native_qdrant_settings()
                .api_secret_ref
                .as_ref()
                .unwrap()
                .as_str(),
            "secret://synthetic/from-env"
        );
    }

    #[test]
    fn source_policies_fixed_switches_and_identity_bounds_fail_closed() {
        for body in [
            "enabled=true",
            "artifact_path='relative/qdrant.exe'",
            "artifact_path='//server/share/qdrant.exe'",
            "expected_sha256='bad'",
            "expected_version=' padded '",
            "api_secret_ref='plaintext'",
            "port=65536",
            "restart_max_attempts=11",
            "startup_timeout_ms=0",
            "health_timeout_ms=0",
            "restart_window_ms=0",
            "auto_download=true",
            "auto_upgrade=true",
            "auto_download=false",
        ] {
            assert!(
                configured(&format!("[qdrant_process]\n{body}\n")).is_err(),
                "must reject {body}"
            );
        }
        assert_eq!(
            capture_environment_document(&[("ELIOT_SEARCH__QDRANT_PROCESS__PORT", "6334")]),
            Err(ConfigError::InvalidEnvironmentKey)
        );
        let cli = capture_cli_document(&[(
            "qdrant_process.artifact_path",
            "C:/synthetic-only/other.exe",
        )])
        .unwrap();
        assert_eq!(
            build_effective(None, None, cli, "direct", "direct"),
            Err(ConfigError::OverrideNotAllowed)
        );
        let file = capture_file_document(format!("schema_version=2\nprofile='direct'\n[qdrant_process]\n{}api_secret_ref='secret://synthetic/exact'\n", identity()).as_bytes(), "pure-cli-enable-fixture").unwrap();
        let cli = capture_cli_document(&[("qdrant_process.enabled", "true")]).unwrap();
        assert!(
            build_effective(Some(file), None, cli, "direct", "direct")
                .unwrap()
                .native_qdrant_settings()
                .enabled
        );
    }
}
