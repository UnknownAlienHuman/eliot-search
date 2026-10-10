//! Bounded non-disclosing effective-configuration projection.

use std::collections::BTreeMap;

use search_contracts::{ProfileId, Sha256Digest32};

use crate::fingerprint::sha256_digest;

use crate::{
    ConfigError, ConfigFingerprint, ConfigKeyPath, ConfigLimits, ConfigRegistry, ConfigValue,
    ConfigValueKind, EffectiveConfigSnapshot, RedactionPolicy,
};

/// Requested ordinary diagnostic disclosure level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisclosureLevel {
    /// Ordinary logs and status output.
    Ordinary,
    /// Privileged metadata output; secret references and raw paths remain hidden.
    PrivilegedMetadata,
}

/// Non-content path location class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PathLocationClass {
    /// Relative path spelling.
    Relative,
    /// Local absolute POSIX or drive-rooted path.
    AbsoluteLocal,
    /// UNC, URI, or other potentially remote location.
    NetworkOrUri,
    /// Empty or otherwise unclassifiable value.
    Unknown,
}

/// Redacted field representation safe for ordinary diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RedactedValue {
    /// Field is intentionally absent.
    Absent,
    /// Public boolean.
    Boolean(bool),
    /// Public integer.
    Integer(i64),
    /// Public finite text, possibly deterministically truncated.
    Text {
        /// Visible text prefix.
        value: String,
        /// Whether bytes were omitted.
        truncated: bool,
    },
    /// String-list metadata only.
    StringList {
        /// Number of list entries.
        items: usize,
        /// Sum of UTF-8 bytes.
        total_bytes: usize,
    },
    /// Path location class plus exact bounded digest; raw path is excluded.
    PathDigest {
        /// Coarse non-content location class.
        class: PathLocationClass,
        /// SHA-256 digest of the exact path spelling.
        digest: Sha256Digest32,
    },
    /// Opaque secret reference hidden completely.
    SecretHidden,
    /// Value kind and size/count metadata only.
    Metadata {
        /// Closed value kind.
        kind: ConfigValueKind,
        /// Byte length or item count.
        extent: usize,
    },
}

/// Bounded redacted view of one effective snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactedConfigView {
    /// Configuration fingerprint.
    pub fingerprint: ConfigFingerprint,
    /// Selected profile.
    pub selected_profile: ProfileId,
    /// Canonically ordered visible entries.
    pub entries: BTreeMap<ConfigKeyPath, RedactedValue>,
    /// Number of entries omitted by the diagnostic ceiling.
    pub omitted_entries: usize,
}

/// Produces a bounded non-disclosing view, charging before owned clones or hashing.
///
/// Secret references are always hidden, and path-like fields are always reduced
/// to location class plus digest even under privileged metadata disclosure.
/// The byte ceiling covers logical header/name/value payload, with fixed-width
/// scalar extents; it is not a JSON/Debug or allocator-resident byte measurement.
///
/// # Errors
/// Returns invalid limits, length overflow, or insufficient header capacity.
/// Entries exceeding either ceiling are omitted whole, in canonical iteration order.
pub fn redacted_view(
    snapshot: &EffectiveConfigSnapshot,
    registry: &ConfigRegistry,
    disclosure: DisclosureLevel,
    limits: ConfigLimits,
) -> Result<RedactedConfigView, ConfigError> {
    let limits = limits.validate()?;
    // Complete tagged fingerprint payload and omitted count (8), plus profile UTF-8.
    let mut charged_bytes = snapshot
        .fingerprint()
        .logical_payload_bytes()
        .checked_add(8)
        .ok_or(ConfigError::LengthOverflow)?
        .checked_add(snapshot.selected_profile().as_str().len())
        .ok_or(ConfigError::LengthOverflow)?;
    if charged_bytes > limits.max_diagnostic_bytes {
        return Err(ConfigError::CapacityExceeded);
    }
    let visible_text_bytes = match disclosure {
        DisclosureLevel::Ordinary => limits.max_text_bytes.min(256),
        DisclosureLevel::PrivilegedMetadata => limits.max_text_bytes.min(1_024),
    };
    let mut entries = BTreeMap::new();
    let mut omitted_entries = 0_usize;

    for (section_name, section) in snapshot.sections() {
        let Some(descriptor) = registry.section(section_name) else {
            continue;
        };
        for (key, field_descriptor) in descriptor.fields() {
            let Some(field) = section.field(key) else {
                continue;
            };
            if entries.len() >= limits.max_diagnostic_entries {
                omitted_entries = omitted_entries.saturating_add(1);
                continue;
            }
            let prepared = prepare_value(
                &field.value,
                field_descriptor.redaction(),
                visible_text_bytes,
            );
            let next_bytes = diagnostic_extent(
                charged_bytes,
                section_name.as_str().len(),
                key.as_str().len(),
                prepared.payload_bytes()?,
            )?;
            if next_bytes > limits.max_diagnostic_bytes {
                omitted_entries = omitted_entries.saturating_add(1);
                continue;
            }
            charged_bytes = next_bytes;
            let path = ConfigKeyPath::new(section_name.clone(), key.clone());
            entries.insert(path, redact_value(prepared));
        }
    }

    Ok(RedactedConfigView {
        fingerprint: snapshot.fingerprint(),
        selected_profile: snapshot.selected_profile().clone(),
        entries,
        omitted_entries,
    })
}

