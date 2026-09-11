mod global;
mod package;
mod spec;

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::json;
use toml::Value;

use super::{indexed_rows, integer, load_doc, string};
use global::validate_global;
use package::validate_package;
use spec::PACKAGES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct W3MilestonePacketReport {
    pub complete: bool,
    pub packages: usize,
    pub milestones: usize,
    pub cases: usize,
    pub qdrant_probes: usize,
    pub launch_stage: Option<String>,
    pub launch_wave: Option<i64>,
    pub errors: Vec<String>,
}

impl W3MilestonePacketReport {
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.complete && self.errors.is_empty()
    }

    fn early(error: String) -> Self {
        Self {
            complete: false,
            packages: 0,
            milestones: 0,
            cases: 0,
            qdrant_probes: 0,
            launch_stage: None,
            launch_wave: None,
            errors: vec![error],
        }
    }
}

#[must_use]
pub fn validate_w3_milestone_packets(
    root: &Path,
) -> W3MilestonePacketReport {
    match validate(root) {
        Ok(report) => report,
        Err(error) => W3MilestonePacketReport::early(error),
    }
}

#[must_use]
pub const fn exit_code(report: &W3MilestonePacketReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

#[must_use]
pub fn render_report_json(report: &W3MilestonePacketReport) -> String {
    let value = if report.complete {
        json!({
            "status": if report.passed() { "PASS" } else { "FAIL" },
            "packages": report.packages,
            "milestones": report.milestones,
            "cases": report.cases,
            "qdrant_probes": report.qdrant_probes,
            "launch_stage": report.launch_stage,
            "launch_wave": report.launch_wave,
            "errors": report.errors,
        })
    } else {
        json!({"status": "FAIL", "errors": report.errors})
    };
    serde_json::to_string_pretty(&value)
        .expect("serializing a bounded W3 milestone report cannot fail")
}

fn validate(root: &Path) -> Result<W3MilestonePacketReport, String> {
    let document = load_doc(root, "swarm/w3-milestone-packets.toml")?;
    let agents = indexed_rows(
        &load_doc(root, "swarm/w3-agent-packets.toml")?,
        "package",
        "name",
    )?;
    let launch = load_doc(root, "swarm/launch-state.toml")?;
    let artifact = load_doc(root, "qualification/qdrant/artifact.toml")?;
    let collection =
        load_doc(root, "qualification/qdrant/collection-schema.toml")?;
    let probes = load_doc(root, "qualification/qdrant/probes.toml")?;
    let cases = load_doc(root, "qualification/w3-milestones/cases-v1.toml")?;
    let packet_rows = indexed_rows(&document, "package", "name")?;
    let mut errors = Vec::new();

    let expected_names: BTreeSet<String> =
        PACKAGES.iter().map(|spec| spec.name.to_owned()).collect();
    let actual_names: BTreeSet<String> = packet_rows.keys().cloned().collect();
    if actual_names != expected_names {
        errors.push("package set mismatch".to_owned());
    }

    validate_global(
        root,
        &document,
        &launch,
        &artifact,
        &collection,
        &probes,
        &cases,
        &mut errors,
    );

    let mut all_ids = Vec::new();
    for spec in &PACKAGES {
        validate_package(
            root,
            spec,
            packet_rows.get(spec.name),
            agents.get(spec.name),
            &mut errors,
        );
        all_ids.extend_from_slice(spec.milestones);
    }
    let unique_ids: BTreeSet<&str> = all_ids.iter().copied().collect();
    if all_ids.len() != 36 || unique_ids.len() != 36 {
        errors.push("milestone IDs are not exactly 36 unique values".to_owned());
    }

    let case_rows = cases.get("case").and_then(Value::as_array);
    let probe_rows = probes.get("probe").and_then(Value::as_array);
    Ok(W3MilestonePacketReport {
        complete: true,
        packages: packet_rows.len(),
        milestones: all_ids.len(),
        cases: case_rows.map_or(0, Vec::len),
        qdrant_probes: probe_rows.map_or(0, Vec::len),
        launch_stage: string(&launch, "active_stage").map(str::to_owned),
        launch_wave: integer(&launch, "active_wave"),
        errors,
    })
}
