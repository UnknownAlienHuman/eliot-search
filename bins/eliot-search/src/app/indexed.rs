//! Dedicated parser and transport handoff for the indexed query verb.

use std::env;
use std::ffi::{OsStr, OsString};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::endpoint_client;
use crate::provider_client::{self, UnsignedRequest};

struct Options {
    address: Option<SocketAddr>,
    data_root: PathBuf,
    token_file: Option<PathBuf>,
    positional: Vec<OsString>,
}

/// Intercepts `eliot-search indexed ...`; all other commands remain owned by
/// the canonical application core.
pub(super) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().and_then(|value| value.to_str()) != Some("indexed") {
        return None;
    }
    if arguments.len() == 2
        && matches!(arguments[1].to_str(), Some("--help" | "-h"))
    {
        print_usage();
        return Some(ExitCode::SUCCESS);
    }
    let result = run(&arguments[1..]);
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", sanitize_json(&error));
            ExitCode::from(provider_client::exit_for_error(&error))
        }
    })
}

fn run(arguments: &[OsString]) -> Result<(), String> {
    let options = parse_options(arguments)?;
    if options.positional.len() != 1 {
        return Err("USAGE_ERROR".to_owned());
    }
    let query = options.positional[0]
        .to_str()
        .ok_or_else(|| "REMOTE_QUERY_NOT_UTF8".to_owned())?;
    let request = UnsignedRequest::indexed_query(query.as_bytes())?;
    let address = match options.address {
        Some(address) => address,
        None => provider_client::read_endpoint_descriptor(&options.data_root)?.address,
    };
    let token_file = options
        .token_file
        .unwrap_or_else(|| options.data_root.join("runtime").join("auth.token"));
    endpoint_client::invoke_remote(&address.to_string(), &token_file, &request)
}

fn parse_options(arguments: &[OsString]) -> Result<Options, String> {
    let mut address = None;
    let mut data_root = None;
    let mut token_file = None;
    let mut positional = Vec::new();
    let mut values = arguments.iter();
    while let Some(argument) = values.next() {
        if argument == OsStr::new("--address") {
            let value = values.next().ok_or_else(|| "USAGE_ERROR".to_owned())?;
            let parsed: SocketAddr = value
                .to_str()
                .ok_or_else(|| "REMOTE_ADDRESS_NOT_UTF8".to_owned())?
                .parse()
                .map_err(|_| "REMOTE_ADDRESS_INVALID".to_owned())?;
            if !parsed.ip().is_loopback() {
                return Err("REMOTE_NON_LOOPBACK_DENIED".to_owned());
            }
            if address.replace(parsed).is_some() {
                return Err("USAGE_ERROR".to_owned());
            }
        } else if argument == OsStr::new("--data-root") {
            let value = values.next().ok_or_else(|| "USAGE_ERROR".to_owned())?;
            if data_root.replace(PathBuf::from(value)).is_some() {
                return Err("USAGE_ERROR".to_owned());
            }
        } else if argument == OsStr::new("--token-file") {
            let value = values.next().ok_or_else(|| "USAGE_ERROR".to_owned())?;
            if token_file.replace(PathBuf::from(value)).is_some() {
                return Err("USAGE_ERROR".to_owned());
            }
        } else if argument.to_str().is_some_and(|text| text.starts_with("--")) {
            return Err("USAGE_ERROR".to_owned());
        } else {
            positional.push(argument.clone());
        }
    }
    Ok(Options {
        address,
        data_root: data_root.unwrap_or_else(default_data_root),
        token_file,
        positional,
    })
}

fn default_data_root() -> PathBuf {
    if let Some(value) = env::var_os("ELIOT_SEARCH_DATA_ROOT") {
        return PathBuf::from(value);
    }
    #[cfg(windows)]
    if let Some(value) = env::var_os("LOCALAPPDATA") {
        return PathBuf::from(value).join("Eliot").join("Search");
    }
    env::current_dir().map_or_else(
        |_| PathBuf::from(".eliot-search"),
        |directory| directory.join(".eliot-search"),
    )
}

fn print_usage() {
    println!(concat!(
        "eliot-search indexed \"TERMS\" ",
        "[--data-root DIR] [--address IP:PORT] [--token-file PATH]\n",
        "Requires accepted search and indexed-Qdrant receipts; an unqualified ",
        "runtime exits 2 with PROVIDER_INDEXED_QUERY_UNAVAILABLE."
    ));
}

fn sanitize_json(value: &str) -> String {
    let mut output = String::with_capacity(value.len().min(512));
    for character in value.chars().take(512) {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => output.push('_'),
            character => output.push(character),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(value: &str) -> OsString {
        OsString::from(value)
    }

    #[test]
    fn indexed_options_are_strict_and_loopback_only() {
        let parsed = parse_options(&[
            arg("needle"),
            arg("--data-root"),
            arg("root"),
            arg("--address"),
            arg("127.0.0.1:39171"),
            arg("--token-file"),
            arg("token"),
        ])
        .expect("valid options");
        assert_eq!(parsed.positional, vec![arg("needle")]);
        assert_eq!(parsed.address.expect("address").port(), 39171);
        assert_eq!(parsed.data_root, PathBuf::from("root"));
        assert_eq!(parsed.token_file, Some(PathBuf::from("token")));

        assert!(parse_options(&[arg("needle"), arg("--unknown")]).is_err());
        assert!(
            parse_options(&[
                arg("needle"),
                arg("--address"),
                arg("93.184.216.34:80"),
            ])
            .is_err()
        );
        assert_eq!(
            run(&[arg("needle"), arg("second")]),
            Err("USAGE_ERROR".to_owned())
        );
        assert!(
            parse_options(&[
                arg("needle"),
                arg("--data-root"),
                arg("one"),
                arg("--data-root"),
                arg("two"),
            ])
            .is_err()
        );
    }
}
