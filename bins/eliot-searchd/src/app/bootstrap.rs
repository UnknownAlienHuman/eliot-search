//! One native parse and typed settings load for root operator commands.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use search_contracts::SourceId;
use serde::Deserialize;

use super::{PublicationRequest, RootCommand, RootInvocation};

const MAX_CONFIG_BYTES: usize = 64 * 1024;
const MAX_ENV_BYTES: usize = 4 * 1024;
const MAX_PATH_BYTES: usize = 32 * 1024;
const ENV_PREFIX: &[u8] = b"ELIOT_SEARCH__";
const DATA_ROOT_ENV: &[u8] = b"ELIOT_SEARCH__BOOTSTRAP__DATA_ROOT";

pub(super) enum Parsed {
    Help,
    Version,
    Root(Box<RootCli>),
}

pub(super) struct RootCli {
    command: RootCommand,
    data_root: Option<PathBuf>,
    config_file: Option<PathBuf>,
    source_file: Option<PathBuf>,
    index_path: Option<PathBuf>,
    source_id: Option<SourceId>,
    unit: Option<usize>,
    publication: Option<PublicationRequest>,
    search_endpoint: Option<String>,
}

#[derive(Default)]
struct ConfigLayer {
    data_root: Option<PathBuf>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    data_root: Option<String>,
}

fn invalid(kind: &'static str) -> String {
    format!("BOOTSTRAP_SETTINGS_INVALID:{kind}")
}

fn is_help_flag(value: &OsStr) -> bool {
    matches!(value.to_str(), Some("--help" | "-h"))
}

fn is_version_flag(value: &OsStr) -> bool {
    matches!(value.to_str(), Some("--version" | "-V"))
}

fn is_help_command(value: &OsStr) -> bool {
    value == OsStr::new("help")
}

fn is_version_command(value: &OsStr) -> bool {
    value == OsStr::new("version")
}

fn is_command(value: &OsStr) -> Option<RootCommand> {
    match value.to_str()? {
        "--inspect-data-root" => Some(RootCommand::Inspect),
        "--initialize-data-root" => Some(RootCommand::Initialize),
        "--check-existing-root-owner" => Some(RootCommand::CheckExistingOwner),
        "--discard-incomplete-data-root" => Some(RootCommand::DiscardIncomplete),
        "--retain-file" => Some(RootCommand::RetainFile),
        "--prepare-source" => Some(RootCommand::PrepareSource),
        "--publish-prepared" => Some(RootCommand::PublishPrepared),
        "--index" => Some(RootCommand::Index),
        "--search-stdio" => Some(RootCommand::Search),
        _ => None,
    }
}

fn bounded_path(value: &OsStr, kind: &'static str) -> Result<PathBuf, String> {
    if value.is_empty() {
        return Err(invalid(kind));
    }
    if value.len() > MAX_PATH_BYTES {
        return Err("BOOTSTRAP_SETTINGS_BOUNDS_EXCEEDED".to_owned());
    }
    if value.as_encoded_bytes().contains(&0) {
        return Err(invalid(kind));
    }
    Ok(PathBuf::from(value))
}

fn option_value<'a>(
    arguments: &'a [OsString],
    index: usize,
    kind: &'static str,
) -> Result<&'a OsString, String> {
    let value = arguments.get(index + 1).ok_or_else(|| invalid(kind))?;
    if is_help_flag(value) || is_version_flag(value) || is_command(value).is_some() {
        return Err(invalid("conflicting_command"));
    }
    Ok(value)
}

fn unit_index(value: &OsStr) -> Result<usize, String> {
    let text = value.to_str().ok_or_else(|| invalid("unit"))?;
    let parsed = text.parse::<usize>().map_err(|_| invalid("unit"))?;
    if parsed.to_string() != text {
        return Err(invalid("unit"));
    }
    Ok(parsed)
}

