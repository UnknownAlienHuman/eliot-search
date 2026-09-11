//! Keeps Qdrant SDK churn inside private transport/probe modules.
//!
//! The package boundary guard prevents cross-crate leakage. These narrower
//! tests keep vendor request/protobuf code out of the bridge's vendor-neutral
//! ownership modules and pin the logical split registered for the package.

use std::fs;
use std::path::{Path, PathBuf};

const BRIDGE_SRC: &str =
    "crates/search-index-qdrant/search-qdrant-bridge/src";
const MODULE_REGISTRY: &str = "swarm/modules/w3.toml";
const MAX_FACADE_LINES: usize = 120;
const SPLIT_REVIEW_LINES: usize = 8_500;
const OWNERSHIP_MODULES: [&str; 9] = [
    "api",
    "config",
    "capability",
    "schema",
    "mutation",
    "readback",
    "query",
    "admin",
    "error",
];

#[test]
fn qdrant_sdk_references_stay_in_private_transport_or_live_probe_code() {
    let root = workspace_root();
    let bridge = root.join(BRIDGE_SRC);
    let mut files = Vec::new();
    collect_rust_files(&bridge, &mut files);

    let mut violations = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&bridge)
            .expect("collected source must remain under bridge")
            .to_string_lossy()
            .replace('\\', "/");
        if allowed_vendor_module(&relative) {
            continue;
        }
        let text = fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
        for (index, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("use qdrant_client")
                || trimmed.contains("use ::qdrant_client")
                || trimmed.contains("extern crate qdrant_client")
                || trimmed.contains("qdrant_client::")
            {
                violations.push(format!("{relative}:{}", index + 1));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Qdrant SDK escaped private transport/probe modules: {violations:?}"
    );
}

#[test]
fn bridge_root_is_a_bounded_facade_over_registered_ownership_modules() {
    let root = workspace_root();
    let bridge = root.join(BRIDGE_SRC);
    let facade_path = bridge.join("lib.rs");
    let facade = fs::read_to_string(&facade_path)
        .unwrap_or_else(|error| panic!("{}: {error}", facade_path.display()));

    assert!(
        facade.lines().count() <= MAX_FACADE_LINES,
        "bridge lib.rs became an implementation monolith again"
    );
    for forbidden in [
        "pub enum BridgeError",
        "pub struct QdrantBridge",
        "impl QdrantBridge",
        "struct CollectionState",
    ] {
        assert!(
            !facade.contains(forbidden),
            "bridge facade owns implementation token: {forbidden}"
        );
    }

    for module in OWNERSHIP_MODULES {
        let declaration = format!("mod {module};");
        assert!(
            facade.contains(&declaration),
            "bridge facade lacks ownership module declaration: {module}"
        );
        let path = bridge.join(format!("{module}.rs"));
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let lines = text.lines().count();
        assert!(
            lines < SPLIT_REVIEW_LINES,
            "{} exceeds the {}-line split-review boundary: {lines}",
            path.display(),
            SPLIT_REVIEW_LINES
        );
    }

    for public_module in ["live", "qualified", "real"] {
        assert!(
            facade.contains(&format!("pub mod {public_module};")),
            "bridge facade lost public module: {public_module}"
        );
    }

    let registry_path = root.join(MODULE_REGISTRY);
    let registry = fs::read_to_string(&registry_path)
        .unwrap_or_else(|error| panic!("{}: {error}", registry_path.display()));
    let package_block = registry
        .split("[[package]]")
        .find(|block| block.contains("name = \"search-qdrant-bridge\""))
        .expect("search-qdrant-bridge module registry block");
    assert!(package_block.contains("module_count = 10"));
    assert!(package_block.contains(
        "modules = [\"lib\", \"api\", \"config\", \"capability\", \"schema\", \"mutation\", \"readback\", \"query\", \"admin\", \"error\"]"
    ));
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member")
}

fn allowed_vendor_module(relative: &str) -> bool {
    relative == "real.rs"
        || relative == "live.rs"
        || relative.starts_with("real/")
        || relative.starts_with("live/")
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
    {
        let entry = entry.expect("bridge source entry must be readable");
        let path = entry.path();
        let file_type = entry
            .file_type()
            .expect("bridge source entry type must be readable");
        if file_type.is_dir() {
            collect_rust_files(&path, files);
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("rs")
        {
            files.push(path);
        }
    }
}