fn diagnostic_extent(
    current: usize,
    section_bytes: usize,
    key_bytes: usize,
    value_bytes: usize,
) -> Result<usize, ConfigError> {
    current
        .checked_add(section_bytes)
        .and_then(|bytes| bytes.checked_add(key_bytes))
        .and_then(|bytes| bytes.checked_add(value_bytes))
        .ok_or(ConfigError::LengthOverflow)
}

// Borrow source text until the whole diagnostic entry has been admitted.
#[derive(Clone, Copy)]
enum PreparedRedaction<'a> {
    Absent,
    Boolean(bool),
    Integer(i64),
    Text {
        value: &'a str,
        truncated: bool,
    },
    StringList {
        items: usize,
        total_bytes: usize,
    },
    PathDigest {
        value: &'a str,
        class: PathLocationClass,
    },
    SecretHidden,
    Metadata {
        kind: ConfigValueKind,
        extent: usize,
    },
}

impl PreparedRedaction<'_> {
    fn payload_bytes(&self) -> Result<usize, ConfigError> {
        let payload = match self {
            Self::Absent | Self::SecretHidden => 0,
            Self::Boolean(_) => 1,
            Self::Integer(_) => 8,
            Self::Text { value, .. } => value
                .len()
                .checked_add(1)
                .ok_or(ConfigError::LengthOverflow)?,
            Self::StringList { .. } => 16,
            Self::PathDigest { .. } => 33,
            Self::Metadata { .. } => 9,
        };
        payload.checked_add(1).ok_or(ConfigError::LengthOverflow)
    }
}

fn prepare_value(
    value: &ConfigValue,
    policy: RedactionPolicy,
    max_visible_text_bytes: usize,
) -> PreparedRedaction<'_> {
    if matches!(value, ConfigValue::Absent) {
        return PreparedRedaction::Absent;
    }
    if matches!(value, ConfigValue::SecretReference(_)) || policy == RedactionPolicy::Secret {
        return PreparedRedaction::SecretHidden;
    }
    match policy {
        RedactionPolicy::PathDigest => {
            let text = match value {
                ConfigValue::Text(text) => text.as_str(),
                _ => "",
            };
            if text.len() > search_contracts::MAX_CANONICAL_BYTES {
                PreparedRedaction::Metadata {
                    kind: ConfigValueKind::Text,
                    extent: text.len(),
                }
            } else {
                PreparedRedaction::PathDigest {
                    value: text,
                    class: classify_path(text),
                }
            }
        }
        RedactionPolicy::MetadataOnly => value.kind().map_or(PreparedRedaction::Absent, |kind| {
            PreparedRedaction::Metadata {
                kind,
                extent: value_extent(value),
            }
        }),
        RedactionPolicy::Public => match value {
            ConfigValue::Absent => PreparedRedaction::Absent,
            ConfigValue::Boolean(value) => PreparedRedaction::Boolean(*value),
            ConfigValue::Integer(value) => PreparedRedaction::Integer(*value),
            ConfigValue::Text(value) => {
                let (value, truncated) = truncate_utf8(value, max_visible_text_bytes);
                PreparedRedaction::Text { value, truncated }
            }
            ConfigValue::StringList(values) => PreparedRedaction::StringList {
                items: values.len(),
                total_bytes: values
                    .iter()
                    .fold(0_usize, |total, value| total.saturating_add(value.len())),
            },
            ConfigValue::SecretReference(_) => PreparedRedaction::SecretHidden,
        },
        RedactionPolicy::Secret => PreparedRedaction::SecretHidden,
    }
}

