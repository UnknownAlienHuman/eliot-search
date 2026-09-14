//! Pure captured-layer parsing, inference and validation fingerprints.

use search_config::{
    ConfigDocument, ConfigError, ConfigKeyPath, ConfigSource,
    ConfigSourceKind, ConfigValue, DocumentValue, LayerOperation,
    parse_document, validate_environment_key,
};
use search_contracts::Blake3Digest32;

use super::registry::daemon_registry;
use super::spec::{
    DAEMON_CONFIG_SCHEMA_VERSION, MAX_CAPTURED_ENTRIES,
    MAX_CAPTURED_FILE_BYTES, MAX_CAPTURED_VALUE_BYTES, RESET_MARKER,
    blake3_digest, daemon_limits, key_name, section_name, source_ref,
};

/// Parses one already-captured file byte slice through the pure document
/// parser. No filesystem read happens here.
pub fn capture_file_document(
    bytes: &[u8],
    source_label: &str,
) -> Result<ConfigDocument, ConfigError> {
    if bytes.is_empty() {
        return Err(ConfigError::EmptyInput);
    }
    if bytes.len() > MAX_CAPTURED_FILE_BYTES {
        return Err(ConfigError::CapacityExceeded);
    }
    let source = ConfigSource {
        kind: ConfigSourceKind::File,
        source_ref: source_ref(source_label)?,
        source_digest: blake3_digest(bytes),
    };
    parse_document(bytes, source, daemon_limits())
}

fn infer_value(text: &str) -> Result<DocumentValue, ConfigError> {
    if text.len() > MAX_CAPTURED_VALUE_BYTES {
        return Err(ConfigError::CapacityExceeded);
    }
    if text == "true" {
        return Ok(DocumentValue::Boolean(true));
    }
    if text == "false" {
        return Ok(DocumentValue::Boolean(false));
    }
    if let Ok(number) = text.parse::<i64>()
        && number.to_string() == text
    {
        return Ok(DocumentValue::Integer(number));
    }
    Ok(DocumentValue::Text(text.to_owned()))
}

fn infer_operation(text: &str) -> Result<LayerOperation, ConfigError> {
    if text == RESET_MARKER {
        Ok(LayerOperation::Reset)
    } else {
        infer_value(text).map(LayerOperation::Set)
    }
}

/// Builds the captured environment layer from already-read `(name, value)`
/// pairs. Unknown prefixed keys fail closed; an empty input yields no layer.
pub fn capture_environment_document(
    pairs: &[(&str, &str)],
) -> Result<Option<ConfigDocument>, ConfigError> {
    if pairs.is_empty() {
        return Ok(None);
    }
    if pairs.len() > MAX_CAPTURED_ENTRIES {
        return Err(ConfigError::CapacityExceeded);
    }
    let registry = daemon_registry()?;
    let mut encoding = Vec::from(b"eliot-searchd/environment/v1\0".as_slice());
    let mut entries = Vec::with_capacity(pairs.len());
    for (name, value) in pairs {
        let path = validate_environment_key(name, &registry)?;
        let operation = infer_operation(value)?;
        encoding.extend_from_slice(name.as_bytes());
        encoding.push(0);
        encoding.extend_from_slice(value.as_bytes());
        encoding.push(0);
        entries.push((path, operation));
    }
    let source = ConfigSource {
        kind: ConfigSourceKind::Environment,
        source_ref: source_ref("captured-environment")?,
        source_digest: blake3_digest(&encoding),
    };
    Ok(Some(ConfigDocument::from_entries(
        DAEMON_CONFIG_SCHEMA_VERSION,
        None,
        source,
        entries,
        daemon_limits(),
    )?))
}

/// Builds the captured CLI layer from already-parsed `section.key` pairs.
pub fn capture_cli_document(
    pairs: &[(&str, &str)],
) -> Result<Option<ConfigDocument>, ConfigError> {
    if pairs.is_empty() {
        return Ok(None);
    }
    if pairs.len() > MAX_CAPTURED_ENTRIES {
        return Err(ConfigError::CapacityExceeded);
    }
    let mut entries = Vec::with_capacity(pairs.len());
    for (dotted, value) in pairs {
        let (section, key) = dotted
            .split_once('.')
            .ok_or(ConfigError::InvalidIdentifier)?;
        let path = ConfigKeyPath::new(section_name(section)?, key_name(key)?);
        entries.push((path, infer_operation(value)?));
    }
    capture_cli_typed_document(entries)
}

