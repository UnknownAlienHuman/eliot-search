//! Complete current CLI help text.

use std::env;
use std::process::ExitCode;

/// Intercepts help so the public command list cannot lag behind specialized
/// command modules.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [] => {
            print_help();
            Some(ExitCode::SUCCESS)
        }
        [argument]
            if argument == "--help" || argument == "-h" || argument == "help" =>
        {
            print_help();
            Some(ExitCode::SUCCESS)
        }
        [argument]
            if argument == "--help" || argument == "-h" || argument == "help" =>
        {
            Some(ExitCode::from(2))
        }
        _ => None,
    }
}

fn print_help() {
    print!(
        concat!(
            "eliot-search ",
            env!("CARGO_PKG_VERSION"),
            "\n\n",
            "CONTROL:\n",
            "  eliot-search --help\n",
            "  eliot-search --version\n",
            "  eliot-search health [--daemon PATH]\n",
            "  eliot-search health-data-root ROOT [--daemon PATH]\n",
            "  eliot-search shutdown [--daemon PATH]\n",
            "  eliot-search self-test [--daemon PATH]\n",
            "  eliot-search serve-data-root ROOT [--daemon PATH]\n\n",
            "ONE-SHOT SEARCH:\n",
            "  eliot-search scan-stdin QUERY [--daemon PATH]\n",
            "  eliot-search scan-stdin-ascii-insensitive QUERY [--daemon PATH]\n",
            "  eliot-search scan-file QUERY FILE [--daemon PATH]\n",
            "  eliot-search scan-file-ascii-insensitive QUERY FILE [--daemon PATH]\n\n",
            "PERSISTENT DIRECT CORPUS:\n",
            "  eliot-search index-file ROOT FILE [--daemon PATH]\n",
            "  eliot-search index-directory ROOT DIRECTORY [--daemon PATH]\n",
            "  eliot-search sync-directory ROOT DIRECTORY [--daemon PATH]\n",
            "  eliot-search search-root ROOT QUERY [--daemon PATH]\n",
            "  eliot-search search-root-ascii-insensitive ROOT QUERY [--daemon PATH]\n",
            "  eliot-search list-sources ROOT [--daemon PATH]\n",
            "  eliot-search verify-root ROOT [--daemon PATH]\n",
            "  eliot-search verify-directory-manifests ROOT [--daemon PATH]\n",
            "  eliot-search retire-source ROOT SOURCE_ID [--daemon PATH]\n",
            "  eliot-search read-revision ROOT REVISION_ID START END [--daemon PATH]\n\n",
            "MAINTENANCE:\n",
            "  eliot-search repair-root ROOT [--daemon PATH]\n",
            "  eliot-search gc-root ROOT --dry-run [--daemon PATH]\n",
            "  eliot-search gc-root ROOT --apply [--daemon PATH]\n\n",
            "Output is newline-delimited JSON. Persistent DIRECT matches are ",
            "source-backed by immutable revision readback. The current ",
            "development revision objects are plaintext and report ",
            "encrypted_at_rest=false.\n",
        )
    );
}