fn redact_value(value: PreparedRedaction<'_>) -> RedactedValue {
    match value {
        PreparedRedaction::Absent => RedactedValue::Absent,
        PreparedRedaction::Boolean(value) => RedactedValue::Boolean(value),
        PreparedRedaction::Integer(value) => RedactedValue::Integer(value),
        PreparedRedaction::Text { value, truncated } => RedactedValue::Text {
            value: value.to_owned(),
            truncated,
        },
        PreparedRedaction::StringList { items, total_bytes } => {
            RedactedValue::StringList { items, total_bytes }
        }
        PreparedRedaction::PathDigest { value, class } => sha256_digest(value.as_bytes()).map_or(
            RedactedValue::Metadata {
                kind: ConfigValueKind::Text,
                extent: value.len(),
            },
            |digest| RedactedValue::PathDigest { class, digest },
        ),
        PreparedRedaction::SecretHidden => RedactedValue::SecretHidden,
        PreparedRedaction::Metadata { kind, extent } => RedactedValue::Metadata { kind, extent },
    }
}

const fn value_extent(value: &ConfigValue) -> usize {
    match value {
        ConfigValue::Absent | ConfigValue::SecretReference(_) => 0,
        ConfigValue::Boolean(_) => 1,
        ConfigValue::Integer(_) => std::mem::size_of::<i64>(),
        ConfigValue::Text(value) => value.len(),
        ConfigValue::StringList(values) => values.len(),
    }
}

