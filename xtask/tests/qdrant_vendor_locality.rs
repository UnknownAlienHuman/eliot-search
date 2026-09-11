//! Keeps Qdrant SDK churn inside private transport/probe modules.
//!
//! The package boundary guard prevents cross-crate leakage. This narrower test
//! prevents vendor request/protobuf code from spreading through the bridge's
//! vendor-neutral model, oracle or qualification-identity modules.

use std::fs;
use std::path::{Path, PathBuf};

const BRIDGE_SRC: &str =
    "crates/search-index-qdrant/search-qdrant-bridge/src";

#[test]
fn qdrant_sdk_references_stay_in_private_transport_or_live_probe_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member");
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
