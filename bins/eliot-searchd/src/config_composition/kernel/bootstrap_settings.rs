//! Private consumed bootstrap settings for direct native root commands.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const MAX_BOOTSTRAP_FILE_BYTES: usize = 64 * 1024;
const MAX_ENV_VALUE_BYTES: usize = 4_096;
const MAX_CLI_PATH_BYTES: usize = 32 * 1024;
const ENV_PREFIX: &str = "ELIOT_SEARCH__";
const DATA_ROOT_ENV: &str = "ELIOT_SEARCH__BOOTSTRAP__DATA_ROOT";

fn is_help_token(value: &OsStr) -> bool {
    value.to_str().is_some_and(|v| v == "--help" || v == "-h")
}
fn is_version_token(value: &OsStr) -> bool {
    value
        .to_str()
        .is_some_and(|v| v == "--version" || v == "-V")
}

pub struct BootstrapSettings {
    pub data_root: PathBuf,
}
pub struct BootstrapInvocation {
    pub command: String,
    pub settings: BootstrapSettings,
}

struct BootstrapLayer {
    data_root: Option<PathBuf>,
}
impl Default for BootstrapLayer {
    fn default() -> Self {
        Self { data_root: None }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLayer {
    data_root: Option<String>,
}

fn invalid(kind: &str) -> String {
    format!("BOOTSTRAP_SETTINGS_INVALID:{}", kind)
}

fn bounded_path(value: &OsStr) -> Result<PathBuf, String> {
    if value.is_empty() || value.len() > MAX_CLI_PATH_BYTES {
        return Err("BOOTSTRAP_SETTINGS_BOUNDS_EXCEEDED".to_owned());
    }
    if value.as_encoded_bytes().contains(&0) {
        return Err(invalid("path_nul"));
    }
    Ok(PathBuf::from(value))
}

fn bounded_text_path(value: &str) -> Result<PathBuf, String> {
    bounded_path(OsStr::new(value))
}

fn resolve_layer_path(layer_path: &Path, base: &Path) -> PathBuf {
    if layer_path.is_absolute() {
        layer_path.to_path_buf()
    } else {
        base.join(layer_path)
    }
}

fn capture_file_layer(config_path: &Path, config_base: &Path) -> Result<BootstrapLayer, String> {
    let metadata = std::fs::symlink_metadata(config_path).map_err(|_| invalid("config_file"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid("config_file"));
    }
    if metadata.len() > MAX_BOOTSTRAP_FILE_BYTES as u64 {
        return Err("BOOTSTRAP_SETTINGS_BOUNDS_EXCEEDED".to_owned());
    }
    let file = std::fs::File::open(config_path).map_err(|_| invalid("config_file"))?;
    let mut bytes = Vec::new();
    let mut reader = (&file).take((MAX_BOOTSTRAP_FILE_BYTES + 1) as u64);
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("config_file"))?;
    if bytes.len() > MAX_BOOTSTRAP_FILE_BYTES {
        return Err("BOOTSTRAP_SETTINGS_BOUNDS_EXCEEDED".to_owned());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid("config_utf8"))?;
    let parsed: FileLayer = toml::from_str(text).map_err(|_| invalid("config_toml"))?;
    let Some(value) = parsed.data_root else {
        return Ok(BootstrapLayer::default());
    };
    let validated = bounded_text_path(&value).map_err(|_| invalid("data_root"))?;
    Ok(BootstrapLayer {
        data_root: Some(resolve_layer_path(&validated, config_base)),
    })
}

fn capture_environment_layer(startup_cwd: &Path) -> Result<BootstrapLayer, String> {
    let mut layer = BootstrapLayer::default();
    for (name, value) in std::env::vars_os() {
        let raw_name = name.as_encoded_bytes();
        if !raw_name.starts_with(ENV_PREFIX.as_bytes()) {
            continue;
        }
        if !raw_name.starts_with(DATA_ROOT_ENV.as_bytes()) {
            return Err(invalid("environment_name"));
        }
        if raw_name != DATA_ROOT_ENV.as_bytes() {
            return Err(invalid("environment_name"));
        }
        if value.len() > MAX_ENV_VALUE_BYTES {
            return Err("BOOTSTRAP_SETTINGS_BOUNDS_EXCEEDED".to_owned());
        }
        if layer.data_root.is_some() {
            return Err(invalid("environment_duplicate"));
        }
        let validated = bounded_path(&value).map_err(|_| invalid("data_root"))?;
        layer.data_root = Some(resolve_layer_path(&validated, startup_cwd));
    }
    Ok(layer)
}

fn is_root_command(value: &OsStr) -> bool {
    matches!(
        value.to_str(),
        Some(
            "--inspect-data-root"
                | "--check-existing-root-owner"
                | "--initialize-data-root"
                | "--discard-incomplete-data-root"
        )
    )
}

pub fn looks_like_root_command(arguments: &[OsString]) -> bool {
    let mut index = 0_usize;
    let mut command: Option<&str> = None;
    let mut help_version: Option<&str> = None;
    while index < arguments.len() {
        if is_root_command(&arguments[index]) {
            if command.is_none() {
                command = arguments[index].to_str();
            }
        } else if is_help_token(&arguments[index]) {
            if help_version.is_none() {
                help_version = arguments[index].to_str();
            }
        } else if is_version_token(&arguments[index]) {
            if help_version.is_none() {
                help_version = arguments[index].to_str();
            }
        } else if arguments[index].to_str() == Some("--config-file")
            || arguments[index].to_str() == Some("--data-root")
        {
            index += 2;
            continue;
        } else if arguments[index]
            .to_str()
            .is_some_and(|item| item.starts_with("--"))
        {
            return command.is_some();
        }
        index += 1;
    }
    command.is_some() && help_version.is_none()
}

fn parse_bootstrap_cli(
    arguments: &[OsString],
) -> Result<(String, Option<PathBuf>, Option<PathBuf>), String> {
    if arguments.is_empty() {
        return Err("USAGE_ERROR".to_owned());
    }
    let mut command: Option<String> = None;
    let mut help_version: Option<String> = None;
    let mut config_file: Option<PathBuf> = None;
    let mut cli_root: Option<PathBuf> = None;
    let mut positionals: Vec<OsString> = Vec::new();
    let mut index = 0_usize;
    while index < arguments.len() {
        let token = arguments[index].to_str();
        if is_root_command(&arguments[index]) {
            if command.is_some() {
                return Err("USAGE_ERROR".to_owned());
            }
            command = Some(token.unwrap().to_owned());
            index += 1;
            continue;
        }
        if is_help_token(&arguments[index]) || is_version_token(&arguments[index]) {
            if help_version.is_none() {
                help_version = token.map(std::string::ToString::to_string);
            }
            index += 1;
            continue;
        }
        if token == Some("--config-file") {
            let value = arguments
                .get(index + 1)
                .ok_or_else(|| invalid("config_file_value"))?;
            if config_file.is_some() {
                return Err(invalid("config_file_duplicate"));
            }
            config_file = Some(bounded_path(value).map_err(|_| invalid("config_file"))?);
            index += 2;
        } else if token == Some("--data-root") {
            let value = arguments
                .get(index + 1)
                .ok_or_else(|| invalid("data_root_value"))?;
            if cli_root.is_some() {
                return Err(invalid("data_root_duplicate"));
            }
            cli_root = Some(bounded_path(value).map_err(|_| invalid("data_root"))?);
            index += 2;
        } else if token == Some("--diagnostic-limit") || token == Some("--diagnostic_limit") {
            return Err(invalid("unconsumed_field"));
        } else if token == Some("--set") {
            return Err(invalid("legacy_or_unconsumed_cli_field"));
        } else if token.is_some_and(|item| item.starts_with("--")) {
            return Err("UNKNOWN_ARGUMENT".to_owned());
        } else {
            positionals.push(arguments[index].clone());
            bounded_path(&arguments[index]).map_err(|_| "USAGE_ERROR".to_owned())?;
            index += 1;
        }
    }
    if help_version.is_some() && command.is_some() {
        return Err(invalid("conflicting_command"));
    }
    let command = command.ok_or_else(|| "USAGE_ERROR".to_owned())?;
    let is_initialize = command == "--initialize-data-root";
    if is_initialize && positionals.len() == 2 {
        return Err(invalid("legacy_init_id_unsupported"));
    }
    if positionals.len() > 1 {
        return Err("USAGE_ERROR".to_owned());
    }
    if let Some(positional) = positionals.first() {
        if cli_root.is_some() {
            return Err(invalid("data_root_duplicate"));
        }
        cli_root = Some(bounded_path(positional).map_err(|_| invalid("data_root"))?);
    }
    Ok((command, cli_root, config_file))
}

pub fn load_bootstrap_settings(arguments: &[OsString]) -> Result<BootstrapInvocation, String> {
    let startup_cwd = std::env::current_dir().map_err(|_| invalid("startup_cwd"))?;
    let (command, cli_root, config_file) = parse_bootstrap_cli(arguments)?;
    let mut file_layer = BootstrapLayer::default();
    let mut config_base = startup_cwd.clone();
    if let Some(config_path) = config_file {
        let config_path = resolve_layer_path(&config_path, &startup_cwd);
        if let Some(parent) = config_path.parent() {
            config_base = parent.to_path_buf();
        }
        file_layer = capture_file_layer(&config_path, &config_base)?;
    }
    let env_layer = capture_environment_layer(&startup_cwd)?;
    let cli_layer = BootstrapLayer {
        data_root: cli_root.map(|value| resolve_layer_path(&value, &startup_cwd)),
    };
    let data_root = cli_layer
        .data_root
        .or(env_layer.data_root)
        .or(file_layer.data_root)
        .ok_or_else(|| invalid("missing_data_root"))?;
    Ok(BootstrapInvocation {
        command,
        settings: BootstrapSettings { data_root },
    })
}
