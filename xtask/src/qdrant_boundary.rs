//! Enforces the replaceable Qdrant adapter boundary.
//!
//! `qdrant-client` is a private implementation dependency of
//! `search-qdrant-bridge`. The rest of the workspace consumes bridge-owned
//! types and may not depend on, import, or publicly expose vendor SDK types.

mod filesystem;
mod manifests;
mod source;

use std::path::Path;

use serde_json::json;
use toml::Value;

use filesystem::{collect_files, read_text, read_toml, relative_path};
use manifests::{
    collect_vendor_dependency_declarations, lockfile_package_version,
    validate_bridge_dependency, validate_workspace_dependency, value_at,
};
use source::{
    contains_vendor_sdk_reference, public_vendor_surface_lines,
    rust_string_constant,
};

const ROOT_MANIFEST: &str = "Cargo.toml";
const LOCKFILE: &str = "Cargo.lock";
const BRIDGE_ROOT: &str = "crates/search-index-qdrant/search-qdrant-bridge";
const BRIDGE_MANIFEST: &str =
    "crates/search-index-qdrant/search-qdrant-bridge/Cargo.toml";
const QUALIFIED_SOURCE: &str =
    "crates/search-index-qdrant/search-qdrant-bridge/src/qualified.rs";
const ARTIFACT_MANIFEST: &str = "qualification/qdrant/artifact.toml";
const VENDOR_CRATE: &str = "qdrant-client";
const VENDOR_MODULE: &str = "qdrant_client";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QdrantBoundaryReport {
    pub errors: Vec<String>,
    pub manifests_scanned: usize,
    pub rust_files_scanned: usize,
    pub sdk_source_files: Vec<String>,
    pub workspace_client_version: Option<String>,
    pub qualified_server_version: Option<String>,
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

#[must_use]
pub fn render_report_json(report: &QdrantBoundaryReport) -> String {
    serde_json::to_string_pretty(&json!({
        "status": if report.passed() { "PASS" } else { "FAIL" },
        "manifests_scanned": report.manifests_scanned,
        "rust_files_scanned": report.rust_files_scanned,
        "sdk_source_files": report.sdk_source_files,
        "workspace_client_version": report.workspace_client_version,
        "qualified_server_version": report.qualified_server_version,
        "errors": report.errors,
    }))
    .expect("serializing a bounded Qdrant boundary report cannot fail")
}

#[must_use]
pub fn validate_qdrant_boundary(root: &Path) -> QdrantBoundaryReport {
    let mut report = QdrantBoundaryReport {
        errors: Vec::new(),
        manifests_scanned: 0,
        rust_files_scanned: 0,
        sdk_source_files: Vec::new(),
        workspace_client_version: None,
        qualified_server_version: None,
    };

    let mut files = Vec::new();
    collect_files(root, root, &mut files, &mut report.errors);
    files.sort();

    let mut root_manifest: Option<Value> = None;
    let mut bridge_manifest: Option<Value> = None;

    for path in &files {
        let relative = relative_path(root, path);
        if path
            .file_name()
            .is_some_and(|name| name == std::ffi::OsStr::new("Cargo.toml"))
        {
            report.manifests_scanned += 1;
            let Some(document) = read_toml(path, &relative, &mut report.errors)
            else {
                continue;
            };

            let mut declarations = Vec::new();
            collect_vendor_dependency_declarations(
                &document,
                &mut Vec::new(),
                &mut declarations,
            );
            for location in declarations {
                let allowed = (relative == ROOT_MANIFEST
                    && location == "workspace.dependencies.qdrant-client")
                    || (relative == BRIDGE_MANIFEST
                        && location == "dependencies.qdrant-client");
                if !allowed {
                    report.errors.push(format!(
                        "{relative}: vendor dependency declared at {location}; \
                         only the workspace pin and bridge dependency are allowed"
                    ));
                }
            }

            if relative == ROOT_MANIFEST {
                root_manifest = Some(document);
            } else if relative == BRIDGE_MANIFEST {
                bridge_manifest = Some(document);
            }
        }

        if path
            .extension()
            .is_some_and(|extension| extension == std::ffi::OsStr::new("rs"))
        {
            report.rust_files_scanned += 1;
            let Some(text) = read_text(path, &relative, &mut report.errors)
            else {
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
        }
    }

    report.sdk_source_files.sort();
    report.sdk_source_files.dedup();

    let workspace_version = root_manifest.as_ref().and_then(|document| {
        validate_workspace_dependency(document, &mut report.errors)
    });
    report.workspace_client_version.clone_from(&workspace_version);

    if let Some(document) = bridge_manifest.as_ref() {
        validate_bridge_dependency(document, &mut report.errors);
    } else {
        report
            .errors
            .push(format!("{BRIDGE_MANIFEST}: manifest not found"));
    }

    let lock_version = read_toml(
        &root.join(LOCKFILE),
        LOCKFILE,
        &mut report.errors,
    )
    .and_then(|document| lockfile_package_version(&document, VENDOR_CRATE));

    let qualified_text = read_text(
        &root.join(QUALIFIED_SOURCE),
        QUALIFIED_SOURCE,
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
        &mut report.errors,
    );
    let artifact_client_crate =
        artifact_string(&artifact, &["client", "crate_name"]);
    let artifact_client_version =
        artifact_string(&artifact, &["client", "version"]);
    let artifact_server_version =
        artifact_string(&artifact, &["server", "version"]);

    if artifact_client_crate.as_deref() != Some(VENDOR_CRATE) {
        report.errors.push(format!(
            "{ARTIFACT_MANIFEST}: client.crate_name must be {VENDOR_CRATE}"
        ));
    }

    compare_versions(
        "workspace dependency",
        workspace_version.as_deref(),
        LOCKFILE,
        lock_version.as_deref(),
        &mut report.errors,
    );
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

fn artifact_string(
    artifact: &Option<Value>,
    path: &[&str],
) -> Option<String> {
    artifact
        .as_ref()
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
