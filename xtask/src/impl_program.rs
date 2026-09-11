//! Port of `tools/validate-implementation-program.py` (T41 slice).
//!
//! Read-only validator over the non-authoritative implementation program,
//! the launch/stage/gate/package registries, product-pulse metrics, the
//! coverage manifest, the structural qualification cases and the manual-only
//! workflow. The JSON report shape (`json.dumps(result, indent=2,
//! sort_keys=True)`), the counters, the minimal
//! `{"errors": ..., "status": "FAIL"}` early-failure object and the exit
//! codes (0 pass, 1 fail) mirror the Python tool; the optional `--json` flag
//! is accepted and ignored exactly like the Python `argparse` switch.
//!
//! Deliberate deviations (same class as the `ticket_drafts` port):
//!
//! * the workflow closure token names this Rust entrypoint
//!   (`validate implementation-program`) instead of the retired Python file;
//! * inputs that crash `CPython` with an uncaught `AttributeError`/`TypeError`
//!   (non-table `discipline`/`current_state`/..., non-list `packages`, ...)
//!   record the same FAIL outcome with an explanatory error and exit code 1
//!   instead of a traceback;
//! * unreadable-file early failures keep the `CPython` exception class
//!   (`FileNotFoundError`/`OSError`/`UnicodeDecodeError`/`TOMLDecodeError`)
//!   but carry the Rust I/O or parser text instead of the `CPython` strerror.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

/// Manual-only workflow guarding the program closure.
pub const WORKFLOW: &str = ".github/workflows/implementation-program.yml";
/// Token proving the workflow invokes this Rust entrypoint.
pub const WORKFLOW_XTASK_TOKEN: &str = "validate implementation-program";
/// Central stage order W0-W10.
pub const EXPECTED_STAGE_IDS: [&str; 11] = [
    "W0", "W1", "W2", "W3", "W4", "W5", "W6", "W7", "W8", "W9", "W10",
];
/// Gate registry G0-G6.
pub const EXPECTED_GATE_IDS: [&str; 7] = ["G0", "G1", "G2", "G3", "G4", "G5", "G6"];
/// `(program key, registry path)` pairs in check order.
pub const EXPECTED_PATHS: [(&str, &str); 9] = [
    ("launch_authority", "swarm/launch-state.toml"),
    ("package_registry", "swarm/crates.toml"),
    ("function_registry", "swarm/function-packets.toml"),
    ("module_registry", "swarm/module-packets.toml"),
    ("package_map_index", "swarm/coverage/package-map-index.toml"),
    ("stage_registry", "swarm/stages.toml"),
    ("stage_readset_registry", "swarm/stage-readsets.toml"),
    ("gate_registry", "swarm/gates.toml"),
    ("configuration_registry", "config/sections.toml"),
];
/// (`target id`, `required stage`) pairs.
pub const EXPECTED_TARGETS: [(&str, &str); 6] = [
    ("buildable_workspace", "W0"),
    ("bootable_service_shell", "W1"),
    ("direct_source_product", "W2"),
    ("useful_baseline_search", "W4"),
    ("release_candidate", "W9"),
    ("optional_depth", "W10"),
];
/// Integration bootstrap order.
pub const EXPECTED_INTEGRATION_ORDER: [&str; 5] = [
    "pin_windows_toolchain",
    "lock_dependency_graph",
    "freeze_build_profiles",
    "establish_test_harness",
    "freeze_artifact_and_data_layout",
];
/// First implementation sequence order.
pub const EXPECTED_NEXT_ORDER: [&str; 7] = [
    "integration_bootstrap_pr",
    "search_contracts_implementation",
    "search_contracts_review_and_handoff",
    "search_domain_implementation",
    "search_ports_implementation",
    "w0_g0_evidence_and_acceptance",
    "advance_launch_to_w1",
];
/// Baseline release gate/receipt sequence.
pub const EXPECTED_BASELINE_REQUIRES: [&str; 7] =
    ["G0", "G1", "G2", "G3", "W7_LIFECYCLE", "G4", "G5"];

/// Validator outcome: `complete == false` renders the minimal early-failure
/// object (unreadable registries), exactly like the Python `except` path.
pub struct ProgramReport {
    /// Whether all registries loaded (minimal `{"errors","status"}` when false).
    pub complete: bool,
    /// PASS (no errors) vs FAIL.
    pub passed: bool,
    /// Program stage row count.
    pub stages: usize,
    /// Package registry row count.
    pub packages: usize,
    /// Program target row count.
    pub targets: usize,
    /// Integration step row count.
    pub integration_steps: usize,
    /// Next-step row count.
    pub next_steps: usize,
    /// Baseline release requirement count.
    pub baseline_requirements: usize,
    /// Program `active_stage` (`None` renders `null`).
    pub current_stage: Option<String>,
    /// Program `active_wave` (`None` renders `null`).
    pub current_wave: Option<i64>,
    /// Whether `Cargo.lock` exists on disk.
    pub cargo_lock_present: bool,
    /// Accumulated error messages in check order.
    pub errors: Vec<String>,
}

