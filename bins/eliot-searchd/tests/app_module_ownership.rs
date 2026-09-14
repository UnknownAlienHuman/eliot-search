use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn app_entry_is_a_thin_binary_facade() {
    let root = crate_root();
    let entry = read(&root, "src/app.rs");

    assert!(entry.contains("#[path = \"app/kernel.rs\"]"));
    assert!(entry.contains("mod kernel;"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "facade grew to {} bytes", entry.len());

    for forbidden in [
        "fn serve_control",
        "fn cmd_index_file",
        "pub fn run_main",
        "DirectStore",
        "std::process::ExitCode",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "command implementation returned to app facade: {forbidden}"
        );
    }
}

#[test]
fn app_kernel_retains_the_existing_command_surface() {
    let root = crate_root();
    let kernel = read(&root, "src/app/kernel.rs");

    for marker in [
        "pub fn run_main",
        "fn serve_control",
        "fn cmd_index_file",
        "fn cmd_index_directory",
        "fn cmd_search_root",
        "fn cmd_gc_root",
        "mod tests",
    ] {
        assert!(kernel.contains(marker), "kernel lost marker {marker}");
    }
    assert_eq!(kernel.matches("pub fn run_main").count(), 1);
    assert!(kernel.len() < 35_000, "app kernel grew to {} bytes", kernel.len());
    for forbidden in ["qdrant_client", "reqwest::", "tokio::"] {
        assert!(
            !kernel.contains(forbidden),
            "binary command owner acquired forbidden transport token {forbidden}"
        );
    }
}
