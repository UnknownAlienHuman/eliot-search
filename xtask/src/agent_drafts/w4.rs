mod global;
mod manifests;
mod package;
mod spec;

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::json;
use toml::Value;

use super::{
    indexed_rows, integer, load_doc, string, symmetric_difference,
};
use global::validate_global;
use manifests::validate_manifests;
use package::validate_package;
use spec::PACKAGES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct W4AgentDraftReport {
    pub complete: bool,
    pub packages: usize,
    pub ticket_drafts: usize,
    pub context_drafts: usize,
    pub qualification_cases: usize,
    pub query_probes: usize,
    pub launch_stage: Option<String>,
    pub launch_wave: Option<i64>,
    pub errors: Vec<String>,
}

impl W4AgentDraftReport {
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.complete && self.errors.is_empty()
    }

    fn early(error: String) -> Self {
        Self {
            complete: false,
            packages: 0,
            ticket_drafts: 0,
            context_drafts: 0,
            qualification_cases: 0,
            query_probes: 0,
            launch_stage: None,
            launch_wave: None,
            errors: vec![error],
        }
    }
}

#[must_use]
pub fn validate_w4_agent_drafts(root: &Path) -> W4AgentDraftReport {
    match validate(root) {
        Ok(report) => report,
        Err(error) => W4AgentDraftReport::early(error),
    }
}

#[must_use]
pub const fn exit_code(report: &W4AgentDraftReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

#[must_use]
pub fn render_report_json(report: &W4AgentDraftReport) -> String {
    let value = if report.complete {
        json!({
            "status": if report.passed() { "PASS" } else { "FAIL" },
            "packages": report.packages,
            "ticket_drafts": report.ticket_drafts,
            "context_drafts": report.context_drafts,
            "qualification_cases": report.qualification_cases,
            "query_probes": report.query_probes,
            "launch_stage": report.launch_stage,
            "launch_wave": report.launch_wave,
            "errors": report.errors,
        })
    } else {
        json!({"status": "FAIL", "errors": report.errors})
    };
    serde_json::to_string_pretty(&value)
        .expect("serializing a bounded W4 agent-draft report cannot fail")
}

fn validate(root: &Path) -> Result<W4AgentDraftReport, String> {
    let packet_doc = load_doc(root, "swarm/w4-agent-packets.toml")?;
    let ticket_manifest =
        load_doc(root, "swarm/ticket-drafts/w4/manifest.toml")?;
    let context_manifest =
        load_doc(root, "swarm/context-drafts/w4/manifest.toml")?;
    let crates = indexed_rows(
        &load_doc(root, "swarm/crates.toml")?,
        "package",
        "name",
    )?;
    let functions = indexed_rows(
        &load_doc(root, "swarm/function-packets.toml")?,
        "package",
        "name",
    )?;
    let stages = indexed_rows(
        &load_doc(root, "swarm/stages.toml")?,
        "stage",
        "id",
    )?;
    let readsets = indexed_rows(
        &load_doc(root, "swarm/stage-readsets.toml")?,
        "override",
        "id",
    )?;
    let launch = load_doc(root, "swarm/launch-state.toml")?;
    let baseline = load_doc(root, "qualification/query/baseline.toml")?;
    let probes = load_doc(root, "qualification/query/probes.toml")?;
    let cases =
        load_doc(root, "qualification/w4-agent-drafts/cases-v1.toml")?;
    let packet_rows = indexed_rows(&packet_doc, "package", "name")?;
    let ticket_rows = indexed_rows(&ticket_manifest, "draft", "package")?;
    let context_rows =
        indexed_rows(&context_manifest, "draft", "package")?;

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

    validate_global(
        &packet_doc,
        &stages,
        &readsets,
        &launch,
        &baseline,
        &probes,
        &mut errors,
    );

    for spec in &PACKAGES {
        let ticket = match load_doc(
            root,
            &format!("swarm/ticket-drafts/w4/{}.toml", spec.name),
        ) {
            Ok(document) => document,
            Err(error) => {
                errors.push(format!(
                    "{}: unable to load ticket: {error}",
                    spec.name
                ));
                continue;
            }
        };
        let context = match load_doc(
            root,
            &format!("swarm/context-drafts/w4/{}.toml", spec.name),
        ) {
            Ok(document) => document,
            Err(error) => {
                errors.push(format!(
                    "{}: unable to load context: {error}",
                    spec.name
                ));
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
    let probe_rows = probes.get("probe").and_then(Value::as_array);
    Ok(W4AgentDraftReport {
        complete: true,
        packages: PACKAGES.len(),
        ticket_drafts: ticket_rows.len(),
        context_drafts: context_rows.len(),
        qualification_cases: case_rows.map_or(0, Vec::len),
        query_probes: probe_rows.map_or(0, Vec::len),
        launch_stage: string(&launch, "active_stage").map(str::to_owned),
        launch_wave: integer(&launch, "active_wave"),
        errors,
    })
}
