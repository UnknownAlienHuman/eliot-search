mod global;
mod manifests;
mod package;
mod spec;

use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use super::{
    AgentDraftReport, indexed_rows, integer, load_doc, string,
    symmetric_difference,
};
use global::validate_global;
use manifests::validate_manifests;
use package::validate_package;
use spec::PACKAGES;

pub(super) fn validate_w2_agent_drafts(root: &Path) -> AgentDraftReport {
    match validate(root) {
        Ok(report) => report,
        Err(error) => AgentDraftReport::early(error),
    }
}

fn validate(root: &Path) -> Result<AgentDraftReport, String> {
    let packet_doc = load_doc(root, "swarm/w2-agent-packets.toml")?;
    let ticket_manifest = load_doc(root, "swarm/ticket-drafts/w2/manifest.toml")?;
    let context_manifest = load_doc(root, "swarm/context-drafts/w2/manifest.toml")?;
    let crates = indexed_rows(&load_doc(root, "swarm/crates.toml")?, "package", "name")?;
    let functions = indexed_rows(
        &load_doc(root, "swarm/function-packets.toml")?,
        "package",
        "name",
    )?;
    let stages = indexed_rows(&load_doc(root, "swarm/stages.toml")?, "stage", "id")?;
    let overrides = indexed_rows(
        &load_doc(root, "swarm/stage-readsets.toml")?,
        "override",
        "id",
    )?;
    let launch = load_doc(root, "swarm/launch-state.toml")?;
    let cases = load_doc(root, "qualification/w2-agent-drafts/cases-v1.toml")?;
    let packet_rows = indexed_rows(&packet_doc, "package", "name")?;
    let ticket_rows = indexed_rows(&ticket_manifest, "draft", "package")?;
    let context_rows = indexed_rows(&context_manifest, "draft", "package")?;

    let expected_names: BTreeSet<String> =
        PACKAGES.iter().map(|spec| spec.name.to_owned()).collect();
    let mut errors = Vec::new();
    for (label, rows) in [
        ("packet", &packet_rows),
        ("ticket", &ticket_rows),
        ("context", &context_rows),
    ] {
        let actual: BTreeSet<String> = rows.keys().cloned().collect();
        if actual != expected_names {
            errors.push(format!(
                "{label} package set mismatch: {:?}",
                symmetric_difference(&actual, &expected_names)
            ));
        }
    }

    validate_global(&packet_doc, &stages, &overrides, &launch, &mut errors);

    for spec in &PACKAGES {
        let ticket = match load_doc(
            root,
            &format!("swarm/ticket-drafts/w2/{}.toml", spec.name),
        ) {
            Ok(document) => document,
            Err(error) => {
                errors.push(format!("{}: unable to load drafts: {error}", spec.name));
                continue;
            }
        };
        let context = match load_doc(
            root,
            &format!("swarm/context-drafts/w2/{}.toml", spec.name),
        ) {
            Ok(document) => document,
            Err(error) => {
                errors.push(format!("{}: unable to load drafts: {error}", spec.name));
                continue;
            }
        };
        validate_package(
            root,
            spec,
            packet_rows.get(spec.name),
            crates.get(spec.name),
            functions.get(spec.name),
            &ticket,
            &context,
            &mut errors,
        );
    }

    validate_manifests(
        &packet_doc,
        &ticket_manifest,
        &context_manifest,
        &cases,
        root,
        &mut errors,
    );

    let case_rows = cases.get("case").and_then(Value::as_array);
    Ok(AgentDraftReport {
        complete: true,
        packages: packet_rows.len(),
        ticket_drafts: ticket_rows.len(),
        context_drafts: context_rows.len(),
        qualification_cases: case_rows.map_or(0, Vec::len),
        launch_stage: string(&launch, "active_stage").map(str::to_owned),
        launch_wave: integer(&launch, "active_wave"),
        errors,
    })
}
