//! Bounded process argument, file and environment acquisition.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use search_config::{ConfigDocument, ConfigError};

use super::activation::try_activate;
use super::capture::{
    capture_cli_document, capture_environment_document, capture_file_document,
};
use super::snapshot::{
    EffectiveDaemonConfig, build_effective, build_effective_defaults,
};
use super::spec::{
    ACTIVATION_BLOCKED, DAEMON_DIRECT_PROFILE, MAX_CAPTURED_ENTRIES,
    MAX_CAPTURED_FILE_BYTES, MAX_CAPTURED_VALUE_BYTES, config_code,
};

/// Closed CLI configuration arguments captured without secret disclosure.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliConfigArgs {
    /// Single `--config-file` path when present.
    pub config_file: Option<PathBuf>,
    /// Repeatable `--set` overrides in command-line order.
    pub overrides: Vec<(String, String)>,
}

fn parse_set(value: &str) -> Result<(String, String), String> {
    if value.is_empty() || value.len() > 4_096 + 1 + 4_096 {
        return Err("DAEMON_CONFIG_BOUNDS_EXCEEDED".to_owned());
    }
    let (dotted, cli_value) = value
        .split_once('=')
        .ok_or_else(|| "DAEMON_CONFIG_INVALID".to_owned())?;
    if dotted.is_empty() || !dotted.contains('.') {
        return Err("DAEMON_CONFIG_INVALID".to_owned());
    }
    let (section, key) = dotted
        .split_once('.')
        .ok_or_else(|| "DAEMON_CONFIG_INVALID".to_owned())?;
    if section.is_empty() || key.is_empty() || key.contains('.') || key.contains('=') {
        return Err("DAEMON_CONFIG_INVALID".to_owned());
    }
    if cli_value.is_empty() || cli_value.len() > MAX_CAPTURED_VALUE_BYTES {
        return Err("DAEMON_CONFIG_BOUNDS_EXCEEDED".to_owned());
    }
    Ok((dotted.to_owned(), cli_value.to_owned()))
}

/// Parses `--config-file <path>` and repeatable `--set <section.key=value>`
/// from already-split arguments. Other arguments are returned untouched.
pub fn parse_cli_config_args(
    args: &[String],
) -> Result<(Vec<String>, CliConfigArgs), String> {
    let mut remaining = Vec::with_capacity(args.len());
    let mut config_file = None;
    let mut overrides = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    "DAEMON_CONFIG_INVALID:missing-config-file-value".to_owned()
                })?;
                if value.is_empty() || value.len() > 32 * 1024 {
                    return Err("DAEMON_CONFIG_BOUNDS_EXCEEDED".to_owned());
                }
                if config_file.is_some() {
                    return Err("DAEMON_CONFIG_DUPLICATE".to_owned());
                }
                config_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--set" => {
                if overrides.len() >= MAX_CAPTURED_ENTRIES {
                    return Err("DAEMON_CONFIG_BOUNDS_EXCEEDED".to_owned());
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    "DAEMON_CONFIG_INVALID:missing-set-value".to_owned()
                })?;
                overrides.push(parse_set(value)?);
                index += 2;
            }
            _ => {
                remaining.push(args[index].clone());
                index += 1;
            }
        }
    }
    Ok((remaining, CliConfigArgs { config_file, overrides }))
}

