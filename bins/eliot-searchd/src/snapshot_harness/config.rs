//! Bounded CLI and local-path configuration for the legacy snapshot harness.

use std::env;
use std::fs;
use std::io::{self, ErrorKind};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process;

use crate::snapshot::SnapshotLimits;

const DEFAULT_ADDRESS: &str = "127.0.0.1:39171";
const DEFAULT_MAX_FILES: usize = 10_000;
const DEFAULT_MAX_FILE_BYTES: u64 = 2 * 1_024 * 1_024;
const DEFAULT_MAX_TOTAL_BYTES: u64 = 512 * 1_024 * 1_024;
const DEFAULT_MAX_RESULTS: usize = 8;
const MAX_CONFIGURED_RESULTS: usize = 8;
const DEFAULT_MAX_EXCERPT_CHARS: usize = 240;
const MAX_CONFIGURED_EXCERPT_CHARS: usize = 512;

#[derive(Debug)]
pub(super) struct Options {
    pub(super) address: SocketAddr,
    pub(super) data_root: PathBuf,
    pub(super) token_file: PathBuf,
    pub(super) source_roots: Vec<PathBuf>,
    pub(super) limits: SnapshotLimits,
    pub(super) self_test: bool,
}

pub(super) fn parse_options() -> io::Result<Options> {
    let mut address = DEFAULT_ADDRESS
        .parse::<SocketAddr>()
        .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "invalid default address"))?;
    let mut data_root = default_data_root()?;
    let mut token_file: Option<PathBuf> = None;
    let mut source_roots = Vec::new();
    let mut limits = SnapshotLimits {
        files: DEFAULT_MAX_FILES,
        file_bytes: DEFAULT_MAX_FILE_BYTES,
        total_bytes: DEFAULT_MAX_TOTAL_BYTES,
        results: DEFAULT_MAX_RESULTS,
        excerpt_chars: DEFAULT_MAX_EXCERPT_CHARS,
    };
    let mut self_test = false;

    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "serve" => {}
            "--address" => {
                address = next_value(&mut arguments, "--address")?
                    .parse()
                    .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "invalid address"))?;
            }
            "--data-root" => {
                data_root = PathBuf::from(next_value(&mut arguments, "--data-root")?);
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(next_value(&mut arguments, "--token-file")?));
            }
            "--source-root" => {
                source_roots.push(PathBuf::from(next_value(&mut arguments, "--source-root")?));
            }
            "--max-files" => {
                limits.files = parse_positive_usize(
                    &next_value(&mut arguments, "--max-files")?,
                    "--max-files",
                )?;
            }
            "--max-file-bytes" => {
                limits.file_bytes = parse_positive_u64(
                    &next_value(&mut arguments, "--max-file-bytes")?,
                    "--max-file-bytes",
                )?;
            }
            "--max-total-bytes" => {
                limits.total_bytes = parse_positive_u64(
                    &next_value(&mut arguments, "--max-total-bytes")?,
                    "--max-total-bytes",
                )?;
            }
            "--max-results" => {
                limits.results = parse_positive_usize(
                    &next_value(&mut arguments, "--max-results")?,
                    "--max-results",
                )?;
            }
            "--max-excerpt-chars" => {
                limits.excerpt_chars = parse_positive_usize(
                    &next_value(&mut arguments, "--max-excerpt-chars")?,
                    "--max-excerpt-chars",
                )?;
            }
            "--self-test" => self_test = true,
            "--help" | "-h" => {
                print_help();
                process::exit(0);
            }
            "--version" | "-V" => {
                println!("eliot-searchd {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            _ => {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("unknown argument: {argument}"),
                ));
            }
        }
    }

    ensure_loopback(address.ip())?;
    limits.validate()?;
    if limits.results > MAX_CONFIGURED_RESULTS
        || limits.excerpt_chars > MAX_CONFIGURED_EXCERPT_CHARS
    {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "response-shaping limits exceed the protocol ceiling",
        ));
    }
    let token_file = token_file.unwrap_or_else(|| data_root.join("runtime").join("auth.token"));
    Ok(Options {
        address,
        data_root,
        token_file,
        source_roots,
        limits,
        self_test,
    })
}

