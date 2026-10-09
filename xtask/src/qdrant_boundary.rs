//! Enforces the replaceable Qdrant adapter boundary.
//!
//! `qdrant-client` is a private implementation dependency of
//! `search-qdrant-bridge`. The rest of the workspace consumes bridge-owned
//! types and may not depend on, import, or publicly expose vendor SDK types.

mod filesystem;
mod manifests;
mod metadata_graph;
mod source;

use std::io::Write;
use std::path::Path;

use serde_json::json;
use toml::Value;

use filesystem::{ScanBudget, collect_files, read_text, read_toml, relative_path};
use manifests::{validate_bridge_inheritance_syntax, validate_workspace_pin_syntax, value_at};
use source::{
    BridgeSource, contains_vendor_sdk_reference, find_cross_file_vendor_surfaces,
    public_vendor_surface_lines, rust_string_constant,
};

const ROOT_MANIFEST: &str = "Cargo.toml";
const BRIDGE_ROOT: &str = "crates/search-index-qdrant/search-qdrant-bridge";
const BRIDGE_MANIFEST: &str = "crates/search-index-qdrant/search-qdrant-bridge/Cargo.toml";
const QUALIFIED_SOURCE: &str = "crates/search-index-qdrant/search-qdrant-bridge/src/qualified.rs";
const ARTIFACT_MANIFEST: &str = "qualification/qdrant/artifact.toml";
const VENDOR_CRATE: &str = "qdrant-client";
const VENDOR_MODULE: &str = "qdrant_client";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QdrantBoundaryReport {
    pub scope: BoundaryScope,
    pub errors: Vec<String>,
    pub manifests_scanned: usize,
    pub rust_files_scanned: usize,
    pub sdk_source_files: Vec<String>,
    pub workspace_client_version: Option<String>,
    pub qualified_server_version: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryScope {
    StaticPolicy,
    CargoAndStaticPolicy,
}

impl QdrantBoundaryReport {
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

#[must_use]
pub const fn exit_code(report: &QdrantBoundaryReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Serialize a typed report with a preallocation guard and a hard output cap.
///
/// # Errors
/// Returns a terminal error for oversized or unserializable output.
pub fn render_report_json(report: &QdrantBoundaryReport) -> Result<String, String> {
    const MAX_REPORT_BYTES: usize = 1024 * 1024;
    let text_bytes = report
        .errors
        .iter()
        .chain(&report.sdk_source_files)
        .chain(report.workspace_client_version.iter())
        .chain(report.qualified_server_version.iter())
        .try_fold(0_usize, |total, text| total.checked_add(text.len()))
        .ok_or("Qdrant report size overflow")?;
    // Six bytes per UTF-8 input byte covers JSON escaping before Value clones.
    if text_bytes > (MAX_REPORT_BYTES - 8192) / 6
        || report.errors.len() + report.sdk_source_files.len() > 1024
    {
        return Err("Qdrant report exceeds bounded output allowance".into());
    }
    let mut sink = ReportSink {
        bytes: Vec::new(),
        limit: MAX_REPORT_BYTES,
    };
    serde_json::to_writer_pretty(
        &mut sink,
        &json!({
            "scope": match report.scope { BoundaryScope::StaticPolicy => "static-policy", BoundaryScope::CargoAndStaticPolicy => "cargo-and-static-policy" },
            "status": if report.passed() { "PASS" } else { "FAIL" },
            "manifests_scanned": report.manifests_scanned,
            "rust_files_scanned": report.rust_files_scanned,
            "sdk_source_files": report.sdk_source_files,
            "workspace_client_version": report.workspace_client_version,
            "qualified_server_version": report.qualified_server_version,
            "errors": report.errors,
        }),
    )
    .map_err(|error| format!("Qdrant report serialization failed: {error}"))?;
    String::from_utf8(sink.bytes).map_err(|error| format!("Qdrant report UTF-8 failed: {error}"))
}

struct ReportSink {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for ReportSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|length| length > self.limit)
        {
            return Err(std::io::Error::other("Qdrant report byte limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Validate Cargo's locked/offline graph, status inventory and static boundary.
#[must_use]
pub fn validate_qdrant_boundary(root: &Path) -> QdrantBoundaryReport {
    let mut report = validate_qdrant_static_boundary(root);
    report.scope = BoundaryScope::CargoAndStaticPolicy;
    match crate::cargo_metadata_adapter::load_inventory(root, false) {
        Ok(inventory) => {
            if let Some(version) = &report.workspace_client_version {
                report
                    .errors
                    .extend(metadata_graph::validate_graph(&inventory, version));
            }
        }
        Err(error) => report.errors.push(error),
    }
    match crate::cargo_metadata_adapter::load_inventory(root, true) {
        Ok(inventory) => report
            .errors
            .extend(crate::package_status::validate_package_status(
                root, &inventory,
            )),
        Err(error) => report.errors.push(error),
    }
    report.errors.sort();
    report.errors.dedup();
    report
}

#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "Static source/artifact checks retain their existing ordering; Rust source replacement is separately owned by #251."
)]
/// Narrow static source/artifact and literal pin/inheritance checks only.
/// This function does not establish Cargo graph or package-status validity.
pub fn validate_qdrant_static_boundary(root: &Path) -> QdrantBoundaryReport {
    let mut report = QdrantBoundaryReport {
        scope: BoundaryScope::StaticPolicy,
        errors: Vec::new(),
        manifests_scanned: 0,
        rust_files_scanned: 0,
        sdk_source_files: Vec::new(),
        workspace_client_version: None,
        qualified_server_version: None,
    };

    let mut budget = ScanBudget::default();
    let mut files = Vec::new();
    collect_files(root, root, 0, &mut budget, &mut files, &mut report.errors);
    files.sort();

    let root_manifest = read_toml(
        &root.join(ROOT_MANIFEST),
        ROOT_MANIFEST,
        &mut budget,
        &mut report.errors,
    );
    let bridge_manifest = read_toml(
        &root.join(BRIDGE_MANIFEST),
        BRIDGE_MANIFEST,
        &mut budget,
        &mut report.errors,
    );
    report.manifests_scanned =
        usize::from(root_manifest.is_some()) + usize::from(bridge_manifest.is_some());
    let mut bridge_sources = Vec::new();

    for path in &files {
        let relative = relative_path(root, path);
        if path
            .extension()
            .is_some_and(|extension| extension == std::ffi::OsStr::new("rs"))
        {
            report.rust_files_scanned += 1;
            let Some(text) = read_text(path, &relative, &mut budget, &mut report.errors) else {
                continue;
            };
            if contains_vendor_sdk_reference(&text) {
                report.sdk_source_files.push(relative.clone());
                if !relative.starts_with(&format!("{BRIDGE_ROOT}/")) {
                    report.errors.push(format!(
                        "{relative}: raw {VENDOR_MODULE} SDK reference escaped \
                         {BRIDGE_ROOT}"
                    ));
                }
                for line in public_vendor_surface_lines(&text) {
                    report.errors.push(format!(
                        "{relative}:{line}: vendor SDK type appears in a public surface"
                    ));
                }
            }
            if relative.starts_with(&format!("{BRIDGE_ROOT}/src/")) {
                bridge_sources.push(BridgeSource::new(relative, text));
            }
        }
    }

    for (relative, line) in
        find_cross_file_vendor_surfaces(&bridge_sources, BRIDGE_ROOT, VENDOR_MODULE)
    {
        report.errors.push(format!(
            "{relative}:{line}: vendor SDK type reaches a public surface through a cross-file alias"
        ));
    }

    report.sdk_source_files.sort();
    report.sdk_source_files.dedup();

    let workspace_version = root_manifest
        .as_ref()
        .and_then(|document| validate_workspace_pin_syntax(document, &mut report.errors));
    report
        .workspace_client_version
        .clone_from(&workspace_version);

    if let Some(document) = bridge_manifest.as_ref() {
        validate_bridge_inheritance_syntax(document, &mut report.errors);
    } else {
        report
            .errors
            .push(format!("{BRIDGE_MANIFEST}: manifest not found"));
    }

    let qualified_text = read_text(
        &root.join(QUALIFIED_SOURCE),
        QUALIFIED_SOURCE,
        &mut budget,
        &mut report.errors,
    );
    let qualified_client_version = qualified_text
        .as_deref()
        .and_then(|text| rust_string_constant(text, "QUALIFIED_CLIENT_VERSION"));
    let qualified_server_version = qualified_text
        .as_deref()
        .and_then(|text| rust_string_constant(text, "QUALIFIED_SERVER_VERSION"));
    report
        .qualified_server_version
        .clone_from(&qualified_server_version);

    let artifact = read_toml(
        &root.join(ARTIFACT_MANIFEST),
        ARTIFACT_MANIFEST,
        &mut budget,
        &mut report.errors,
    );
    let artifact_client_crate = artifact_string(artifact.as_ref(), &["client", "crate_name"]);
    let artifact_client_version = artifact_string(artifact.as_ref(), &["client", "version"]);
    let artifact_server_version = artifact_string(artifact.as_ref(), &["server", "version"]);

    if artifact_client_crate.as_deref() != Some(VENDOR_CRATE) {
        report.errors.push(format!(
            "{ARTIFACT_MANIFEST}: client.crate_name must be {VENDOR_CRATE}"
        ));
    }

    compare_versions(
        "workspace dependency",
        workspace_version.as_deref(),
        QUALIFIED_SOURCE,
        qualified_client_version.as_deref(),
        &mut report.errors,
    );
    compare_versions(
        "workspace dependency",
        workspace_version.as_deref(),
        ARTIFACT_MANIFEST,
        artifact_client_version.as_deref(),
        &mut report.errors,
    );
    compare_versions(
        QUALIFIED_SOURCE,
        qualified_server_version.as_deref(),
        ARTIFACT_MANIFEST,
        artifact_server_version.as_deref(),
        &mut report.errors,
    );

    report
}

fn artifact_string(artifact: Option<&Value>, path: &[&str]) -> Option<String> {
    artifact
        .and_then(|document| value_at(document, path))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn compare_versions(
    left_label: &str,
    left: Option<&str>,
    right_label: &str,
    right: Option<&str>,
    errors: &mut Vec<String>,
) {
    match (left, right) {
        (Some(left), Some(right)) if left == right => {}
        (Some(left), Some(right)) => errors.push(format!(
            "Qdrant version mismatch: {left_label}={left}, \
             {right_label}={right}"
        )),
        (None, _) => errors.push(format!(
            "Qdrant version comparison unavailable: \
             {left_label} has no version"
        )),
        (_, None) => errors.push(format!(
            "Qdrant version comparison unavailable: \
             {right_label} has no version"
        )),
    }
}

#[cfg(test)]
mod report_fixtures {
    use super::*;

    fn report() -> QdrantBoundaryReport {
        QdrantBoundaryReport {
            scope: BoundaryScope::StaticPolicy,
            errors: Vec::new(),
            manifests_scanned: 0,
            rust_files_scanned: 0,
            sdk_source_files: Vec::new(),
            workspace_client_version: None,
            qualified_server_version: None,
        }
    }

    #[test]
    fn report_scope_and_failure_remain_explicit() {
        let mut report = report();
        let json: serde_json::Value =
            serde_json::from_str(&render_report_json(&report).expect("bounded report"))
                .expect("JSON");
        assert_eq!(json["scope"], "static-policy");
        report.scope = BoundaryScope::CargoAndStaticPolicy;
        report.errors.push("Cargo metadata failed".into());
        let json: serde_json::Value =
            serde_json::from_str(&render_report_json(&report).expect("bounded failure"))
                .expect("JSON");
        assert_eq!(json["scope"], "cargo-and-static-policy");
        assert_eq!(json["status"], "FAIL");
    }

    #[test]
    fn report_overflow_is_terminal_before_value_clones() {
        let mut report = report();
        report.errors.push("x".repeat(1024 * 1024));
        assert!(render_report_json(&report).is_err());
        let mut sink = ReportSink {
            bytes: Vec::new(),
            limit: 2,
        };
        assert!(sink.write_all(b"abc").is_err());
        assert!(sink.bytes.is_empty());
    }
}