/// Process exit code: 0 on PASS, 1 on FAIL.
#[must_use]
pub const fn exit_code(report: &ProgramReport) -> i32 {
    if report.complete && report.passed {
        0
    } else {
        1
    }
}

fn append_json_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{09}' => out.push_str("\\t"),
            '\u{0A}' => out.push_str("\\n"),
            '\u{0C}' => out.push_str("\\f"),
            '\u{0D}' => out.push_str("\\r"),
            // `json.dumps` default (`ensure_ascii=True`): short escapes above,
            // lowercase `\uXXXX` otherwise outside printable ASCII.
            c if (c as u32) < 0x20 || (c as u32) > 0x7E => {
                let mut units = [0_u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn append_errors(out: &mut String, errors: &[String]) {
    if errors.is_empty() {
        out.push_str("[]");
        return;
    }
    out.push_str("[\n");
    for (index, error) in errors.iter().enumerate() {
        out.push_str("    ");
        append_json_string(out, error);
        if index + 1 < errors.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]");
}

/// Render the report exactly like `json.dumps(result, indent=2, sort_keys=True)`.
#[must_use]
pub fn render_report_json(report: &ProgramReport) -> String {
    let status = if report.complete && report.passed {
        "PASS"
    } else {
        "FAIL"
    };
    let mut out = String::from("{\n");
    if !report.complete {
        out.push_str("  \"errors\": ");
        append_errors(&mut out, &report.errors);
        out.push_str(",\n  \"status\": \"FAIL\"\n}");
        return out;
    }
    let mut stage = String::from("null");
    if let Some(active) = &report.current_stage {
        stage.clear();
        append_json_string(&mut stage, active);
    }
    let wave = report
        .current_wave
        .map_or_else(|| "null".to_owned(), |value| value.to_string());
    let mut errors = String::new();
    append_errors(&mut errors, &report.errors);
    let mut status_json = String::new();
    append_json_string(&mut status_json, status);
    let lock = if report.cargo_lock_present {
        "true"
    } else {
        "false"
    };
    let fields: [(&str, String); 12] = [
        (
            "baseline_release_requirements",
            report.baseline_requirements.to_string(),
        ),
        ("cargo_lock_present", lock.to_owned()),
        ("current_stage", stage),
        ("current_wave", wave),
        ("errors", errors),
        ("integration_steps", report.integration_steps.to_string()),
        ("next_steps", report.next_steps.to_string()),
        ("packages", report.packages.to_string()),
        ("stages", report.stages.to_string()),
        ("status", status_json),
        ("targets", report.targets.to_string()),
        ("warnings", "[]".to_owned()),
    ];
    for (index, (key, value)) in fields.iter().enumerate() {
        out.push_str("  \"");
        out.push_str(key);
        out.push_str("\": ");
        out.push_str(value);
        if index + 1 < fields.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push('}');
    out
}

// --- TOML access ------------------------------------------------------------

fn load_doc(root: &Path, relative: &str) -> Result<toml::Value, String> {
    match std::fs::read(root.join(relative)) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("FileNotFoundError: {relative}: {err}"))
        }
        Err(err) => Err(format!("OSError: {relative}: {err}")),
        Ok(bytes) => match String::from_utf8(bytes) {
            Err(err) => Err(format!("UnicodeDecodeError: {relative}: {err}")),
            Ok(text) => text
                .parse::<toml::Value>()
                .map_err(|err| format!("TOMLDecodeError: {relative}: {err}")),
        },
    }
}

/// `rows`: array-of-tables keyed by `identity`, duplicate-rejecting, in
/// document order (mirrors the Python `index_rows` insertion order).
fn rows<'a>(
    document: &'a toml::Value,
    key: &str,
    identity: &str,
) -> Result<Vec<(String, &'a toml::Value)>, String> {
    let Some(items) = document.get(key).and_then(toml::Value::as_array) else {
        return Err(format!("ValueError: {key} must be an array of tables"));
    };
    let mut result = Vec::with_capacity(items.len());
    let mut seen = BTreeSet::new();
    for row in items {
        let Some(id) = row.get(identity).and_then(toml::Value::as_str) else {
            return Err(format!("ValueError: invalid {key} row"));
        };
        if !seen.insert(id.to_owned()) {
            return Err(format!("ValueError: duplicate {key} identity: {id}"));
        }
        result.push((id.to_owned(), row));
    }
    Ok(result)
}

/// Table child: missing stays missing (the Python `{}` default fails the
/// downstream checks); present-but-not-a-table records one explanatory error
/// where `CPython` raises an uncaught `AttributeError`.
fn child<'a>(
    value: Option<&'a toml::Value>,
    key: &str,
    errors: &mut Vec<String>,
) -> Option<&'a toml::Value> {
    match value.and_then(|table| table.get(key)) {
        None => None,
        Some(inner @ toml::Value::Table(_)) => Some(inner),
        Some(_) => {
            errors.push(format!("{key} is not a table"));
            None
        }
    }
}