#[derive(Default)]
struct PublicationOptions {
    endpoint: Option<String>,
}
impl PublicationOptions {
    fn take_option(&mut self, arguments: &[OsString], index: usize) -> Result<bool, String> {
        let argument = &arguments[index];
        if argument == OsStr::new("--qdrant-url") {
            let value = arguments
                .get(index + 1)
                .and_then(|v| v.to_str())
                .filter(|v| !v.is_empty() && v.len() <= 4096 && !v.as_bytes().contains(&0))
                .ok_or_else(|| invalid("publication_argument"))?;
            if self.endpoint.replace(value.to_owned()).is_some() {
                return Err(invalid("publication_duplicate"));
            }
        } else {
            return Ok(false);
        }
        Ok(true)
    }
    fn finish(
        self,
        command: RootCommand,
    ) -> Result<(Option<PublicationRequest>, Option<String>), String> {
        if command == RootCommand::Search {
            if self.endpoint.is_none() {
                return Err(invalid("search_startup"));
            }
            return Ok((None, self.endpoint));
        }
        if matches!(command, RootCommand::PublishPrepared | RootCommand::Index)
            != self.endpoint.is_some()
        {
            return Err(invalid("publication_selection"));
        }
        Ok((
            self.endpoint
                .map(|endpoint| PublicationRequest { endpoint }),
            None,
        ))
    }
}

fn validate_source_selection(
    command: RootCommand,
    file: bool,
    id: bool,
    unit: bool,
) -> Result<(), String> {
    if (command == RootCommand::RetainFile) != file {
        return Err(invalid("source_file_required"));
    }
    if (command == RootCommand::PrepareSource) != id
        || (unit && command != RootCommand::PrepareSource)
    {
        return Err(invalid("source_id_or_unit"));
    }
    Ok(())
}

fn local_invocation(arguments: &[OsString]) -> Option<Parsed> {
    if arguments.is_empty() {
        return Some(Parsed::Help);
    }
    if arguments.len() == 1 && (is_help_flag(&arguments[0]) || is_help_command(&arguments[0])) {
        return Some(Parsed::Help);
    }
    if arguments.len() == 1 && (is_version_flag(&arguments[0]) || is_version_command(&arguments[0]))
    {
        return Some(Parsed::Version);
    }
    None
}

pub(super) fn parse(arguments: &[OsString]) -> Result<Parsed, String> {
    if let Some(local) = local_invocation(arguments) {
        return Ok(local);
    }

    let mut command = None;
    let mut data_root = None;
    let mut config_file = None;
    let mut source_file = None;
    let mut index_path = None;
    let mut source_id = None;
    let mut unit = None;
    let mut publication = PublicationOptions::default();
    let mut positional_root = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        if is_help_flag(argument) || is_version_flag(argument) {
            return Err(invalid("conflicting_command"));
        }
        if let Some(parsed) = is_command(argument) {
            if command.replace(parsed).is_some() {
                return Err("USAGE_ERROR".to_owned());
            }
            index += 1;
        } else if argument == OsStr::new("--data-root") {
            if data_root.is_some() {
                return Err(invalid("data_root_duplicate"));
            }
            let value = option_value(arguments, index, "data_root_value")?;
            data_root = Some(bounded_path(value, "data_root")?);
            index += 2;
        } else if argument == OsStr::new("--config-file") {
            if config_file.is_some() {
                return Err(invalid("config_file_duplicate"));
            }
            let value = option_value(arguments, index, "config_file_value")?;
            config_file = Some(bounded_path(value, "config_file")?);
            index += 2;
        } else if argument == OsStr::new("--source-file") {
            if source_file.is_some() {
                return Err(invalid("source_file_duplicate"));
            }
            let value = option_value(arguments, index, "source_file_value")?;
            source_file = Some(bounded_path(value, "source_file")?);
            index += 2;
        } else if argument == OsStr::new("--index-path") {
            if index_path.is_some() || command != Some(RootCommand::Index) {
                return Err(invalid("index_path"));
            }
            let value = arguments
                .get(index + 1)
                .ok_or_else(|| invalid("index_path_value"))?;
            index_path = Some(bounded_path(value, "index_path")?);
            index += 2;
        } else if argument == OsStr::new("--source-id") {
            if source_id.is_some() {
                return Err(invalid("source_id_duplicate"));
            }
            let value = option_value(arguments, index, "source_id")?
                .to_str()
                .ok_or_else(|| invalid("source_id"))?;
            source_id = Some(SourceId::parse(value).map_err(|_| invalid("source_id"))?);
            index += 2;
        } else if argument == OsStr::new("--unit") {
            if unit.is_some() {
                return Err(invalid("unit_duplicate"));
            }
            unit = Some(unit_index(option_value(arguments, index, "unit")?)?);
            index += 2;
        } else if publication.take_option(arguments, index)? {
            index += 2;
        } else if argument
            .to_str()
            .is_some_and(|value| value.starts_with("--"))
        {
            return Err("UNKNOWN_ARGUMENT".to_owned());
        } else {
            if positional_root.is_some() {
                return Err("USAGE_ERROR".to_owned());
            }
            positional_root = Some(bounded_path(argument, "data_root")?);
            index += 1;
        }
    }

    let command = command.ok_or_else(|| "UNKNOWN_ARGUMENT".to_owned())?;
    let (publication, search_endpoint) = publication.finish(command)?;
    validate_source_selection(
        command,
        source_file.is_some(),
        source_id.is_some(),
        unit.is_some(),
    )?;
    if (command == RootCommand::Index) != index_path.is_some() {
        return Err(invalid("index_path_required"));
    }
    if data_root.is_some() && positional_root.is_some() {
        return Err(invalid("data_root_duplicate"));
    }
    Ok(Parsed::Root(Box::new(RootCli {
        command,
        data_root: data_root.or(positional_root),
        config_file,
        source_file,
        index_path,
        source_id,
        unit,
        publication,
        search_endpoint,
    })))
}

