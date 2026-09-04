//! Human-readable bridge for the owner-fenced DIRECT stdio service.
//!
//! The daemon protocol remains tab/hex framed. This client accepts bounded
//! UTF-8 command lines, encodes query and native path arguments, and streams
//! newline-delimited JSON responses without opening the data root itself.

use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};

const MAX_INPUT_LINE_BYTES: usize = 128 * 1024;
const MAX_RESPONSE_LINE_BYTES: usize = 64 * 1024;

/// Intercepts `serve-data-root ROOT [--daemon PATH]` before one-shot forwarding.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().and_then(|value| value.to_str()) != Some("serve-data-root") {
        return None;
    }
    let result = parse_arguments(&arguments[1..]).and_then(|(root, daemon)| {
        run_session(&daemon_path(daemon), &root)
    });
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

    let user_input = io::stdin();
    let mut user_input = user_input.lock();
    loop {
        let Some(line) = read_user_line(&mut user_input)? else {
            send_protocol_command(&mut child, "shutdown")?;
            drain_until_terminal(&mut daemon_output, Terminal::Shutdown)?;
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let translated = match translate_command(&line) {
            Ok(command) => command,
            Err(error) => {
                eprintln!("{{\"event\":\"client_error\",\"error\":{}}}", json_string(&error));
                continue;
            }
        };
        send_protocol_command(&mut child, &translated.protocol)?;
        drain_until_terminal(&mut daemon_output, translated.terminal)?;
        if translated.terminal == Terminal::Shutdown {
            break;
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
    Search,
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
            Self::Search => line.contains("\"event\":\"corpus_search_complete\""),
            Self::SourceList => line.contains("\"event\":\"source_list_complete\""),
            Self::Shutdown => line.contains("\"event\":\"data_root_stopped\""),
        }
    }
}

fn drain_until_terminal(
    reader: &mut impl BufRead,
    terminal: Terminal,
) -> Result<(), String> {
    loop {
        let line = read_response_line(reader)?
            .ok_or_else(|| "DAEMON_CLOSED_MID_RESPONSE".to_owned())?;
        println!("{line}");
        if terminal.reached(&line) {
            return Ok(());
        }
    }
}

struct TranslatedCommand {
    protocol: String,
    terminal: Terminal,
}

fn translate_command(line: &str) -> Result<TranslatedCommand, String> {
    let trimmed = line.trim();
    let (verb, remainder) = split_verb(trimmed);
    match verb {
        "health" if remainder.is_empty() => single("health"),
        "version" if remainder.is_empty() => single("version"),
        "verify" if remainder.is_empty() => single("verify"),
        "list-sources" if remainder.is_empty() => Ok(TranslatedCommand {
            protocol: "list-sources".to_owned(),
            terminal: Terminal::SourceList,
        }),
        "shutdown" if remainder.is_empty() => Ok(TranslatedCommand {
            protocol: "shutdown".to_owned(),
            terminal: Terminal::Shutdown,
        }),
        "gc-dry-run" if remainder.is_empty() => single("gc\tdry-run"),
        "gc-apply" if remainder.is_empty() => single("gc\tapply"),
        "index-file" => {
            let path = required_remainder(remainder, "INDEX_FILE_PATH_REQUIRED")?;
            Ok(TranslatedCommand {
                protocol: format!("index-file\t{}", encode_path(Path::new(path))?),
                terminal: Terminal::Single,
            })
        }
        "index-directory" => {
            let path = required_remainder(remainder, "INDEX_DIRECTORY_PATH_REQUIRED")?;
            Ok(TranslatedCommand {
                protocol: format!(
                    "index-directory\t{}",
                    encode_path(Path::new(path))?
                ),
                terminal: Terminal::DirectoryIndex,
            })
        }
        "search" | "search-i" => {
            let query = required_remainder(remainder, "SEARCH_QUERY_REQUIRED")?;
            let mode = if verb == "search-i" {
                "ascii-insensitive"
            } else {
                "sensitive"
            };
            Ok(TranslatedCommand {
                protocol: format!("search\t{mode}\t{}", hex(query.as_bytes())),
                terminal: Terminal::Search,
            })
        }
        "retire" => {
            let source_id = required_remainder(remainder, "SOURCE_ID_REQUIRED")?;
            if source_id.split_whitespace().count() != 1 {
                return Err("SOURCE_ID_INVALID".to_owned());
            }
            single(&format!("retire\t{source_id}"))
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
        "help" if remainder.is_empty() => {
            eprintln!(
                concat!(
                    "session commands: health, version, verify, list-sources, ",
                    "index-file PATH, index-directory PATH, search QUERY, ",
                    "search-i QUERY, retire SOURCE_ID, read-revision REVISION_ID ",
                    "START END, gc-dry-run, gc-apply, shutdown"
                )
            );
            Err("CLIENT_HELP_SHOWN".to_owned())
        }
        _ => Err("CLIENT_SESSION_COMMAND_INVALID".to_owned()),
    }
}

fn single(protocol: &str) -> Result<TranslatedCommand, String> {
    Ok(TranslatedCommand {
        protocol: protocol.to_owned(),
        terminal: Terminal::Single,
    })
}

fn split_verb(value: &str) -> (&str, &str) {
    match value.find(char::is_whitespace) {
        Some(index) => (&value[..index], value[index..].trim_start()),
        None => (value, ""),
    }
}

fn required_remainder<'a>(
    value: &'a str,
    error: &'static str,
) -> Result<&'a str, String> {
    if value.is_empty() {
        Err(error.to_owned())
    } else {
        Ok(value)
    }
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
    if bytes.len() > 32 * 1024 {
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
