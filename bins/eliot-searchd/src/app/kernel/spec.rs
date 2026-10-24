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
        "SHELL CONTROL:\n",
        "  eliot-searchd --help\n",
        "  eliot-searchd --version\n",
        "  eliot-searchd --health\n",
        "  eliot-searchd --config-status\n",
        "  eliot-searchd --self-test\n",
        "  eliot-searchd --stdio\n",
        "self-test checks in-process helpers only, not storage or Qdrant.\n",
        "stdio accepts health, version and shutdown; it is not indexed Search.\n\n",
        "CONFIGURATION LAYERS (global, before subcommand):\n",
        "  eliot-searchd [--config-file PATH] [--set section.key=value]... COMMAND\n",
        "  Layers apply as defaults < file < environment < CLI; mixed partial\n",
        "  obligations fail closed with DAEMON_CONFIG_PARTIAL_REFUSED.\n\n",
        "ONE-SHOT SEARCH OVER CURRENT BYTES:\n",
        "  eliot-searchd --scan-stdin QUERY\n",
        "  eliot-searchd --scan-stdin-ascii-insensitive QUERY\n",
        "  eliot-searchd --scan-file QUERY FILE\n",
        "  eliot-searchd --scan-file-ascii-insensitive QUERY FILE\n",
        "This does not retain a revision or use an index.\n\n",
        "LEGACY ROOT COMMAND NAMES (may be intercepted or refused):\n",
        "  eliot-searchd --health-data-root ROOT\n",
        "  eliot-searchd --serve-data-root ROOT\n",
        "  eliot-searchd --search-root ROOT QUERY\n",
        "  eliot-searchd --search-root-ascii-insensitive ROOT QUERY\n",
        "  eliot-searchd --list-sources ROOT\n",
        "  eliot-searchd --verify-root ROOT\n",
        "  eliot-searchd --read-revision ROOT REVISION_ID START END\n\n",
        "LEGACY SOURCE REGISTRATION AND PREPARATION:\n",
        "  eliot-searchd --source-roots ROOT\n",
        "  eliot-searchd --register-source-root ROOT DIRECTORY\n",
        "  eliot-searchd --unregister-source-root ROOT DIRECTORY\n",
        "  eliot-searchd --sync-source-roots ROOT\n",
        "  eliot-searchd --prepare-revision ROOT REVISION_ID\n",
        "  eliot-searchd --prepare-root ROOT [CURSOR]\n",
        "Registration observes selected roots; it does not grant access or purge retained data.\n",
        "Preparation consumes retained bytes; these paths are not a connected indexed application.\n\n",
        "LEGACY OFFLINE MIGRATION STAGING:\n",
        "  eliot-searchd --plan-control-migration ROOT TARGET_NAMESPACE_UUID OUTPUT_DIRECTORY\n",
        "  eliot-searchd --bootstrap-control-migration ROOT TARGET_NAMESPACE_UUID OUTPUT_DIRECTORY\n",
        "  eliot-searchd --stage-source-control-migration ROOT TARGET_NAMESPACE_UUID OUTPUT_DIRECTORY\n",
        "These require an existing unowned root and a separate existing output directory.\n\n",
        "The persistent source -> Qdrant -> CLI Search path is not connected.\n",
        "Legacy write and cleanup commands are unavailable.\n",
        "TCP ports, endpoint files and token files are not accepted by the primary build.\n",
        "Help and self-test do not establish storage mode, handler readiness or persistence.\n",
    )
}

pub(super) fn version_json() -> String {
    format!(
        "{{\"binary\":\"eliot-searchd\",\"version\":\"{}\",\"protocol_version\":{}}}",
        env!("CARGO_PKG_VERSION"),
        PROTOCOL_VERSION,
    )
}
