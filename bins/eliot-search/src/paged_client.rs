//! Human-readable client for the paged owner-fenced DIRECT service.
//!
//! The daemon protocol is tab/hex framed. This client accepts bounded UTF-8
//! commands, tracks only the latest opaque session continuation token, and
//! never opens the data root or retained revisions itself.

use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};

const MAX_INPUT_LINE_BYTES: usize = 128 * 1024;
const MAX_RESPONSE_LINE_BYTES: usize = 64 * 1024;
const MAX_QUERY_BYTES: usize = 64 * 1024;
const MAX_PATH_BYTES: usize = 32 * 1024;
const DEFAULT_PAGE_SIZE: usize = 100;
const MAX_PAGE_SIZE: usize = 1_000;

/// Intercepts `serve-data-root ROOT [--daemon PATH]` before one-shot forwarding.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().and_then(|value| value.to_str()) != Some("serve-data-root") {
        return None;
    }
    let result = parse_arguments(&arguments[1..])
        .and_then(|(root, daemon)| run_session(&daemon_path(daemon), &root));
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            ExitCode::from(2)
        }
    })
}

fn parse_arguments(arguments: &[OsString]) -> Result<(PathBuf, Option<PathBuf>), String> {
    match arguments {
        [root] => Ok((PathBuf::from(root), None)),
        [root, flag, daemon] if flag == OsStr::new("--daemon") => {
            Ok((PathBuf::from(root), Some(PathBuf::from(daemon))))
        }
        _ => Err("USAGE_ERROR".to_owned()),
    }
}

fn daemon_path(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Some(path) = env::var_os("ELIOT_SEARCHD_BIN") {
        return PathBuf::from(path);
    }
    let executable = env::current_exe().unwrap_or_else(|_| PathBuf::from("eliot-search"));
    let sibling = executable.with_file_name(if cfg!(windows) {
        "eliot-searchd.exe"
    } else {
        "eliot-searchd"
    });
    if sibling.exists() {
        sibling
    } else {
        PathBuf::from(if cfg!(windows) {
            "eliot-searchd.exe"
        } else {
            "eliot-searchd"
        })
    }
}

