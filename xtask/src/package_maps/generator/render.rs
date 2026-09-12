//! Deterministic package-map TOML and Markdown rendering.

mod indexes;
mod package;

use std::collections::BTreeMap;

use super::model::PackageMapModel;
use super::super::{DOC_INDEX_PATH, HUMAN_INDEX_PATH, INDEX_PATH, INTEGRATION_PATH};

pub(super) fn render_outputs(model: &PackageMapModel) -> BTreeMap<String, String> {
    let mut outputs = BTreeMap::new();
    for package in &model.packages {
        let _ = outputs.insert(
            package.paths.overview.clone(),
            package::render_overview(package),
        );
        let _ = outputs.insert(
            package.paths.operations.clone(),
            package::render_operations(package),
        );
        let _ = outputs.insert(
            package.paths.documents.clone(),
            package::render_documents(package),
        );
        let _ = outputs.insert(
            package.paths.relations.clone(),
            package::render_relations(package),
        );
    }
    let _ = outputs.insert(
        INTEGRATION_PATH.to_owned(),
        indexes::render_integration(model),
    );
    let _ = outputs.insert(
        DOC_INDEX_PATH.to_owned(),
        indexes::render_document_index(model),
    );
    let index = indexes::render_package_index(model, &outputs);
    let _ = outputs.insert(INDEX_PATH.to_owned(), index);
    let _ = outputs.insert(
        HUMAN_INDEX_PATH.to_owned(),
        indexes::render_human_index(model),
    );
    outputs
}

pub(super) fn quote(value: &str) -> String {
    crate::coverage_graph::quote_json(value)
}

pub(super) fn array(values: &[String]) -> String {
    let refs: Vec<&str> = values.iter().map(String::as_str).collect();
    crate::coverage_graph::arr(&refs)
}

pub(super) fn finish(lines: Vec<String>) -> String {
    let mut output = lines.join("\n");
    while output.ends_with('\n') {
        let _ = output.pop();
    }
    output.push('\n');
    output
}
