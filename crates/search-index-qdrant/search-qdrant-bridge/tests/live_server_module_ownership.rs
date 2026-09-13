use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn disposable_server_responsibilities_stay_separated() {
    let root = package_root();
    let facade = read(&root, "src/live/server.rs");
    assert!(facade.len() < 2_500, "server facade grew to {} bytes", facade.len());
    for module in ["artifact", "diagnostics", "endpoint", "process"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(!facade.contains("Command::new"));
    assert!(!facade.contains("Sha256"));
    assert!(!facade.contains("Qdrant::from_url"));

    let artifact = read(&root, "src/live/server/artifact.rs");
    assert!(artifact.contains("QUALIFIED_EXE_BYTES"));
    assert!(artifact.contains("QUALIFIED_EXE_SHA256_HEX"));
    assert!(artifact.contains("Sha256"));
    assert!(!artifact.contains("Command::new"));
    assert!(!artifact.contains("Qdrant::from_url"));

    let endpoint = read(&root, "src/live/server/endpoint.rs");
    assert!(endpoint.contains("Qdrant::from_url"));
    assert!(endpoint.contains("127.0.0.1"));
    assert!(endpoint.contains("localhost"));
    assert!(!endpoint.contains("std::fs"));
    assert!(!endpoint.contains("Command::new"));
    assert!(!endpoint.contains("Sha256"));

    let process = read(&root, "src/live/server/process.rs");
    assert!(process.contains("Command::new"));
    assert!(process.contains("--config-path"));
    assert!(process.contains("verify_executable(exe_path)?"));
    assert!(process.contains("read_log_tail"));
    assert!(!process.contains("QUALIFIED_EXE_SHA256_HEX"));
    assert!(!process.contains("Sha256"));

    let diagnostics = read(&root, "src/live/server/diagnostics.rs");
    assert!(diagnostics.contains("LOG_TAIL_BYTES"));
    assert!(diagnostics.contains("SeekFrom::Start"));
    assert!(diagnostics.contains("String::from_utf8_lossy"));
    assert!(!diagnostics.contains("read_to_string"));
    assert!(!diagnostics.contains("Command::new"));
}
