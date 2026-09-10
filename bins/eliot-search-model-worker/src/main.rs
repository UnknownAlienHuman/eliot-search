#![doc = include_str!("../README.md")]
// Binary composition boundary only. No model profile is qualified, so every
// invocation — including any forced-activation flag or environment variable —
// reports UNAVAILABLE on stderr and exits nonzero (invariant 17: unqualified
// profiles are disabled). No worker implementation, network, or inference here.

/// Process exit code for the honestly-unavailable worker (never zero).
const EXIT_UNAVAILABLE: u8 = 2;

/// Content-free announcement: names the disabled typed failure and the
/// one-line qualification path. Contains no input, secrets, or paths.
const UNAVAILABLE_MESSAGE: &str = "eliot-search-model-worker: UNAVAILABLE MODEL_WORKER_PROFILE_DISABLED: no model profile is qualified; qualification requires an accepted P15 report, candidate ADR, and G6 authorization";

fn main() -> std::process::ExitCode {
    // Argv/env are deliberately ignored: forced activation is refused, never honored.
    eprintln!("{UNAVAILABLE_MESSAGE}");
    std::process::ExitCode::from(EXIT_UNAVAILABLE)
}
