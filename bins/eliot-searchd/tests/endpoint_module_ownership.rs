use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn endpoint_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/endpoint.rs");
    assert!(entry.contains("#[path = \"endpoint/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "TcpListener",
        "PairingMachine",
        "PairingLedger",
        "fn serve_listener",
        "fn authenticate_connection",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "endpoint implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/endpoint/kernel.rs");
    for module in ["codec", "pairing", "server", "spec", "wire"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use server::serve_loopback_with_source;"));
    assert!(kernel.contains("pub use spec::{"));
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 3_000, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn endpoint_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/endpoint/kernel/spec.rs", "pub trait EndpointKeySource"),
        ("src/endpoint/kernel/wire.rs", "fn read_bounded_line("),
        ("src/endpoint/kernel/codec.rs", "fn encode_challenge("),
        ("src/endpoint/kernel/pairing.rs", "fn authenticate_connection"),
        ("src/endpoint/kernel/server.rs", "pub fn serve_loopback_with_source"),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 16_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden vendor token {forbidden}"
            );
        }
    }

    let spec = read(&root, "src/endpoint/kernel/spec.rs");
    assert!(!spec.contains("TcpListener"));
    assert!(!spec.contains("PairingMachine"));
    assert!(!spec.contains("blake3::"));

    let pairing = read(&root, "src/endpoint/kernel/pairing.rs");
    assert!(pairing.contains("PairingMachine"));
    assert!(pairing.contains("ledger.consume"));
    assert!(pairing.contains("blake3::keyed_hash"));
    assert!(!pairing.contains("TcpListener"));

    let server = read(&root, "src/endpoint/kernel/server.rs");
    assert!(server.contains("TcpListener::bind"));
    assert!(server.contains("authenticate_connection"));
    assert!(server.contains("complete_request"));
    assert!(!server.contains("PairingMachine"));
    assert!(!server.contains("blake3::"));

    let wire = read(&root, "src/endpoint/kernel/wire.rs");
    assert!(wire.contains("ENDPOINT_FRAME_TOO_LARGE"));
    assert!(wire.contains("ENDPOINT_READ_TIMEOUT"));
    assert!(!wire.contains("PairingMachine"));
    assert!(!wire.contains("blake3::"));

    let codec = read(&root, "src/endpoint/kernel/codec.rs");
    assert!(codec.contains("PAIRING_CHALLENGE"));
    assert!(codec.contains("PAIRING_AUTH"));
    assert!(codec.contains("PAIRING_VERIFIED"));
    assert!(!codec.contains("TcpListener"));
}

#[test]
fn endpoint_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/endpoint/kernel/tests.rs");
    assert!(tests.len() < 35_000, "endpoint tests grew to {} bytes", tests.len());
    for case in [
        "binding_digest_is_deterministic_and_role_bound",
        "challenge_line_round_trips_with_strict_parse",
        "full_handshake_proves_mutually_then_dispatches",
        "wrong_key_tampered_proof_and_reused_ledger_entry_fail",
        "ledger_capacity_fails_closed_without_eviction",
        "key_source_failure_fails_the_connection_without_a_challenge",
        "fatal_handler_drops_listener_and_never_dispatches_the_next_queued_command",
        "silent_client_read_timeout_is_typed_and_bounded",
        "real_socket_disconnect_mid_large_response_then_clean_health_has_no_contamination",
        "slow_reader_write_is_bounded_by_write_timeout",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}