fn get<'a>(table: Option<&'a toml::Value>, key: &str) -> Option<&'a toml::Value> {
    table.and_then(|value| value.get(key))
}

fn is_str(table: Option<&toml::Value>, key: &str, expected: &str) -> bool {
    get(table, key).and_then(toml::Value::as_str) == Some(expected)
}

fn is_int(table: Option<&toml::Value>, key: &str, expected: i64) -> bool {
    get(table, key).and_then(toml::Value::as_integer) == Some(expected)
}

fn is_bool(table: Option<&toml::Value>, key: &str, expected: bool) -> bool {
    get(table, key).and_then(toml::Value::as_bool) == Some(expected)
}

fn is_str_list(table: Option<&toml::Value>, key: &str, expected: &[&str]) -> bool {
    let Some(items) = get(table, key).and_then(toml::Value::as_array) else {
        return false;
    };
    items.len() == expected.len()
        && items
            .iter()
            .zip(expected.iter())
            .all(|(item, want)| item.as_str() == Some(*want))
}

/// Python `blockers.get(key) == 0` (int `0`, float `0.0` and `False` compare
/// equal to zero in `CPython`; bit-exact float comparison mirrors `==`).
const fn is_zero(value: Option<&toml::Value>) -> bool {
    match value {
        Some(toml::Value::Integer(0) | toml::Value::Boolean(false)) => true,
        Some(toml::Value::Float(f)) => {
            f.to_bits() == 0.0_f64.to_bits() || f.to_bits() == (-0.0_f64).to_bits()
        }
        _ => false,
    }
}

/// Python `blockers.get(key) == 1.0` (`1`, `1.0` and `True` compare equal;
/// bit-exact float comparison mirrors `==`).
const fn is_one(value: Option<&toml::Value>) -> bool {
    match value {
        Some(toml::Value::Integer(1) | toml::Value::Boolean(true)) => true,
        Some(toml::Value::Float(f)) => f.to_bits() == 1.0_f64.to_bits(),
        _ => false,
    }
}

fn is_non_blank_str(value: Option<&toml::Value>) -> bool {
    value
        .and_then(toml::Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
}

/// Python f-string rendering of a scalar package identity (bools render
/// `True`/`False` exactly like `CPython`).
fn scalar_text(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => text.clone(),
        toml::Value::Integer(number) => number.to_string(),
        toml::Value::Float(number) => number.to_string(),
        toml::Value::Boolean(true) => "True".to_owned(),
        toml::Value::Boolean(false) => "False".to_owned(),
        _ => format!("{value:?}"),
    }
}

/// Python `str.join` rendering of a sorted string list (`"['a', 'b']"`).
fn python_str_list(items: &BTreeSet<String>) -> String {
    let mut out = String::from("[");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push('\'');
        out.push_str(item);
        out.push('\'');
    }
    out.push(']');
    out
}

fn require(errors: &mut Vec<String>, condition: bool, message: String) {
    if !condition {
        errors.push(message);
    }
}

// --- loaded documents -------------------------------------------------------

struct Docs {
    program: toml::Value,
    launch: toml::Value,
    stages_doc: toml::Value,
    gates_doc: toml::Value,
    packages_doc: toml::Value,
    metrics: toml::Value,
    coverage: toml::Value,
    cases: toml::Value,
}

fn load_all(root: &Path) -> Result<Docs, String> {
    Ok(Docs {
        program: load_doc(root, "swarm/implementation-program.toml")?,
        launch: load_doc(root, "swarm/launch-state.toml")?,
        stages_doc: load_doc(root, "swarm/stages.toml")?,
        gates_doc: load_doc(root, "swarm/gates.toml")?,
        packages_doc: load_doc(root, "swarm/crates.toml")?,
        metrics: load_doc(root, "qualification/product-pulse/metrics.toml")?,
        coverage: load_doc(root, "swarm/coverage/manifest.toml")?,
        cases: load_doc(root, "qualification/implementation-program/cases-v1.toml")?,
    })
}

