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
        "CONFIGURATION (existing consumers only):\n",
        "  eliot-searchd [--config-file PATH] [--set section.key=value]... COMMAND\n",
        "  Native root commands accept [--config-file PATH] [--data-root ROOT] [ROOT]\n",
        "  flags. Only data_root is\n",
        "  consumed; --set and diagnostic_limit are\n",
        "  rejected until a real consumer exists.\n\n",
        "ONE-SHOT SEARCH:\n",
        "  eliot-searchd --scan-stdin QUERY\n",
        "  eliot-searchd --scan-stdin-ascii-insensitive QUERY\n",
        "  eliot-searchd --scan-file QUERY FILE\n",
        "  eliot-searchd --scan-file-ascii-insensitive QUERY FILE\n\n",
        "PERSISTENT DIRECT CORPUS:\n",
        "  eliot-searchd --search-root ROOT QUERY\n",
        "  eliot-searchd --search-root-ascii-insensitive ROOT QUERY\n",
        "  eliot-searchd --list-sources ROOT\n",
        "  eliot-searchd --verify-root ROOT\n",
        "  eliot-searchd --read-revision ROOT REVISION_ID START END\n\n",
        "PERSISTENT SOURCE-ROOT REGISTRATION:\n",
        "  eliot-searchd --source-roots ROOT\n",
        "  eliot-searchd --register-source-root ROOT DIRECTORY\n",
        "  eliot-searchd --unregister-source-root ROOT DIRECTORY\n",
        "  eliot-searchd --sync-source-roots ROOT\n",
        "Registration controls explicit observation, not access grants or purge.\n",
        "Unregistering does not revoke already retained revisions.\n\n",
        "RETAINED PREPARATION:\n",
        "  eliot-searchd --prepare-revision ROOT REVISION_ID\n",
        "  eliot-searchd --prepare-root ROOT [CURSOR]\n",
        "Preparation is explicit; search never rebuilds it.\n\n",
        "PLAIN NATIVE ROOT COMMANDS:\n",
        "  eliot-searchd --inspect-data-root [ROOT] [--config-file PATH] [--data-root ROOT]\n",
        "  eliot-searchd --check-existing-root-owner [ROOT] [--config-file PATH] [--data-root ROOT]\n",
        "  eliot-searchd --initialize-data-root [ROOT] [--config-file PATH] [--data-root ROOT]\n",
        "  eliot-searchd --discard-incomplete-data-root [ROOT] [--config-file PATH] [--data-root ROOT]\n\n",
        "OFFLINE SOURCE-MAPPING COMMANDS:\n",
        "  eliot-searchd --plan-control-migration ROOT NAMESPACE_UUID OUTPUT_DIRECTORY\n",
        "  eliot-searchd --bootstrap-control-migration ROOT NAMESPACE_UUID OUTPUT_DIRECTORY\n",
        "  eliot-searchd --stage-source-control-migration ROOT NAMESPACE_UUID OUTPUT_DIRECTORY\n\n",
        "Legacy write and cleanup commands are unavailable.\n",
        "These are existing DIRECT handlers, not the completed plain-storage/Qdrant product.\n",
        "Platform and root-format restrictions remain; unsupported roots are not converted.\n",
        "The loopback proxy requires the explicit legacy-loopback-harness feature.\n",

    )
}

pub(super) fn version_json() -> String {
    format!(
        "{{\"binary\":\"eliot-searchd\",\"version\":\"{}\",\"protocol_version\":{}}}",
        env!("CARGO_PKG_VERSION"),
        PROTOCOL_VERSION,
    )
}