/// Strips global config flags from `OsString` arguments while preserving
/// non-UTF8 non-config root paths. Config values themselves must be UTF-8.
pub fn strip_config_args_os(
    args: &[OsString],
) -> Result<(Vec<OsString>, CliConfigArgs), String> {
    let mut remaining = Vec::with_capacity(args.len());
    let mut config_file = None;
    let mut overrides = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let token = args[index].to_str();
        if token == Some("--config-file") {
            let value = args.get(index + 1).ok_or_else(|| {
                "DAEMON_CONFIG_INVALID:missing-config-file-value".to_owned()
            })?;
            let text = value
                .to_str()
                .ok_or_else(|| "DAEMON_CONFIG_INVALID".to_owned())?;
            if text.is_empty() || text.len() > 32 * 1024 {
                return Err("DAEMON_CONFIG_BOUNDS_EXCEEDED".to_owned());
            }
            if config_file.is_some() {
                return Err("DAEMON_CONFIG_DUPLICATE".to_owned());
            }
            config_file = Some(PathBuf::from(value));
            index += 2;
        } else if token == Some("--set") {
            if overrides.len() >= MAX_CAPTURED_ENTRIES {
                return Err("DAEMON_CONFIG_BOUNDS_EXCEEDED".to_owned());
            }
            let value = args.get(index + 1).ok_or_else(|| {
                "DAEMON_CONFIG_INVALID:missing-set-value".to_owned()
            })?;
            let text = value
                .to_str()
                .ok_or_else(|| "DAEMON_CONFIG_INVALID".to_owned())?;
            overrides.push(parse_set(text)?);
            index += 2;
        } else {
            remaining.push(args[index].clone());
            index += 1;
        }
    }
    Ok((remaining, CliConfigArgs { config_file, overrides }))
}

/// Reads one bounded configuration file without leaking its path.
pub fn read_config_file_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| "DAEMON_CONFIG_INVALID".to_owned())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("DAEMON_CONFIG_INVALID".to_owned());
    }
    if metadata.len() > MAX_CAPTURED_FILE_BYTES as u64 {
        return Err("DAEMON_CONFIG_BOUNDS_EXCEEDED".to_owned());
    }
    let bytes =
        std::fs::read(path).map_err(|_| "DAEMON_CONFIG_INVALID".to_owned())?;
    if bytes.len() > MAX_CAPTURED_FILE_BYTES {
        return Err("DAEMON_CONFIG_BOUNDS_EXCEEDED".to_owned());
    }
    Ok(bytes)
}

/// Captures the process environment layer from `ELIOT_SEARCH__*` variables.
pub fn capture_process_environment() -> Result<Option<ConfigDocument>, ConfigError> {
    let mut pairs = Vec::<(String, String)>::new();
    for (name, value) in std::env::vars() {
        if name.starts_with("ELIOT_SEARCH__") {
            if pairs.len() >= MAX_CAPTURED_ENTRIES
                || value.len() > MAX_CAPTURED_VALUE_BYTES
            {
                return Err(ConfigError::CapacityExceeded);
            }
            pairs.push((name, value));
        }
    }
    if pairs.is_empty() {
        return Ok(None);
    }
    let borrowed = pairs
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    capture_environment_document(&borrowed)
}

/// Builds and gates startup-effective configuration in precedence order:
/// defaults, file, environment, CLI.
pub fn effective_from_process(
    config: &CliConfigArgs,
) -> Result<EffectiveDaemonConfig, String> {
    let current = build_effective_defaults().map_err(|error| {
        let code = config_code(error);
        format!("{ACTIVATION_BLOCKED}:{code}:{error}")
    })?;
    let file = if let Some(path) = &config.config_file {
        let bytes = read_config_file_bytes(path)?;
        Some(capture_file_document(&bytes, "config-file").map_err(|error| {
            let code = config_code(error);
            format!("{code}:{error}")
        })?)
    } else {
        None
    };
    let environment = capture_process_environment().map_err(|error| {
        let code = config_code(error);
        format!("{code}:{error}")
    })?;
    let cli = if config.overrides.is_empty() {
        None
    } else {
        let borrowed = config
            .overrides
            .iter()
            .map(|(dotted, value)| (dotted.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        capture_cli_document(&borrowed).map_err(|error| {
            let code = config_code(error);
            format!("{code}:{error}")
        })?
    };
    let candidate = build_effective(
        file,
        environment,
        cli,
        DAEMON_DIRECT_PROFILE,
        DAEMON_DIRECT_PROFILE,
    )
    .map_err(|error| {
        let code = config_code(error);
        format!("{code}:{error}")
    })?;
    try_activate(&current, candidate, &BTreeSet::new())
}