/// Builds the captured CLI layer from already-typed operations.
pub fn capture_cli_typed_document(
    entries: Vec<(ConfigKeyPath, LayerOperation)>,
) -> Result<Option<ConfigDocument>, ConfigError> {
    if entries.is_empty() {
        return Ok(None);
    }
    if entries.len() > MAX_CAPTURED_ENTRIES {
        return Err(ConfigError::CapacityExceeded);
    }
    let mut encoding = Vec::from(b"eliot-searchd/cli-typed/v1\0".as_slice());
    for (path, operation) in &entries {
        encoding.extend_from_slice(path.section().as_str().as_bytes());
        encoding.push(b'.');
        encoding.extend_from_slice(path.key().as_str().as_bytes());
        encoding.push(0);
        match operation {
            LayerOperation::Set(DocumentValue::Boolean(value)) => {
                encoding.extend_from_slice(value.to_string().as_bytes());
            }
            LayerOperation::Set(DocumentValue::Integer(value)) => {
                encoding.extend_from_slice(value.to_string().as_bytes());
            }
            LayerOperation::Set(DocumentValue::Text(value)) => {
                if value.len() > MAX_CAPTURED_VALUE_BYTES {
                    return Err(ConfigError::CapacityExceeded);
                }
                encoding.extend_from_slice(value.as_bytes());
            }
            LayerOperation::Set(DocumentValue::StringList(_)) => {
                return Err(ConfigError::ValueOutOfBounds);
            }
            LayerOperation::Reset => encoding.extend_from_slice(b"__RESET__"),
        }
        encoding.push(0);
    }
    let source = ConfigSource {
        kind: ConfigSourceKind::Cli,
        source_ref: source_ref("captured-cli")?,
        source_digest: blake3_digest(&encoding),
    };
    Ok(Some(ConfigDocument::from_entries(
        DAEMON_CONFIG_SCHEMA_VERSION,
        None,
        source,
        entries,
        daemon_limits(),
    )?))
}

fn encode_config_value(value: &ConfigValue, encoding: &mut Vec<u8>) {
    match value {
        ConfigValue::Absent => encoding.extend_from_slice(b"absent\0"),
        ConfigValue::Boolean(flag) => {
            encoding.extend_from_slice(b"boolean:");
            encoding.push(u8::from(*flag));
            encoding.push(0);
        }
        ConfigValue::Integer(number) => {
            encoding.extend_from_slice(b"integer:");
            encoding.extend_from_slice(&number.to_be_bytes());
            encoding.push(0);
        }
        ConfigValue::Text(text) => {
            encoding.extend_from_slice(b"text:");
            encoding.extend_from_slice(text.as_bytes());
            encoding.push(0);
        }
        ConfigValue::SecretReference(reference) => {
            encoding.extend_from_slice(b"secret:");
            encoding.extend_from_slice(reference.as_str().as_bytes());
            encoding.push(0);
        }
        ConfigValue::StringList(items) => {
            encoding.extend_from_slice(b"list:");
            for item in items {
                encoding.extend_from_slice(item.as_bytes());
                encoding.push(0);
            }
            encoding.push(0);
        }
    }
}

/// Capability-structural validation digest over one projected section.
pub(super) fn validation_digest(
    input: &search_config::ConfigSectionInput,
) -> Blake3Digest32 {
    let mut encoding = Vec::new();
    encoding.extend_from_slice(b"eliot-searchd/section-validation/v1\0");
    encoding.extend_from_slice(input.section_name().as_str().as_bytes());
    encoding.push(0);
    encoding.extend_from_slice(input.owner().as_str().as_bytes());
    encoding.push(0);
    encoding.extend_from_slice(&input.schema_revision().get().to_be_bytes());
    for (key, field) in input.fields() {
        encoding.extend_from_slice(key.as_str().as_bytes());
        encoding.push(0);
        encode_config_value(&field.value, &mut encoding);
    }
    blake3_digest(&encoding)
}
