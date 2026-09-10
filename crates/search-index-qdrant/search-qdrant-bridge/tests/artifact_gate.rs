//! T22 artifact/client admission gate: failing-first contract tests.
//!
//! The exact qualified pair is Qdrant server 1.19.0 (build 74f3e85b,
//! `x86_64-pc-windows-msvc`) with Rust client `qdrant-client` 1.19.0.
//! Anything else must fail admission — including patch drift.

use search_qdrant_bridge::qualified::{
    ObservedArtifact, ObservedClient, QUALIFIED_ARCH, QUALIFIED_CLIENT_CHECKSUM,
    QUALIFIED_CLIENT_VERSION, QUALIFIED_EXE_BYTES, QUALIFIED_EXE_SHA256_HEX, QUALIFIED_OS,
    QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION, QualificationError, verify_artifact,
    verify_client,
};

fn observed_artifact() -> ObservedArtifact {
    ObservedArtifact {
        version: QUALIFIED_SERVER_VERSION.to_owned(),
        build: QUALIFIED_SERVER_BUILD.to_owned(),
        exe_sha256_hex: QUALIFIED_EXE_SHA256_HEX.to_owned(),
        exe_bytes: QUALIFIED_EXE_BYTES,
        arch: QUALIFIED_ARCH.to_owned(),
        os: QUALIFIED_OS.to_owned(),
    }
}

fn observed_client() -> ObservedClient {
    ObservedClient {
        crate_name: "qdrant-client".to_owned(),
        version: QUALIFIED_CLIENT_VERSION.to_owned(),
        source_checksum: QUALIFIED_CLIENT_CHECKSUM.to_owned(),
    }
}

#[test]
fn exact_artifact_and_client_admit() {
    let artifact = verify_artifact(&observed_artifact()).expect("exact artifact admits");
    assert_eq!(artifact.server_version, "1.19.0");
    assert_eq!(artifact.server_build, "74f3e85b");
    let client = verify_client(&observed_client()).expect("exact client admits");
    assert_eq!(client.client_version, "1.19.0");
}

#[test]
fn wrong_server_version_rejected() {
    let mut observed = observed_artifact();
    observed.version = "1.19.1".to_owned();
    assert_eq!(
        verify_artifact(&observed).expect_err("patch drift must reject"),
        QualificationError::ServerVersionMismatch
    );
}

#[test]
fn wrong_server_build_rejected() {
    let mut observed = observed_artifact();
    observed.build = "deadbee0".to_owned();
    assert_eq!(
        verify_artifact(&observed).expect_err("build drift must reject"),
        QualificationError::ServerBuildMismatch
    );
}

#[test]
fn wrong_executable_hash_rejected() {
    let mut observed = observed_artifact();
    observed.exe_sha256_hex =
        "0000000000000000000000000000000000000000000000000000000000000000".to_owned();
    assert_eq!(
        verify_artifact(&observed).expect_err("hash drift must reject"),
        QualificationError::ArtifactDigestMismatch
    );
}

#[test]
fn wrong_executable_size_rejected() {
    let mut observed = observed_artifact();
    observed.exe_bytes = QUALIFIED_EXE_BYTES + 1;
    assert_eq!(
        verify_artifact(&observed).expect_err("size drift must reject"),
        QualificationError::ArtifactSizeMismatch
    );
}

#[test]
fn wrong_architecture_rejected() {
    let mut observed = observed_artifact();
    observed.arch = "aarch64".to_owned();
    assert_eq!(
        verify_artifact(&observed).expect_err("arch drift must reject"),
        QualificationError::ArchitectureMismatch
    );
}

#[test]
fn wrong_os_rejected() {
    let mut observed = observed_artifact();
    observed.os = "linux".to_owned();
    assert_eq!(
        verify_artifact(&observed).expect_err("os drift must reject"),
        QualificationError::OsMismatch
    );
}

#[test]
fn older_client_minor_rejected_despite_lenient_upstream_check() {
    // Upstream `is_compatible` tolerates +-1 minor with a warning; the
    // bridge gate must not: 1.18.x predates the 1.19 proto fields the
    // qualification relies on (`search_params.idf.corpus`).
    let mut observed = observed_client();
    observed.version = "1.18.0".to_owned();
    assert_eq!(
        verify_client(&observed).expect_err("minor drift must reject"),
        QualificationError::ClientVersionMismatch
    );
}

#[test]
fn wrong_client_checksum_rejected() {
    let mut observed = observed_client();
    observed.source_checksum =
        "0000000000000000000000000000000000000000000000000000000000000000".to_owned();
    assert_eq!(
        verify_client(&observed).expect_err("checksum drift must reject"),
        QualificationError::ClientChecksumMismatch
    );
}