fn run_session(daemon: &Path, root: &Path) -> Result<(), String> {
    let mut child = Command::new(daemon)
        .arg("--serve-data-root")
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("DAEMON_START_ERROR:{error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "DAEMON_STDOUT_UNAVAILABLE".to_owned())?;
    let mut daemon_output = BufReader::new(stdout);
    let ready = read_response_line(&mut daemon_output)?
        .ok_or_else(|| "DAEMON_CLOSED_BEFORE_READY".to_owned())?;
    if !ready.contains("\"event\":\"data_root_ready\"") {
        terminate_child(&mut child);
        return Err(format!("DAEMON_NOT_READY:{ready}"));
    }
    println!("{ready}");
    print_session_help();

    let user_input = io::stdin();
    let mut user_input = user_input.lock();
    let mut latest_continuation: Option<String> = None;
    loop {
        let Some(line) = read_user_line(&mut user_input)? else {
            send_protocol_command(&mut child, "shutdown")?;
            drain_until_terminal(&mut daemon_output, Terminal::Shutdown)?;
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let action = match translate_command(&line, latest_continuation.as_deref()) {
            Ok(action) => action,
            Err(error) => {
                eprintln!(
                    "{{\"event\":\"client_error\",\"error\":{}}}",
                    json_string(&error)
                );
                continue;
            }
        };
        match action {
            ClientAction::Help => print_session_help(),
            ClientAction::Protocol(translated) => {
                send_protocol_command(&mut child, &translated.protocol)?;
                let terminal_line =
                    drain_until_terminal(&mut daemon_output, translated.terminal)?;
                if translated.clears_continuation {
                    latest_continuation = None;
                }
                if translated.terminal == Terminal::SearchPage {
                    latest_continuation = extract_continuation_token(&terminal_line)?;
                    if let Some(token) = &latest_continuation {
                        eprintln!(
                            "{{\"event\":\"client_continuation_ready\",\"token\":\"{}\"}}",
                            token
                        );
                    }
                }
                if translated.terminal == Terminal::Shutdown {
                    break;
                }
            }
        }
    }

    drop(child.stdin.take());
    let status = child
        .wait()
        .map_err(|error| format!("DAEMON_WAIT_ERROR:{error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("DAEMON_EXITED:{status}"))
    }
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn send_protocol_command(child: &mut Child, command: &str) -> Result<(), String> {
    let input = child
        .stdin
        .as_mut()
        .ok_or_else(|| "DAEMON_STDIN_UNAVAILABLE".to_owned())?;
    input
        .write_all(command.as_bytes())
        .and_then(|()| input.write_all(b"\n"))
        .and_then(|()| input.flush())
        .map_err(|error| format!("DAEMON_WRITE_ERROR:{error}"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Terminal {
    Single,
    DirectoryIndex,
    StreamingSearch,
    SearchPage,
    SourceList,
    Shutdown,
}

impl Terminal {
    fn reached(self, line: &str) -> bool {
        if line.contains("\"event\":\"error\"") {
            return true;
        }
        match self {
            Self::Single => true,
            Self::DirectoryIndex => {
                line.contains("\"event\":\"directory_index_complete\"")
            }
            Self::StreamingSearch => {
                line.contains("\"event\":\"corpus_search_complete\"")
            }
            Self::SearchPage => line.contains("\"event\":\"search_page_complete\""),
            Self::SourceList => line.contains("\"event\":\"source_list_complete\""),
            Self::Shutdown => line.contains("\"event\":\"data_root_stopped\""),
        }
    }
}

fn drain_until_terminal(
    reader: &mut impl BufRead,
    terminal: Terminal,
) -> Result<String, String> {
    loop {
        let line = read_response_line(reader)?
            .ok_or_else(|| "DAEMON_CLOSED_MID_RESPONSE".to_owned())?;
        println!("{line}");
        if terminal.reached(&line) {
            return Ok(line);
        }
    }
}

struct TranslatedCommand {
    protocol: String,
    terminal: Terminal,
    clears_continuation: bool,
}

enum ClientAction {
    Help,
    Protocol(TranslatedCommand),
}

fn translate_command(
    line: &str,
    latest_continuation: Option<&str>,
) -> Result<ClientAction, String> {
    let trimmed = line.trim();
    let (verb, remainder) = split_verb(trimmed);
    let translated = match verb {
        "help" if remainder.is_empty() => return Ok(ClientAction::Help),
        "health" if remainder.is_empty() => single("health"),
        "version" if remainder.is_empty() => single("version"),
        "verify" if remainder.is_empty() => single("verify"),
        "verify-directory-manifests" if remainder.is_empty() => {
            single("verify-directory-manifests")
        }
        "list-sources" if remainder.is_empty() => command(
            "list-sources".to_owned(),
            Terminal::SourceList,
            false,
        ),
        "shutdown" if remainder.is_empty() => {
            command("shutdown".to_owned(), Terminal::Shutdown, true)
        }
        "gc-dry-run" if remainder.is_empty() => single("gc\tdry-run"),
        "gc-apply" if remainder.is_empty() => single("gc\tapply"),
        "index-file" => command(
            format!(
                "index-file\t{}",
                encode_path(Path::new(required_remainder(
                    remainder,
                    "INDEX_FILE_PATH_REQUIRED",
                )?))?,
            ),
            Terminal::Single,
            true,
        ),
        "index-directory" => command(
            format!(
                "index-directory\t{}",
                encode_path(Path::new(required_remainder(
                    remainder,
                    "INDEX_DIRECTORY_PATH_REQUIRED",
                )?))?,
            ),
            Terminal::DirectoryIndex,
            true,
        ),
        "sync-directory" => command(
            format!(
                "sync-directory\t{}",
                encode_path(Path::new(required_remainder(
                    remainder,
                    "SYNC_DIRECTORY_PATH_REQUIRED",
                )?))?,
            ),
            Terminal::Single,
            true,
        ),
        "search" | "search-i" => {
            let query = required_remainder(remainder, "SEARCH_QUERY_REQUIRED")?;
            paged_search(verb == "search-i", DEFAULT_PAGE_SIZE, query)?
        }
        "search-page" | "search-page-i" => {
            let (page_size, query) = split_page_and_query(remainder)?;
            paged_search(verb == "search-page-i", page_size, query)?
        }
        "search-all" | "search-all-i" => {
            let query = required_remainder(remainder, "SEARCH_QUERY_REQUIRED")?;
            validate_query(query)?;
            command(
                format!(
                    "search\t{}\t{}",
                    if verb == "search-all-i" {
                        "ascii-insensitive"
                    } else {
                        "sensitive"
                    },
                    hex(query.as_bytes()),
                ),
                Terminal::StreamingSearch,
                false,
            )
        }
        "next" => {
            let token = latest_continuation
                .ok_or_else(|| "NO_ACTIVE_CONTINUATION".to_owned())?;
            let page_size = if remainder.is_empty() {
                DEFAULT_PAGE_SIZE
            } else {
                parse_page_size(remainder)?
            };
            command(
                format!("continue\t{token}\t{page_size}"),
                Terminal::SearchPage,
                false,
            )
        }
        "continue" => {
            let fields = remainder.split_whitespace().collect::<Vec<_>>();
            let (token, page_size) = match fields.as_slice() {
                [token] => (*token, DEFAULT_PAGE_SIZE),
                [token, page_size] => (*token, parse_page_size(page_size)?),
                _ => return Err("CONTINUE_USAGE".to_owned()),
            };
            command(
                format!("continue\t{token}\t{page_size}"),
                Terminal::SearchPage,
                false,
            )
        }
        "retire" => {
            let source_id = required_remainder(remainder, "SOURCE_ID_REQUIRED")?;
            if source_id.split_whitespace().count() != 1 {
                return Err("SOURCE_ID_INVALID".to_owned());
            }
            command(
                format!("retire\t{source_id}"),
                Terminal::Single,
                true,
            )
        }
        "read-revision" => {
            let fields = remainder.split_whitespace().collect::<Vec<_>>();
            if fields.len() != 3 {
                return Err("READ_REVISION_USAGE".to_owned());
            }
            single(&format!(
                "read-revision\t{}\t{}\t{}",
                fields[0], fields[1], fields[2]
            ))
        }
        _ => return Err("CLIENT_SESSION_COMMAND_INVALID".to_owned()),
    };
    Ok(ClientAction::Protocol(translated))
}

fn paged_search(
    ascii_insensitive: bool,
    page_size: usize,
    query: &str,
) -> Result<TranslatedCommand, String> {
    validate_query(query)?;
    Ok(command(
        format!(
            "search-page\t{}\t{}\t{}",
            if ascii_insensitive {
                "ascii-insensitive"
            } else {
                "sensitive"
            },
            page_size,
            hex(query.as_bytes()),
        ),
        Terminal::SearchPage,
        false,
    ))
}

fn split_page_and_query(value: &str) -> Result<(usize, &str), String> {
    let (page_size, query) = split_verb(value);
    if page_size.is_empty() || query.is_empty() {
        return Err("SEARCH_PAGE_USAGE".to_owned());
    }
    Ok((parse_page_size(page_size)?, query))
}

fn parse_page_size(value: &str) -> Result<usize, String> {
    let page_size = value
        .parse::<usize>()
        .map_err(|_| "PAGE_SIZE_INVALID".to_owned())?;
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        Err("PAGE_SIZE_INVALID".to_owned())
    } else {
        Ok(page_size)
    }
}

fn validate_query(query: &str) -> Result<(), String> {
    if query.is_empty() {
        Err("SEARCH_QUERY_REQUIRED".to_owned())
    } else if query.len() > MAX_QUERY_BYTES {
        Err("SEARCH_QUERY_TOO_LARGE".to_owned())
    } else {
        Ok(())
    }
}

fn command(protocol: String, terminal: Terminal, clears_continuation: bool) -> TranslatedCommand {
    TranslatedCommand {
        protocol,
        terminal,
        clears_continuation,
    }
}

fn single(protocol: &str) -> TranslatedCommand {
    command(protocol.to_owned(), Terminal::Single, false)
}

fn extract_continuation_token(line: &str) -> Result<Option<String>, String> {
    if line.contains("\"event\":\"error\"") {
        return Ok(None);
    }
    let marker = "\"continuation_token\":";
    let start = line
        .find(marker)
        .ok_or_else(|| "CONTINUATION_RESPONSE_INVALID".to_owned())?
        + marker.len();
    let remainder = &line[start..];
    if remainder.starts_with("null") {
        return Ok(None);
    }
    let remainder = remainder
        .strip_prefix('"')
        .ok_or_else(|| "CONTINUATION_RESPONSE_INVALID".to_owned())?;
    let end = remainder
        .find('"')
        .ok_or_else(|| "CONTINUATION_RESPONSE_INVALID".to_owned())?;
    let token = &remainder[..end];
    if token.len() != 64 || !token.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err("CONTINUATION_TOKEN_INVALID".to_owned());
    }
    Ok(Some(token.to_owned()))
}

fn split_verb(value: &str) -> (&str, &str) {
    match value.find(char::is_whitespace) {
        Some(index) => (&value[..index], value[index..].trim_start()),
        None => (value, ""),
    }
}

fn required_remainder<'a>(value: &'a str, error: &'static str) -> Result<&'a str, String> {
    if value.is_empty() {
        Err(error.to_owned())
    } else {
        Ok(value)
    }
}

fn print_session_help() {
    eprintln!(
        concat!(
            "session commands: health, version, verify, ",
            "verify-directory-manifests, list-sources, index-file PATH, ",
            "index-directory PATH, sync-directory PATH, search QUERY, ",
            "search-i QUERY, search-page SIZE QUERY, search-page-i SIZE QUERY, ",
            "next [SIZE], continue TOKEN [SIZE], search-all QUERY, ",
            "search-all-i QUERY, retire SOURCE_ID, read-revision REVISION_ID ",
            "START END, gc-dry-run, gc-apply, shutdown"
        )
    );
}

fn read_user_line(reader: &mut impl BufRead) -> Result<Option<String>, String> {
    let Some(bytes) = read_bounded_line(reader, MAX_INPUT_LINE_BYTES)? else {
        return Ok(None);
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "CLIENT_COMMAND_NOT_UTF8".to_owned())
}

fn read_response_line(reader: &mut impl BufRead) -> Result<Option<String>, String> {
    let Some(bytes) = read_bounded_line(reader, MAX_RESPONSE_LINE_BYTES)? else {
        return Ok(None);
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "DAEMON_RESPONSE_NOT_UTF8".to_owned())
}

fn read_bounded_line(
    reader: &mut impl BufRead,
    maximum: usize,
) -> Result<Option<Vec<u8>>, String> {
    let mut output = Vec::new();
    let mut too_large = false;
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|error| format!("LINE_READ_ERROR:{error}"))?;
        if buffer.is_empty() {
            return if output.is_empty() {
                Ok(None)
            } else if too_large {
                Err("LINE_TOO_LARGE".to_owned())
            } else {
                Ok(Some(output))
            };
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(buffer.len(), |index| index + 1);
        let content_length = newline.unwrap_or(buffer.len());
        if !too_large {
            if output.len().saturating_add(content_length) > maximum {
                too_large = true;
                output.clear();
            } else {
                output.extend_from_slice(&buffer[..content_length]);
            }
        }
        reader.consume(consumed);
        if newline.is_some() {
            if output.last() == Some(&b'\r') {
                output.pop();
            }
            return if too_large {
                Err("LINE_TOO_LARGE".to_owned())
            } else {
                Ok(Some(output))
            };
        }
    }
}

fn encode_path(path: &Path) -> Result<String, String> {
    let bytes = path_bytes(path);
    if bytes.len() > MAX_PATH_BYTES {
        return Err("CLIENT_PATH_TOO_LARGE".to_owned());
    }
    Ok(hex(&bytes))
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(&mut output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}
