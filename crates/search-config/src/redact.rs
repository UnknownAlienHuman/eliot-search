//! Bounded non-disclosing effective-configuration projection.

use std::collections::BTreeMap;

use search_contracts::{ProfileId, Sha256Digest32};

use crate::fingerprint::sha256_digest;

use crate::{
    ConfigFingerprint, ConfigKeyPath, ConfigLimits, ConfigRegistry, ConfigValue, ConfigValueKind,
    EffectiveConfigSnapshot, RedactionPolicy,
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
    /// Path location class plus digest; raw path is excluded.
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

/// Produces a bounded non-disclosing view.
///
/// Secret references are always hidden, and path-like fields are always reduced
/// to location class plus digest even under privileged metadata disclosure.
#[must_use]
pub fn redacted_view(
    snapshot: &EffectiveConfigSnapshot,
    registry: &ConfigRegistry,
    disclosure: DisclosureLevel,
    limits: ConfigLimits,
) -> RedactedConfigView {
    let max_entries = limits.max_diagnostic_entries;
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
            if entries.len() >= max_entries {
                omitted_entries = omitted_entries.saturating_add(1);
                continue;
            }
            let path = ConfigKeyPath::new(section_name.clone(), key.clone());
            entries.insert(
                path,
                redact_value(
                    &field.value,
                    field_descriptor.redaction(),
                    visible_text_bytes,
                ),
            );
        }
    }

    RedactedConfigView {
        fingerprint: snapshot.fingerprint(),
        selected_profile: snapshot.selected_profile().clone(),
        entries,
        omitted_entries,
    }
}

fn redact_value(
    value: &ConfigValue,
    policy: RedactionPolicy,
    max_visible_text_bytes: usize,
) -> RedactedValue {
    if matches!(value, ConfigValue::Absent) {
        return RedactedValue::Absent;
    }
    if matches!(value, ConfigValue::SecretReference(_)) || policy == RedactionPolicy::Secret {
        return RedactedValue::SecretHidden;
    }
    match policy {
        RedactionPolicy::PathDigest => {
            let text = match value {
                ConfigValue::Text(text) => text.as_str(),
                _ => "",
            };
            RedactedValue::PathDigest {
                class: classify_path(text),
                digest: sha256_digest(text.as_bytes()),
            }
        }
        RedactionPolicy::MetadataOnly => {
            value
                .kind()
                .map_or(RedactedValue::Absent, |kind| RedactedValue::Metadata {
                    kind,
                    extent: value_extent(value),
                })
        }
        RedactionPolicy::Public => match value {
            ConfigValue::Absent => RedactedValue::Absent,
            ConfigValue::Boolean(value) => RedactedValue::Boolean(*value),
            ConfigValue::Integer(value) => RedactedValue::Integer(*value),
            ConfigValue::Text(value) => {
                let (value, truncated) = truncate_utf8(value, max_visible_text_bytes);
                RedactedValue::Text { value, truncated }
            }
            ConfigValue::StringList(values) => RedactedValue::StringList {
                items: values.len(),
                total_bytes: values
                    .iter()
                    .fold(0_usize, |total, value| total.saturating_add(value.len())),
            },
            ConfigValue::SecretReference(_) => RedactedValue::SecretHidden,
        },
        RedactionPolicy::Secret => RedactedValue::SecretHidden,
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

fn truncate_utf8(value: &str, max_bytes: usize) -> (String, bool) {
    if value.len() <= max_bytes {
        return (value.to_owned(), false);
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    (value[..end].to_owned(), true)
}

#[cfg(test)]
mod tests {
    use super::{PathLocationClass, classify_path, truncate_utf8};

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
}
