//! Closed daemon command and stdio protocol constants.

pub(super) const PROTOCOL_VERSION: u16 = 1;
pub(super) const MAX_COMMAND_BYTES: usize = 1_024;
pub(super) const MAX_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Command {
    Health,
    Version,
    Shutdown,
}

impl Command {
    pub(super) fn parse(value: &str) -> Result<Self, &'static str> {
        match value.trim() {
            "health" | "HEALTH" => Ok(Self::Health),
            "version" | "VERSION" => Ok(Self::Version),
            "shutdown" | "SHUTDOWN" => Ok(Self::Shutdown),
            _ => Err("UNSUPPORTED_COMMAND"),
        }
    }
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
        "  eliot-searchd --config-status\n",
        "  eliot-searchd --health-data-root ROOT\n",
        "  eliot-searchd --self-test\n",
        "  eliot-searchd --stdio\n",
        "  eliot-searchd --serve-data-root ROOT\n\n",
        "CONFIGURATION LAYERS (global, before subcommand):\n",
        "  eliot-searchd [--config-file PATH] [--set section.key=value]... COMMAND\n",
        "  Layers apply as defaults < file < environment < CLI; mixed partial\n",
        "  obligations fail closed with DAEMON_CONFIG_PARTIAL_REFUSED.\n\n",
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
        "PERSISTENT SOURCE-ROOT REGISTRATION:\n",
        "  eliot-searchd --source-roots ROOT\n",
        "  eliot-searchd --register-source-root ROOT DIRECTORY\n",
        "  eliot-searchd --unregister-source-root ROOT DIRECTORY\n",
        "  eliot-searchd --sync-source-roots ROOT\n",
        "Registration controls explicit observation, not access grants or purge.\n",
        "Unregistering does not revoke already retained revisions.\n\n",
        "MAINTENANCE:\n",
        "  eliot-searchd --repair-root ROOT\n",
        "  eliot-searchd --gc-root ROOT --dry-run\n",
        "  eliot-searchd --gc-root ROOT --apply\n\n",
        "Persistent DIRECT search is source-backed by verified immutable ",
        "revision objects. The current development object store is plaintext and ",
        "reports encrypted_at_rest=false.\n",
    )
}

pub(super) fn version_json() -> String {
    format!(
        "{{\"binary\":\"eliot-searchd\",\"version\":\"{}\",\"protocol_version\":{}}}",
        env!("CARGO_PKG_VERSION"),
        PROTOCOL_VERSION,
    )
}