struct Indexes<'a> {
    program_stages: Vec<(String, &'a toml::Value)>,
    stages: Vec<(String, &'a toml::Value)>,
    gates: Vec<(String, &'a toml::Value)>,
    packages: Vec<(String, &'a toml::Value)>,
    targets: Vec<(String, &'a toml::Value)>,
    integration_steps: Vec<(String, &'a toml::Value)>,
    next_steps: Vec<(String, &'a toml::Value)>,
}

fn index_all(docs: &Docs) -> Result<Indexes<'_>, String> {
    Ok(Indexes {
        program_stages: rows(&docs.program, "stage", "id")?,
        stages: rows(&docs.stages_doc, "stage", "id")?,
        gates: rows(&docs.gates_doc, "gate", "id")?,
        packages: rows(&docs.packages_doc, "package", "name")?,
        targets: rows(&docs.program, "target", "id")?,
        integration_steps: rows(&docs.program, "integration_step", "id")?,
        next_steps: rows(&docs.program, "next_step", "id")?,
    })
}

fn incomplete(message: String) -> ProgramReport {
    ProgramReport {
        complete: false,
        passed: false,
        stages: 0,
        packages: 0,
        targets: 0,
        integration_steps: 0,
        next_steps: 0,
        baseline_requirements: 0,
        current_stage: None,
        current_wave: None,
        cargo_lock_present: false,
        errors: vec![message],
    }
}

// --- checks -----------------------------------------------------------------

fn check_paths(program: &toml::Value, root: &Path, errors: &mut Vec<String>) {
    for (key, expected) in EXPECTED_PATHS {
        require(
            errors,
            get(Some(program), key).and_then(toml::Value::as_str) == Some(expected),
            format!("program path mismatch: {key}"),
        );
        require(
            errors,
            root.join(expected).is_file(),
            format!("program path missing: {expected}"),
        );
    }
}

fn check_identity(program: &toml::Value, errors: &mut Vec<String>) {
    require(
        errors,
        is_int(Some(program), "schema_version", 1),
        "implementation program schema version mismatch".to_owned(),
    );
    require(
        errors,
        is_str(Some(program), "status", "PLANNED_NOT_AUTHORIZED"),
        "implementation program status is not non-authoritative".to_owned(),
    );
    require(
        errors,
        get(Some(program), "source_main_commit")
            .and_then(toml::Value::as_str)
            .is_some_and(|commit| {
                commit.len() == 40
                    && commit
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            }),
        "source main commit is not an exact SHA".to_owned(),
    );
    for key in [
        "implementation_authorized_by_this_program",
        "launch_state_changed",
        "package_acceptance_claimed",
        "gate_or_wave_acceptance_claimed",
        "runtime_evidence_available",
        "product_acceptance_claimed",
    ] {
        require(
            errors,
            is_bool(Some(program), key, false),
            format!("authority/non-claim flag changed: {key}"),
        );
    }
}

fn check_discipline(program: &toml::Value, packages_doc: &toml::Value, errors: &mut Vec<String>) {
    let discipline = child(Some(program), "discipline", errors);
    for key in [
        "one_writer_one_package",
        "one_worktree_one_task",
        "package_write_scope_only",
        "accepted_public_handoffs_only",
    ] {
        require(
            errors,
            is_bool(discipline, key, true),
            format!("discipline invariant disabled: {key}"),
        );
    }
    for key in [
        "dependency_implementation_reads_allowed",
        "package_writer_may_edit_shared_registries",
        "package_writer_may_self_review",
        "package_writer_may_advance_launch_state",
    ] {
        require(
            errors,
            is_bool(discipline, key, false),
            format!("discipline prohibition disabled: {key}"),
        );
    }
    require(
        errors,
        is_str(
            discipline,
            "ordinary_architecture_master_access",
            "exception-only",
        ),
        "architecture access policy mismatch".to_owned(),
    );
    require(
        errors,
        is_int(discipline, "maximum_static_context_files", 16),
        "static context ceiling mismatch".to_owned(),
    );
    require(
        errors,
        is_int(discipline, "normal_handwritten_src_target", 7500),
        "normal source target mismatch".to_owned(),
    );
    require(
        errors,
        is_int(discipline, "mandatory_split_review_lines", 8500),
        "split-review line threshold mismatch".to_owned(),
    );
    require(
        errors,
        is_int(discipline, "hard_handwritten_line_limit", 10_000)
            && is_int(
                Some(packages_doc),
                "hard_handwritten_rust_line_limit",
                10_000,
            ),
        "hard line limit mismatch".to_owned(),
    );
}

struct CurrentView {
    stage: Option<String>,
    wave: Option<i64>,
    lock_present: bool,
}

fn check_current_state(
    program: &toml::Value,
    launch: &toml::Value,
    coverage: &toml::Value,
    root: &Path,
    errors: &mut Vec<String>,
) -> CurrentView {
    let current = child(Some(program), "current_state", errors);
    let draft = child(Some(launch), "draft_control", errors);
    let coverage_state = child(Some(coverage), "current_state", errors);
    let launch_view = Some(launch);
    require(
        errors,
        get(current, "active_stage").and_then(toml::Value::as_str) == Some("P00")
            && get(launch_view, "active_stage").and_then(toml::Value::as_str) == Some("P00"),
        "current active stage mismatch".to_owned(),
    );
    require(
        errors,
        get(current, "active_wave").and_then(toml::Value::as_integer) == Some(0)
            && get(launch_view, "active_wave").and_then(toml::Value::as_integer) == Some(0),
        "current active wave mismatch".to_owned(),
    );
    require(
        errors,
        is_str_list(current, "authorized_packages", &["search-contracts"])
            && is_str_list(launch_view, "authorized_packages", &["search-contracts"]),
        "authorized package mismatch".to_owned(),
    );
    require(
        errors,
        is_str_list(
            current,
            "conditional_packages",
            &["search-domain", "search-ports"],
        ) && is_str_list(
            launch_view,
            "conditional_packages",
            &["search-domain", "search-ports"],
        ),
        "conditional package mismatch".to_owned(),
    );
    for (key, coverage_key) in [
        ("implemented_packages", "implemented_packages"),
        ("materialized_writer_contexts", "materialized_contexts"),
        ("issued_implementation_tickets", "issued_tickets"),
        ("active_writer_leases", "active_leases"),
        ("accepted_package_handoffs", "accepted_package_handoffs"),
        ("accepted_gate_receipts", "accepted_gates"),
        ("accepted_wave_receipts", "accepted_wave_receipts"),
    ] {
        let left = get(current, key).and_then(toml::Value::as_integer);
        let right = if coverage_key == "implemented_packages"
            || coverage_key == "accepted_gates"
            || coverage_key == "accepted_wave_receipts"
        {
            get(coverage_state, coverage_key).and_then(toml::Value::as_integer)
        } else {
            get(draft, coverage_key).and_then(toml::Value::as_integer)
        };
        let message = match key {
            "implemented_packages" => "implemented package count mismatch",
            "materialized_writer_contexts" => "materialized context count mismatch",
            "issued_implementation_tickets" => "issued ticket count mismatch",
            "active_writer_leases" => "active lease count mismatch",
            "accepted_package_handoffs" => "accepted handoff count mismatch",
            "accepted_gate_receipts" => "accepted gate count mismatch",
            _ => "accepted wave count mismatch",
        };
        require(
            errors,
            left == Some(0) && right == Some(0),
            message.to_owned(),
        );
    }
    let lock_present = root.join("Cargo.lock").is_file();
    require(
        errors,
        get(current, "cargo_lock_present").and_then(toml::Value::as_bool) == Some(lock_present),
        "Cargo.lock presence is reported incorrectly".to_owned(),
    );
    for key in [
        "windows_toolchain_selected",
        "qdrant_profile_selected",
        "rust_parser_profile_selected",
        "product_pulse_accepted",
        "optional_depth_selected",
    ] {
        require(
            errors,
            is_bool(current, key, false),
            format!("current unselected state changed: {key}"),
        );
    }
    CurrentView {
        stage: get(current, "active_stage")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        wave: get(current, "active_wave").and_then(toml::Value::as_integer),
        lock_present,
    }
}

fn check_stage_order(
    program_stages: &[(String, &toml::Value)],
    stages: &[(String, &toml::Value)],
    errors: &mut Vec<String>,
) {
    let expected: Vec<String> = EXPECTED_STAGE_IDS
        .iter()
        .map(|id| (*id).to_owned())
        .collect();
    let program_ids: Vec<String> = program_stages.iter().map(|(id, _)| id.clone()).collect();
    let central_ids: Vec<String> = stages.iter().map(|(id, _)| id.clone()).collect();
    require(
        errors,
        central_ids == expected,
        "central stage order is not W0-W10".to_owned(),
    );
    require(
        errors,
        program_ids == expected,
        "program stage order is not W0-W10".to_owned(),
    );
    require(
        errors,
        program_ids.iter().collect::<BTreeSet<_>>() == central_ids.iter().collect::<BTreeSet<_>>(),
        "program/central stage set mismatch".to_owned(),
    );
}

fn expected_closes(source: &toml::Value) -> Vec<String> {
    let mut closes = Vec::new();
    if let Some(completion) = source
        .get("completion_receipt")
        .and_then(toml::Value::as_str)
        && !completion.is_empty()
    {
        closes.push(completion.to_owned());
    }
    if source.get("closes_gate").and_then(toml::Value::as_bool) == Some(true)
        && let Some(contributes) = source
            .get("contributes_to_gate")
            .and_then(toml::Value::as_str)
    {
        closes.push(contributes.to_owned());
    }
    closes
}

fn check_one_stage(
    stage_id: &str,
    row: &toml::Value,
    source: &toml::Value,
    package_names: &BTreeSet<String>,
    covered: &mut BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    require(
        errors,
        row.get("name") == source.get("name"),
        format!("{stage_id}: name mismatch"),
    );
    require(
        errors,
        row.get("required_gates") == source.get("requires_accepted_gates"),
        format!("{stage_id}: gate prerequisite mismatch"),
    );
    require(
        errors,
        row.get("required_receipts") == source.get("requires_accepted_receipts"),
        format!("{stage_id}: receipt prerequisite mismatch"),
    );
    require(
        errors,
        row.get("packages") == source.get("packages"),
        format!("{stage_id}: package set/order mismatch"),
    );
    let have_closes: Option<Vec<String>> = row
        .get("closes")
        .and_then(toml::Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect::<Option<Vec<String>>>()
        });
    require(
        errors,
        have_closes == Some(expected_closes(source)),
        format!("{stage_id}: completion/gate closure mismatch"),
    );
    match row.get("packages") {
        None => {}
        Some(toml::Value::Array(items)) => {
            for package in items {
                match package.as_str() {
                    Some(name) if package_names.contains(name) => {
                        covered.insert(name.to_owned());
                    }
                    _ => errors.push(format!(
                        "{stage_id}: unknown package {}",
                        scalar_text(package)
                    )),
                }
            }
        }
        Some(_) => errors.push(format!("{stage_id}: packages is not an array")),
    }
    require(
        errors,
        is_non_blank_str(row.get("required_product_result")),
        format!("{stage_id}: required product result missing"),
    );
}

fn check_stages(
    program_stages: &[(String, &toml::Value)],
    stages: &[(String, &toml::Value)],
    packages: &[(String, &toml::Value)],
    stages_doc: &toml::Value,
    gates: &[(String, &toml::Value)],
    errors: &mut Vec<String>,
) {
    let program_map: BTreeMap<&str, &toml::Value> = program_stages
        .iter()
        .map(|(id, row)| (id.as_str(), *row))
        .collect();
    let central_map: BTreeMap<&str, &toml::Value> =
        stages.iter().map(|(id, row)| (id.as_str(), *row)).collect();
    let package_names: BTreeSet<String> = packages.iter().map(|(name, _)| name.clone()).collect();
    let empty = toml::Value::Table(toml::map::Map::new());
    let mut covered = BTreeSet::new();
    for stage_id in EXPECTED_STAGE_IDS {
        let row = program_map.get(stage_id).copied().unwrap_or(&empty);
        let source = central_map.get(stage_id).copied().unwrap_or(&empty);
        check_one_stage(stage_id, row, source, &package_names, &mut covered, errors);
    }
    if covered != package_names {
        let mut diff: BTreeSet<String> = package_names.difference(&covered).cloned().collect();
        diff.extend(covered.difference(&package_names).cloned());
        errors.push(format!(
            "program package closure mismatch: {}",
            python_str_list(&diff)
        ));
    }
    require(
        errors,
        stages.len() == EXPECTED_STAGE_IDS.len()
            && get(Some(stages_doc), "stage_count").and_then(toml::Value::as_integer) == Some(11),
        "central stage count mismatch".to_owned(),
    );
    require(
        errors,
        gates
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<BTreeSet<_>>()
            == EXPECTED_GATE_IDS.iter().copied().collect::<BTreeSet<_>>(),
        "gate registry is not G0-G6".to_owned(),
    );
}

fn check_release_boundary(program: &toml::Value, errors: &mut Vec<String>) {
    let boundary = child(Some(program), "release_boundary", errors);
    for (key, expected) in [
        ("first_bootable_stage", "W1"),
        ("first_direct_source_stage", "W2"),
        ("first_useful_search_stage", "W4"),
        ("release_candidate_stage", "W9"),
        ("optional_depth_stage", "W10"),
    ] {
        let message = match key {
            "first_bootable_stage" => "bootable stage mismatch",
            "first_direct_source_stage" => "DIRECT stage mismatch",
            "first_useful_search_stage" => "useful baseline stage mismatch",
            "release_candidate_stage" => "release-candidate stage mismatch",
            _ => "optional-depth stage mismatch",
        };
        require(errors, is_str(boundary, key, expected), message.to_owned());
    }
    require(
        errors,
        is_str_list(
            boundary,
            "baseline_release_requires",
            &EXPECTED_BASELINE_REQUIRES,
        ),
        "baseline release gate/receipt sequence mismatch".to_owned(),
    );
    require(
        errors,
        is_bool(boundary, "baseline_release_requires_g6", false),
        "G6 became a baseline requirement".to_owned(),
    );
    require(
        errors,
        is_bool(boundary, "baseline_release_requires_w10", false),
        "W10 became a baseline requirement".to_owned(),
    );
}

fn check_targets(targets: &[(String, &toml::Value)], errors: &mut Vec<String>) {
    let map: BTreeMap<&str, &toml::Value> = targets
        .iter()
        .map(|(id, row)| (id.as_str(), *row))
        .collect();
    require(
        errors,
        map.keys().copied().collect::<BTreeSet<_>>()
            == EXPECTED_TARGETS
                .iter()
                .map(|(id, _)| *id)
                .collect::<BTreeSet<_>>(),
        "target state set mismatch".to_owned(),
    );
    let empty = toml::Value::Table(toml::map::Map::new());
    for (target_id, stage_id) in EXPECTED_TARGETS {
        let row = map.get(target_id).copied().unwrap_or(&empty);
        require(
            errors,
            row.get("required_stage").and_then(toml::Value::as_str) == Some(stage_id),
            format!("{target_id}: required stage mismatch"),
        );
        require(
            errors,
            is_non_blank_str(row.get("claim")),
            format!("{target_id}: claim missing"),
        );
    }
    let optional = map.get("optional_depth").copied().unwrap_or(&empty);
    require(
        errors,
        optional
            .get("baseline_release_dependency")
            .and_then(toml::Value::as_bool)
            == Some(false),
        "optional depth became a baseline dependency".to_owned(),
    );
}

fn check_slo(program: &toml::Value, metrics: &toml::Value, errors: &mut Vec<String>) {
    let target_slo = child(Some(program), "architecture_targets", errors);
    let metric_slo = child(Some(metrics), "candidate_slo", errors);
    let percentiles = child(Some(metrics), "percentiles", errors);
    for key in [
        "warm_exact_keyword_navigation_p95_ms",
        "warm_single_scope_lexical_p95_ms",
        "warm_cross_repository_comparison_p95_ms",
        "first_useful_progressive_card_ms",
    ] {
        require(
            errors,
            get(target_slo, key) == get(metric_slo, key),
            format!("architecture target mismatch: {key}"),
        );
    }
    require(
        errors,
        get(target_slo, "minimum_percentile_samples").and_then(toml::Value::as_integer) == Some(30)
            && get(percentiles, "minimum_measured_samples").and_then(toml::Value::as_integer)
                == Some(30),
        "percentile sample floor mismatch".to_owned(),
    );
    require(
        errors,
        is_str(target_slo, "status", "TARGET_NOT_MEASURED"),
        "performance targets are overclaimed".to_owned(),
    );
}

fn check_blockers(program: &toml::Value, errors: &mut Vec<String>) {
    let blockers = child(Some(program), "release_hard_blockers", errors);
    for key in [
        "false_complete_negative_claim_count",
        "stale_leakage_count",
        "access_leakage_count",
        "secret_or_content_leakage_count",
        "protocol_resource_leak_count",
    ] {
        require(
            errors,
            is_zero(get(blockers, key)),
            format!("release hard blocker is not zero: {key}"),
        );
    }
    require(
        errors,
        is_one(get(blockers, "required_fault_cell_recovery_rate")),
        "fault recovery acceptance is not 100%".to_owned(),
    );
    for key in [
        "all_mandatory_package_handoffs_required",
        "all_active_leases_closed",
        "all_unreviewed_submissions_closed",
        "windows_install_upgrade_recovery_rollback_uninstall_required",
        "independent_g5_review_required",
    ] {
        require(
            errors,
            is_bool(blockers, key, true),
            format!("release prerequisite disabled: {key}"),
        );
    }
}

fn check_sequence(
    program: &toml::Value,
    key: &str,
    expected: &[&str],
    count: usize,
    order_message: &str,
    ordinal_message: &str,
    errors: &mut Vec<String>,
) {
    let Some(items) = program.get(key).and_then(toml::Value::as_array) else {
        // CPython raises an uncaught `TypeError` here; record FAIL instead.
        errors.push(order_message.to_owned());
        errors.push(ordinal_message.to_owned());
        return;
    };
    let mut ordered: Vec<&toml::Value> = items.iter().collect();
    ordered.sort_by_key(|item| {
        item.get("order")
            .and_then(toml::Value::as_integer)
            .unwrap_or(-1)
    });
    let ids: Vec<&str> = ordered
        .iter()
        .filter_map(|item| item.get("id").and_then(toml::Value::as_str))
        .collect();
    require(errors, ids == expected, order_message.to_owned());
    let orders: Option<Vec<i64>> = items
        .iter()
        .map(|item| item.get("order").and_then(toml::Value::as_integer))
        .collect();
    let mut sorted = orders.clone().unwrap_or_default();
    sorted.sort_unstable();
    let want: Vec<i64> = (1..=i64::try_from(count).unwrap_or(0)).collect();
    require(
        errors,
        orders.is_some() && sorted == want,
        ordinal_message.to_owned(),
    );
}

fn check_coverage_link(coverage: &toml::Value, errors: &mut Vec<String>) {
    require(
        errors,
        is_str(
            Some(coverage),
            "implementation_program",
            "swarm/implementation-program.toml",
        ),
        "coverage manifest does not link the implementation program".to_owned(),
    );
}

fn check_cross_cutting(program: &toml::Value, errors: &mut Vec<String>) {
    let cross = child(Some(program), "cross_cutting", errors);
    for section in [
        "error_model",
        "resources",
        "security",
        "persistence",
        "observability",
        "packaging",
        "testing",
    ] {
        let row = child(cross, section, errors);
        require(
            errors,
            get(row, "required")
                .and_then(toml::Value::as_array)
                .is_some_and(|items| !items.is_empty()),
            format!("cross-cutting requirement set missing: {section}"),
        );
    }
    let testing = child(cross, "testing", errors);
    require(
        errors,
        is_bool(testing, "compile_or_unit_tests_alone_pass_gate", false),
        "compile/unit tests can incorrectly pass a gate".to_owned(),
    );
}

fn check_cases(cases: &toml::Value, errors: &mut Vec<String>) {
    require(
        errors,
        is_str(Some(cases), "status", "STRUCTURAL_NOT_EXECUTED"),
        "qualification case status mismatch".to_owned(),
    );
    require(
        errors,
        is_int(Some(cases), "case_count", 24),
        "qualification case count mismatch".to_owned(),
    );
    let Some(case_rows) = cases.get("case").and_then(toml::Value::as_array) else {
        errors.push("qualification case inventory mismatch".to_owned());
        return;
    };
    require(
        errors,
        case_rows.len() == 24,
        "qualification case inventory mismatch".to_owned(),
    );
    let ids: Vec<Option<&str>> = case_rows
        .iter()
        .map(|row| row.get("id").and_then(toml::Value::as_str))
        .collect();
    require(
        errors,
        ids.iter().collect::<BTreeSet<_>>().len() == 24 && ids.len() == 24,
        "qualification case IDs are not unique".to_owned(),
    );
    require(
        errors,
        case_rows.iter().all(|row| {
            row.get("mandatory").and_then(toml::Value::as_bool) == Some(true)
                && row.get("result").and_then(toml::Value::as_str) == Some("UNAVAILABLE")
        }),
        "qualification cases contain premature evidence".to_owned(),
    );
}

fn check_workflow(root: &Path, errors: &mut Vec<String>) {
    let workflow_path = root.join(WORKFLOW);
    require(
        errors,
        workflow_path.is_file(),
        "implementation program workflow missing".to_owned(),
    );
    let Ok(workflow) = std::fs::read_to_string(&workflow_path) else {
        errors.push(format!("{WORKFLOW} is not readable"));
        return;
    };
    for token in [
        "workflow_dispatch:",
        "contents: read",
        "persist-credentials: false",
        WORKFLOW_XTASK_TOKEN,
    ] {
        require(
            errors,
            workflow.contains(token),
            format!("implementation workflow missing token: {token}"),
        );
    }
    for (trigger, name) in [
        ("\n  push:", "push:"),
        ("\n  pull_request:", "pull_request:"),
        ("\n  pull_request_target:", "pull_request_target:"),
        ("\n  merge_group:", "merge_group:"),
        ("\n  schedule:", "schedule:"),
        ("\n  workflow_run:", "workflow_run:"),
        ("\n  repository_dispatch:", "repository_dispatch:"),
        ("\n  workflow_call:", "workflow_call:"),
    ] {
        require(
            errors,
            !workflow.contains(trigger),
            format!("automatic workflow trigger present: {name}"),
        );
    }
}

/// Read-only implementation-program validation against `root` (repository root).
#[must_use]
pub fn validate_implementation_program(root: &Path) -> ProgramReport {
    let mut errors: Vec<String> = Vec::new();
    let docs = match load_all(root) {
        Ok(docs) => docs,
        Err(message) => return incomplete(message),
    };
    let indexes = match index_all(&docs) {
        Ok(indexes) => indexes,
        Err(message) => return incomplete(message),
    };
    check_paths(&docs.program, root, &mut errors);
    check_identity(&docs.program, &mut errors);
    check_discipline(&docs.program, &docs.packages_doc, &mut errors);
    let current = check_current_state(
        &docs.program,
        &docs.launch,
        &docs.coverage,
        root,
        &mut errors,
    );
    check_stage_order(&indexes.program_stages, &indexes.stages, &mut errors);
    check_stages(
        &indexes.program_stages,
        &indexes.stages,
        &indexes.packages,
        &docs.stages_doc,
        &indexes.gates,
        &mut errors,
    );
    check_release_boundary(&docs.program, &mut errors);
    check_targets(&indexes.targets, &mut errors);
    check_slo(&docs.program, &docs.metrics, &mut errors);
    check_blockers(&docs.program, &mut errors);
    check_sequence(
        &docs.program,
        "integration_step",
        &EXPECTED_INTEGRATION_ORDER,
        EXPECTED_INTEGRATION_ORDER.len(),
        "integration bootstrap order mismatch",
        "integration step ordinals mismatch",
        &mut errors,
    );
    check_sequence(
        &docs.program,
        "next_step",
        &EXPECTED_NEXT_ORDER,
        EXPECTED_NEXT_ORDER.len(),
        "first implementation sequence mismatch",
        "next-step ordinals mismatch",
        &mut errors,
    );
    check_coverage_link(&docs.coverage, &mut errors);
    check_cross_cutting(&docs.program, &mut errors);
    check_cases(&docs.cases, &mut errors);
    check_workflow(root, &mut errors);
    let boundary = child(Some(&docs.program), "release_boundary", &mut errors);
    let baseline_requirements = get(boundary, "baseline_release_requires")
        .and_then(toml::Value::as_array)
        .map_or(0, Vec::len);
    ProgramReport {
        complete: true,
        passed: errors.is_empty(),
        stages: indexes.program_stages.len(),
        packages: indexes.packages.len(),
        targets: indexes.targets.len(),
        integration_steps: indexes.integration_steps.len(),
        next_steps: indexes.next_steps.len(),
        baseline_requirements,
        current_stage: current.stage,
        current_wave: current.wave,
        cargo_lock_present: current.lock_present,
        errors,
    }
}
