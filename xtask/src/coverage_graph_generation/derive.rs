//! Source-to-reviewed-registry drift detection for coverage graph v2.
//!
//! These modules derive identities and source locations only. They never choose
//! a package-local owner. Module routing remains an explicit reviewed input.

mod documentation;
mod operations;
mod registry;
mod source;

use std::path::Path;

use toml::Value;

pub(super) fn validate(
    root: &Path,
    manifest: &Value,
    package_document: &Value,
    operation_document: &Value,
    documentation_document: &Value,
    dependency_document: &Value,
    module_document: &Value,
) -> Vec<String> {
    let mut errors = Vec::new();
    let files = match source::git_files(root) {
        Ok(files) => files,
        Err(error) => return vec![error],
    };
    operations::validate(
        root,
        manifest,
        package_document,
        operation_document,
        &files,
        &mut errors,
    );
    documentation::validate(
        root,
        documentation_document,
        &files,
        &mut errors,
    );
    registry::validate_dependencies(
        package_document,
        dependency_document,
        &mut errors,
    );
    registry::validate_modules(
        root,
        manifest,
        module_document,
        &mut errors,
    );
    errors
}