fn resolve_layer_path(path: &Path, base: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn read_config(path: &Path, startup_cwd: &Path) -> Result<ConfigLayer, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| invalid("config_file"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid("config_file"));
    }
    if metadata.len() > MAX_CONFIG_BYTES as u64 {
        return Err("BOOTSTRAP_SETTINGS_BOUNDS_EXCEEDED".to_owned());
    }

    let file = fs::File::open(path).map_err(|_| invalid("config_file"))?;
    let mut bytes = Vec::new();
    file.take((MAX_CONFIG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("config_file"))?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err("BOOTSTRAP_SETTINGS_BOUNDS_EXCEEDED".to_owned());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid("config_utf8"))?;
    let parsed: ConfigFile = toml::from_str(text).map_err(|_| invalid("config_toml"))?;
    let Some(data_root) = parsed.data_root else {
        return Ok(ConfigLayer::default());
    };
    let data_root = bounded_path(OsStr::new(&data_root), "data_root")?;
    let base = path.parent().unwrap_or(startup_cwd);
    Ok(ConfigLayer {
        data_root: Some(resolve_layer_path(&data_root, base)),
    })
}

fn read_environment(startup_cwd: &Path) -> Result<ConfigLayer, String> {
    let mut data_root = None;
    for (name, value) in std::env::vars_os() {
        let name = name.as_encoded_bytes();
        if !name.starts_with(ENV_PREFIX) {
            continue;
        }
        if name != DATA_ROOT_ENV {
            return Err(invalid("environment_name"));
        }
        if value.len() > MAX_ENV_BYTES {
            return Err("BOOTSTRAP_SETTINGS_BOUNDS_EXCEEDED".to_owned());
        }
        if data_root.is_some() {
            return Err(invalid("environment_duplicate"));
        }
        let value = bounded_path(&value, "data_root")?;
        data_root = Some(resolve_layer_path(&value, startup_cwd));
    }
    Ok(ConfigLayer { data_root })
}

pub(super) fn resolve(cli: RootCli, startup_cwd: &Path) -> Result<RootInvocation, String> {
    let file_layer = match cli.config_file {
        Some(config_file) => {
            let config_file = resolve_layer_path(&config_file, startup_cwd);
            read_config(&config_file, startup_cwd)?
        }
        None => ConfigLayer::default(),
    };
    let environment_layer = read_environment(startup_cwd)?;
    let data_root = cli
        .data_root
        .map(|path| resolve_layer_path(&path, startup_cwd))
        .or(environment_layer.data_root)
        .or(file_layer.data_root)
        .ok_or_else(|| invalid("missing_data_root"))?;
    Ok(RootInvocation {
        command: cli.command,
        data_root,
        source_file: cli
            .source_file
            .map(|path| resolve_layer_path(&path, startup_cwd)),
        index_path: cli
            .index_path
            .map(|path| resolve_layer_path(&path, startup_cwd)),
        source_id: cli.source_id,
        unit: cli.unit,
        publication: cli.publication,
        search_endpoint: cli.search_endpoint,
    })
}