fn classify_path(value: &str) -> PathLocationClass {
    if value.is_empty() {
        PathLocationClass::Unknown
    } else if value.contains("://") || value.starts_with("\\\\") || value.starts_with("//") {
        PathLocationClass::NetworkOrUri
    } else if value.starts_with('/')
        || value
            .as_bytes()
            .get(1)
            .is_some_and(|separator| *separator == b':')
    {
        PathLocationClass::AbsoluteLocal
    } else {
        PathLocationClass::Relative
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> (&str, bool) {
    if value.len() <= max_bytes {
        return (value, false);
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    (&value[..end], true)
}

#[cfg(test)]
mod tests {
    use super::{
        PathLocationClass, RedactedValue, classify_path, diagnostic_extent, prepare_value,
        truncate_utf8,
    };
    use crate::{ConfigValue, ConfigValueKind, RedactionPolicy, SecretReference};
    use search_contracts::MAX_CANONICAL_BYTES;

    fn redact_value(value: &ConfigValue, policy: RedactionPolicy, cap: usize) -> RedactedValue {
        super::redact_value(prepare_value(value, policy, cap))
    }

    // #394 source-authored overflow proof; AUTHORED_NOT_RUN.
    #[test]
    fn diagnostic_extent_overflow_returns_no_partial_extent() {
        assert_eq!(
            diagnostic_extent(usize::MAX, 1, 0, 0),
            Err(crate::ConfigError::LengthOverflow)
        );
        assert_eq!(diagnostic_extent(40, 1, 1, 2), Ok(44));
    }

    // Logical scalar/list/path costs and borrowed UTF-8 preparation, AUTHORED_NOT_RUN.
    #[test]
    fn prepared_redaction_charges_closed_payload_and_preserves_borrowed_text() {
        for (value, policy, expected) in [
            (ConfigValue::Absent, RedactionPolicy::Public, 1),
            (ConfigValue::Boolean(true), RedactionPolicy::Public, 2),
            (ConfigValue::Integer(-1), RedactionPolicy::Public, 9),
            (
                ConfigValue::StringList(vec!["abc".into()]),
                RedactionPolicy::Public,
                17,
            ),
            (
                ConfigValue::Text("relative/path".into()),
                RedactionPolicy::PathDigest,
                34,
            ),
            (
                ConfigValue::Text("private".into()),
                RedactionPolicy::MetadataOnly,
                10,
            ),
        ] {
            assert_eq!(
                prepare_value(&value, policy, 256).payload_bytes(),
                Ok(expected)
            );
        }
        let value = ConfigValue::Text("\u{e9}\u{e9}".into());
        let prepared = prepare_value(&value, RedactionPolicy::Public, 3);
        assert_eq!(prepared.payload_bytes(), Ok(4)); // tag + 2 UTF-8 bytes + truncation flag
        assert_eq!(
            super::redact_value(prepared),
            RedactedValue::Text {
                value: "\u{e9}".into(),
                truncated: true,
            }
        );
    }

    #[test]
    fn path_classification_never_needs_io() {
        assert_eq!(
            classify_path("C:/Users/example"),
            PathLocationClass::AbsoluteLocal
        );
        assert_eq!(
            classify_path("//server/share"),
            PathLocationClass::NetworkOrUri
        );
        assert_eq!(classify_path("relative/path"), PathLocationClass::Relative);
    }

    #[test]
    fn truncation_preserves_utf8_boundaries() {
        let (value, truncated) = truncate_utf8("éé", 3);
        assert_eq!(value, "é");
        assert!(truncated);
    }

    // #386 focused compatibility/disclosure fixtures. AUTHORED_NOT_RUN.
    #[test]
    fn path_digest_keeps_exact_standard_sha_without_prefixing() {
        let value = ConfigValue::Text("abc".to_owned());
        let RedactedValue::PathDigest { class, digest } =
            redact_value(&value, RedactionPolicy::PathDigest, 1)
        else {
            panic!("bounded path must have a digest")
        };
        assert_eq!(class, PathLocationClass::Relative);
        assert_eq!(
            digest.to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn oversized_path_yields_only_extent_and_never_an_identity_or_text() {
        let value = ConfigValue::Text("private-path-marker/".repeat(MAX_CANONICAL_BYTES / 20 + 1));
        let extent = match &value {
            ConfigValue::Text(text) => text.len(),
            _ => unreachable!(),
        };
        assert!(extent > MAX_CANONICAL_BYTES);
        let result = redact_value(&value, RedactionPolicy::PathDigest, usize::MAX);
        assert_eq!(
            result,
            RedactedValue::Metadata {
                kind: ConfigValueKind::Text,
                extent
            }
        );
        let debug = format!("{result:?}");
        assert!(!debug.contains("private-path-marker"));
        assert!(!debug.contains("PathDigest"));
    }

    #[test]
    fn exact_hard_ceiling_path_still_has_a_digest() {
        let value = ConfigValue::Text("x".repeat(MAX_CANONICAL_BYTES));
        assert!(matches!(
            redact_value(&value, RedactionPolicy::PathDigest, 1),
            RedactedValue::PathDigest { .. }
        ));
    }

    #[test]
    fn secret_policy_precedes_path_digest_and_other_policies_stay_unchanged() {
        let secret = ConfigValue::SecretReference(
            SecretReference::new("secret://synthetic-ref", 64).expect("reference"),
        );
        assert_eq!(
            redact_value(&secret, RedactionPolicy::PathDigest, 1),
            RedactedValue::SecretHidden
        );
        let value = ConfigValue::Text("abc".to_owned());
        assert_eq!(
            redact_value(&value, RedactionPolicy::Secret, 1),
            RedactedValue::SecretHidden
        );
        assert_eq!(
            redact_value(&value, RedactionPolicy::Public, 1),
            RedactedValue::Text {
                value: "a".to_owned(),
                truncated: true
            }
        );
        assert_eq!(
            redact_value(&value, RedactionPolicy::MetadataOnly, 1),
            RedactedValue::Metadata {
                kind: ConfigValueKind::Text,
                extent: 3
            }
        );
    }
}
