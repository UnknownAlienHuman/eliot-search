use std::ffi::OsString;

pub(super) const MAX_DIAGNOSTIC_REVISION_SLICE_BYTES: u64 = 24 * 1024;

pub(super) fn is_persistent_command(command: &str) -> bool {
    matches!(
        command,
        "--health-data-root"
            | "--index-file"
            | "--index-directory"
            | "--search-root"
            | "--search-root-ascii-insensitive"
            | "--list-sources"
            | "--verify-root"
            | "--retire-source"
            | "--read-revision"
            | "--repair-root"
            | "--gc-root"
    )
}

pub(super) const fn help() -> &'static str {
    concat!(
        "eliot-searchd ",
        env!("CARGO_PKG_VERSION"),
        "\n\n",
        "CONTROL:\n",
        "  eliot-searchd --help\n",
        "  eliot-searchd --version\n",
        "  eliot-searchd --health\n",
        "  eliot-searchd --health-data-root ROOT\n",
        "  eliot-searchd --self-test\n",
        "  eliot-searchd --stdio\n",
        "  eliot-searchd --serve-data-root ROOT\n",
        "  eliot-searchd --serve-loopback-data-root ROOT PORT TOKEN_FILE\n\n",
        "ONE-SHOT SEARCH:\n",
        "  eliot-searchd --scan-stdin QUERY\n",
        "  eliot-searchd --scan-stdin-ascii-insensitive QUERY\n",
        "  eliot-searchd --scan-file QUERY FILE\n",
        "  eliot-searchd --scan-file-ascii-insensitive QUERY FILE\n\n",
        "PERSISTENT DIRECT CORPUS:\n",
        "  eliot-searchd --index-file ROOT FILE\n",
        "  eliot-searchd --index-directory ROOT DIRECTORY\n",
        "  eliot-searchd --search-root ROOT QUERY\n",
        "  eliot-searchd --search-root-ascii-insensitive ROOT QUERY\n",
        "  eliot-searchd --list-sources ROOT\n",
        "  eliot-searchd --verify-root ROOT\n",
        "  eliot-searchd --retire-source ROOT SOURCE_ID\n",
        "  eliot-searchd --read-revision ROOT REVISION_ID START END\n\n",
        "MAINTENANCE:\n",
        "  eliot-searchd --repair-root ROOT\n",
        "  eliot-searchd --gc-root ROOT --dry-run\n",
        "  eliot-searchd --gc-root ROOT --apply\n\n",
        "Windows protects retained revisions with DPAPI and a per-namespace ",
        "Credential Manager secret. Other platforms retain the explicit ",
        "plaintext-development profile. Every persistent response derives ",
        "encrypted_at_rest from the complete current object inventory.\n",
    )
}

pub(super) fn require_count(arguments: &[OsString], expected: usize) -> Result<(), String> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err("USAGE_ERROR".to_owned())
    }
}

pub(super) fn parse_u64(value: &OsString, label: &str) -> Result<u64, String> {
    value
        .to_str()
        .ok_or_else(|| format!("INVALID_{label}"))?
        .parse::<u64>()
        .map_err(|_| format!("INVALID_{label}"))
}