pub(super) fn canonical_source_roots(roots: Vec<PathBuf>) -> io::Result<Vec<PathBuf>> {
    let roots = if roots.is_empty() {
        vec![env::current_dir()?]
    } else {
        roots
    };
    let mut canonical = Vec::new();
    for root in roots {
        let root = canonical_local_directory(&root, false)?;
        if !canonical.contains(&root) {
            canonical.push(root);
        }
    }
    Ok(canonical)
}

pub(super) fn canonical_local_directory(path: &Path, create: bool) -> io::Result<PathBuf> {
    if create {
        fs::create_dir_all(path)?;
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "directory must be a real local directory, not a symbolic link",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "directory reparse points are not accepted",
            ));
        }
    }
    let canonical = fs::canonicalize(path)?;
    let metadata = fs::symlink_metadata(&canonical)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "canonical directory identity is invalid",
        ));
    }
    Ok(canonical)
}

pub(super) fn ensure_loopback(ip: IpAddr) -> io::Result<()> {
    if ip.is_loopback() {
        Ok(())
    } else {
        Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "eliot-searchd may bind only to a loopback address",
        ))
    }
}

fn next_value(arguments: &mut impl Iterator<Item = String>, name: &str) -> io::Result<String> {
    arguments.next().ok_or_else(|| {
        io::Error::new(ErrorKind::InvalidInput, format!("{name} requires a value"))
    })
}

fn parse_positive_usize(value: &str, name: &str) -> io::Result<usize> {
    let value = value
        .parse::<usize>()
        .map_err(|_| io::Error::new(ErrorKind::InvalidInput, format!("invalid {name}")))?;
    if value == 0 {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("{name} must be positive"),
        ));
    }
    Ok(value)
}

fn parse_positive_u64(value: &str, name: &str) -> io::Result<u64> {
    let value = value
        .parse::<u64>()
        .map_err(|_| io::Error::new(ErrorKind::InvalidInput, format!("invalid {name}")))?;
    if value == 0 {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("{name} must be positive"),
        ));
    }
    Ok(value)
}

fn default_data_root() -> io::Result<PathBuf> {
    if let Some(value) = env::var_os("ELIOT_SEARCH_DATA_ROOT") {
        return Ok(PathBuf::from(value));
    }
    #[cfg(windows)]
    if let Some(value) = env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(value).join("Eliot").join("Search"));
    }
    #[cfg(not(windows))]
    if let Some(value) = env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(value).join("eliot-search"));
    }
    env::current_dir().map(|directory| directory.join(".eliot-search"))
}

fn print_help() {
    println!(
        concat!(
            "eliot-searchd {}\n\n",
            "USAGE:\n",
            "    eliot-searchd [serve] [OPTIONS]\n\n",
            "OPTIONS:\n",
            "    --address <IP:PORT>       Loopback endpoint (default {})\n",
            "    --data-root <PATH>        Owned local state root\n",
            "    --token-file <PATH>       Local authentication token file\n",
            "    --source-root <PATH>      Search root; repeatable (default cwd)\n",
            "    --max-files <N>           Snapshot file ceiling (default {})\n",
            "    --max-file-bytes <N>      Per-file byte ceiling (default {})\n",
            "    --max-total-bytes <N>     Snapshot byte ceiling (default {})\n",
            "    --max-results <N>         Result ceiling, at most {} (default {})\n",
            "    --max-excerpt-chars <N>   Excerpt ceiling, at most {} (default {})\n",
            "    --self-test               Run bounded startup self-test and exit\n",
            "    -V, --version             Print version\n",
            "    -h, --help                Print help\n\n",
            "The current local snapshot retains plaintext UTF-8 revisions. It is\n",
            "source-backed but not yet the encrypted production storage profile."
        ),
        env!("CARGO_PKG_VERSION"),
        DEFAULT_ADDRESS,
        DEFAULT_MAX_FILES,
        DEFAULT_MAX_FILE_BYTES,
        DEFAULT_MAX_TOTAL_BYTES,
        MAX_CONFIGURED_RESULTS,
        DEFAULT_MAX_RESULTS,
        MAX_CONFIGURED_EXCERPT_CHARS,
        DEFAULT_MAX_EXCERPT_CHARS,
    );
}
