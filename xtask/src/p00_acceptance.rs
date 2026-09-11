//! Port of `tools/validate-p00-foundation-acceptance.py` (T41 slice).
//!
//! Read-only validator over the non-authoritative P00 foundation acceptance
//! registry, the package/gate/stage/launch/orchestration registries, the
//! non-claimable schema-v2 drafts, the zero-state control roots, the
//! acceptance matrix docs and the manual-only workflows. A successful run
//! proves structural agreement only; it never accepts a package, G0, W0 or
//! W1 authority.
//!
//! The JSON report shape (`json.dumps(report, ensure_ascii=False,
//! sort_keys=True, separators=(",", ":"))`), the check identifiers/details,
//! the accumulated `"<id>: <detail>"` errors and the exit codes (0 pass,
//! 1 fail) mirror the Python tool.
//!
//! Deliberate deviations (presentation only, same class as the earlier T41
//! ports):
//!
//! * output is always the canonical compact JSON report, exactly like the
//!   Python `--json` branch; the `--json` CLI flag is accepted and ignored
//!   (every workflow call site already passes `-Json`);
//! * inputs that crash `CPython` with an uncaught exception (undecodable
//!   bytes, a directory where a workflow file is expected) record the same
//!   FAIL outcome with an explanatory error and exit code 1 instead of a
//!   traceback;
//! * unreadable TOML inputs keep the `CPython` branch identifiers
//!   (`file:`/`toml:`) but carry the Rust I/O or parser text instead of the
//!   `CPython` strerror (only observable on already-invalid inputs).

use std::path::Path;

/// P00 foundation packages in exact topological order.
pub const EXPECTED_PACKAGES: [&str; 3] = ["search-contracts", "search-domain", "search-ports"];
/// W1 packages unlocked only after accepted G0 and W0.
pub const EXPECTED_W1_PACKAGES: [&str; 7] = [
    "search-config",
    "search-runtime-owner",
    "search-os-secrets",
    "search-control-redb",
    "search-provider-protocol",
    "eliot-searchd",
    "eliot-search",
];
/// Complete context-to-handoff control-record ladder per package.
pub const EXPECTED_CHAIN: [&str; 7] = [
    "context_manifest_v1",
    "assignment_ticket_v1",
    "writer_lease_v1",
    "lease_event_v1:ACKNOWLEDGED",
    "package_submission_v1",
    "independent_review_v1:ACCEPT_SUBMISSION_FOR_INTEGRATION",
    "package_handoff_v1",
];
/// Ordered G0 evidence set shared with the gate registry.
pub const EXPECTED_G0: [&str; 10] = [
    "architecture_hash_challenge",
    "workspace_registry_assignment_parity",
    "dependency_graph_acyclic",
    "dependency_direction_policy",
    "recipe_set_exact",
    "epoch_and_sentinel_contract",
    "canonical_public_schema_fixtures",
    "reason_code_registry",
    "contract_domain_tests",
    "dependency_source_and_license_policy",
];
/// Ordered P00 checkpoint sequence.
pub const EXPECTED_CHECKPOINTS: [&str; 4] = ["P00-A", "P00-B", "P00-C", "P00-D"];
/// Control-record roots that must contain no issued records.
pub const PROTECTED_ROOTS: [&str; 8] = [
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
    "swarm/supersessions",
    "swarm/wave-receipts",
];
/// Automatic workflow triggers forbidden by the manual-only policy, in the
/// exact Python alternation order.
pub const FORBIDDEN_WORKFLOW_TRIGGERS: [&str; 32] = [
    "push",
    "pull_request",
    "pull_request_target",
    "merge_group",
    "schedule",
    "workflow_run",
    "repository_dispatch",
    "workflow_call",
    "release",
    "issues",
    "issue_comment",
    "discussion",
    "discussion_comment",
    "create",
    "delete",
    "branch_protection_rule",
    "check_run",
    "check_suite",
    "deployment",
    "deployment_status",
    "fork",
    "gollum",
    "label",
    "milestone",
    "page_build",
    "project",
    "project_card",
    "project_column",
    "public",
    "registry_package",
    "status",
    "watch",
];
/// Matrix tokens proving navigation closure.
pub const EXPECTED_MATRIX_TOKENS: [&str; 12] = [
    "package handoff published",
    "accepted G0 receipt",
    "accepted W0 receipt",
    "P00-A",
    "P00-B",
    "P00-C",
    "P00-D",
    "search-domain",
    "search-ports",
    "W1 unlock matrix",
    "UNAVAILABLE",
    "does not unlock W1",
];
/// Report `validator` identity.
pub const VALIDATOR_ID: &str = "p00_foundation_acceptance_v1";

/// Single structural check (`status` renders `PASS`/`FAIL`).
pub struct AcceptanceCheck {
    /// Stable check identifier (also prefixes the accumulated error).
    pub id: String,
    /// Whether the check passed.
    pub passed: bool,
    /// Human-readable expectation detail.
    pub detail: String,
}

/// Validator outcome; `passed == errors.is_empty()`.
pub struct AcceptanceReport {
    /// PASS (no errors) vs FAIL.
    pub passed: bool,
    /// Checks in evaluation order.
    pub checks: Vec<AcceptanceCheck>,
    /// Accumulated `"<id>: <detail>"` errors in check order.
    pub errors: Vec<String>,
}

/// Process exit code: 0 on PASS, 1 on FAIL.
#[must_use]
pub const fn exit_code(report: &AcceptanceReport) -> i32 {
    if report.passed { 0 } else { 1 }
}

// --- JSON rendering ---------------------------------------------------------

/// `ensure_ascii=False` string escaping: short escapes plus lowercase
/// `\u00XX` for remaining controls; everything else (including non-ASCII
/// and DEL) passes through raw, exactly like `CPython` `json.dumps`.
fn append_json_string(out: &mut String, text: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
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
            c if (c as u32) < 0x20 => {
                let byte = c as u8;
                out.push_str("\\u00");
                out.push(HEX[usize::from(byte >> 4)] as char);
                out.push(HEX[usize::from(byte & 0x0F)] as char);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn append_string_array(out: &mut String, items: &[String]) {
    out.push('[');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        append_json_string(out, item);
    }
    out.push(']');
}

/// Render the report exactly like `json.dumps(report, ensure_ascii=False,
/// sort_keys=True, separators=(",", ":"))`.
#[must_use]
pub fn render_report_json(report: &AcceptanceReport) -> String {
    let mut out = String::from("{\"checks\":[");
    for (index, check) in report.checks.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str("{\"detail\":");
        append_json_string(&mut out, &check.detail);
        out.push_str(",\"id\":");
        append_json_string(&mut out, &check.id);
        out.push_str(",\"status\":");
        append_json_string(&mut out, if check.passed { "PASS" } else { "FAIL" });
        out.push('}');
    }
    out.push_str("],\"errors\":");
    append_string_array(&mut out, &report.errors);
    out.push_str(",\"g0_acceptance_claimed\":false,\"non_authoritative\":true");
    out.push_str(",\"package_acceptance_claimed\":false,\"schema_version\":1,\"status\":");
    append_json_string(&mut out, if report.passed { "PASS" } else { "FAIL" });
    out.push_str(",\"validator\":");
    append_json_string(&mut out, VALIDATOR_ID);
    out.push_str(",\"w0_acceptance_claimed\":false,\"w1_authority_claimed\":false}");
    out
}

// --- TOML access ------------------------------------------------------------

struct Validation {
    checks: Vec<AcceptanceCheck>,
    errors: Vec<String>,
}

impl Validation {
    fn require(&mut self, condition: bool, id: &str, detail: &str) {
        self.checks.push(AcceptanceCheck {
            id: id.to_owned(),
            passed: condition,
            detail: detail.to_owned(),
        });
        if !condition {
            self.errors.push(format!("{id}: {detail}"));
        }
    }
}

/// Load `relative` below `root`; missing/undecodable files and non-table
/// roots record the Python `file:`/`toml:` checks and yield an empty table.
fn load_toml(root: &Path, relative: &str, validation: &mut Validation) -> toml::Value {
    let empty = || toml::Value::Table(toml::map::Map::new());
    let Ok(bytes) = std::fs::read(root.join(relative)) else {
        validation.require(
            false,
            &format!("file:{relative}"),
            "required file is missing",
        );
        return empty();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        validation.require(
            false,
            &format!("toml:{relative}"),
            "invalid TOML: file is not valid UTF-8",
        );
        return empty();
    };
    let value: toml::Value = match text.parse() {
        Ok(value) => value,
        Err(err) => {
            validation.require(
                false,
                &format!("toml:{relative}"),
                &format!("invalid TOML: {err}"),
            );
            return empty();
        }
    };
    validation.require(
        value.is_table(),
        &format!("toml:{relative}"),
        "TOML root is a table",
    );
    if value.is_table() { value } else { empty() }
}

fn as_str<'a>(value: &'a toml::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

fn as_int(value: &toml::Value, key: &str) -> Option<i64> {
    value.get(key)?.as_integer()
}

fn as_bool(value: &toml::Value, key: &str) -> Option<bool> {
    value.get(key)?.as_bool()
}

/// `strings`: owned strings iff the value is a list of only strings,
/// otherwise empty (mirrors the Python `()` fallback, including the
/// missing-field case that still satisfies empty expectations).
fn str_list(value: Option<&toml::Value>) -> Vec<String> {
    if let Some(toml::Value::Array(items)) = value
        && items.iter().all(toml::Value::is_str)
    {
        items
            .iter()
            .map(|item| item.as_str().expect("all strings").to_owned())
            .collect()
    } else {
        Vec::new()
    }
}

/// Array-of-tables row keyed by `key == expected`; `None` unless exactly one
/// row matches (mirrors the Python zero-or-many miss).
fn find_table<'a>(
    document: &'a toml::Value,
    array_key: &str,
    key: &str,
    expected: &str,
) -> Option<&'a toml::Value> {
    let rows = document.get(array_key)?.as_array()?;
    let mut matches = rows.iter().filter(|row| {
        row.as_table()
            .and_then(|table| table.get(key))
            .and_then(toml::Value::as_str)
            == Some(expected)
    });
    let found = matches.next()?;
    if matches.next().is_none() {
        Some(found)
    } else {
        None
    }
}

/// `machine_files`: sorted repo-relative POSIX paths below `relative`,
/// following symlinked files like Python `is_file()` and ignoring only
/// `README.md`/`.gitkeep` basenames.
fn machine_files(root: &Path, relative: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.join(relative)];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // `metadata` follows symlinks, like Python `is_file()`.
            if std::fs::metadata(&path).is_ok_and(|meta| meta.is_file()) {
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| matches!(name, "README.md" | ".gitkeep"))
                {
                    continue;
                }
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            } else if path.is_dir() {
                stack.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Read a UTF-8 text file with `CPython` universal-newline semantics;
/// missing/unreadable files yield empty text (callers only probe tokens).
fn read_text(root: &Path, relative: &str) -> String {
    std::fs::read(root.join(relative))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|text| text.replace("\r\n", "\n").replace('\r', "\n"))
        .unwrap_or_default()
}

fn leading_ws_len(line: &str) -> usize {
    line.char_indices()
        .take_while(|(_, c)| c.is_whitespace())
        .map(|(index, c)| index + c.len_utf8())
        .last()
        .unwrap_or(0)
}

/// Anchored `keyword:` line with `min_ws..=max_ws` leading whitespace and
/// trailing whitespace only (mirrors `^\s{N,M}keyword:\s*$`).
fn anchored_keyword(line: &str, min_ws: usize, max_ws: usize, keyword: &str) -> bool {
    let ws = leading_ws_len(line);
    if ws < min_ws || ws > max_ws {
        return false;
    }
    line[ws..]
        .strip_prefix(keyword)
        .and_then(|rest| rest.strip_prefix(':'))
        .is_some_and(|rest| rest.chars().all(char::is_whitespace))
}

// --- registry ---------------------------------------------------------------

fn validate_registry(root: &Path, validation: &mut Validation) -> toml::Value {
    let registry = load_toml(root, "swarm/p00-foundation-acceptance.toml", validation);
    check_registry_header(&registry, validation);
    check_registry_packages(&registry, validation);
    check_registry_checkpoints(&registry, validation);
    check_registry_evidence(&registry, validation);
    check_registry_acceptance_tables(&registry, validation);
    registry
}

fn check_registry_header(registry: &toml::Value, validation: &mut Validation) {
    validation.require(
        as_int(registry, "schema_version") == Some(1),
        "registry-schema",
        "schema_version is 1",
    );
    validation.require(
        as_str(registry, "registry_kind") == Some("p00_foundation_acceptance_v1"),
        "registry-kind",
        "registry kind is closed",
    );
    validation.require(
        as_str(registry, "status") == Some("DESIGNED_NOT_EXECUTED"),
        "registry-status",
        "registry remains designed, not executed",
    );
    validation.require(
        as_str(registry, "owner") == Some("integration-owner"),
        "registry-owner",
        "integration owner owns acceptance",
    );
    validation.require(
        (
            as_str(registry, "stage"),
            as_str(registry, "phase"),
            as_int(registry, "wave"),
            as_str(registry, "gate"),
        ) == (Some("W0"), Some("P00"), Some(0), Some("G0")),
        "registry-stage",
        "registry is bound to W0/P00/G0",
    );
    validation.require(
        as_str(registry, "completion_receipt") == Some("W0"),
        "registry-receipt",
        "W0 receipt",
    );
    validation.require(
        as_int(registry, "package_count") == Some(3),
        "registry-package-count",
        "three packages",
    );
    validation.require(
        as_int(registry, "checkpoint_count") == Some(4),
        "registry-checkpoint-count",
        "four checkpoints",
    );
    validation.require(
        as_int(registry, "g0_evidence_count") == Some(10),
        "registry-evidence-count",
        "ten G0 items",
    );

    let authority_ok = registry
        .get("authority")
        .and_then(toml::Value::as_table)
        .is_some_and(|table| {
            !table.is_empty() && table.values().all(|value| value.as_bool() == Some(false))
        });
    validation.require(
        authority_ok,
        "registry-no-authority",
        "all authority flags are false",
    );

    let package_names: Vec<Option<&str>> = registry
        .get("package")
        .and_then(toml::Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.as_table()
                        .and_then(|table| table.get("name"))
                        .and_then(toml::Value::as_str)
                })
                .collect()
        })
        .unwrap_or_default();
    validation.require(
        package_names.len() == EXPECTED_PACKAGES.len()
            && package_names
                .iter()
                .zip(EXPECTED_PACKAGES)
                .all(|(actual, expected)| *actual == Some(expected)),
        "registry-packages",
        "exact package order",
    );
}

fn check_registry_packages(registry: &toml::Value, validation: &mut Validation) {
    for (order, name) in EXPECTED_PACKAGES.iter().enumerate() {
        let row = find_table(registry, "package", "name", name);
        validation.require(
            row.is_some(),
            &format!("package:{name}"),
            "one package row exists",
        );
        let Some(row) = row else { continue };
        let expected_class = if *name == "search-contracts" {
            "AUTHORIZED"
        } else {
            "CONDITIONAL"
        };
        let expected_predecessors: &[&str] = if *name == "search-contracts" {
            &[]
        } else {
            &["search-contracts"]
        };
        let expected_parallel: &[&str] = if *name == "search-contracts" {
            &[]
        } else if *name == "search-domain" {
            &["search-ports"]
        } else {
            &["search-domain"]
        };
        validation.require(
            as_int(row, "order") == i64::try_from(order).ok(),
            &format!("package:{name}:order"),
            "topological order",
        );
        validation.require(
            as_str(row, "launch_class") == Some(expected_class),
            &format!("package:{name}:class"),
            expected_class,
        );
        validation.require(
            as_str(row, "ticket_draft")
                == Some(format!("swarm/ticket-drafts/p00/{name}.toml").as_str()),
            &format!("package:{name}:ticket"),
            "exact ticket draft",
        );
        validation.require(
            as_str(row, "context_draft")
                == Some(format!("swarm/context-drafts/p00/{name}.toml").as_str()),
            &format!("package:{name}:context"),
            "exact context draft",
        );
        validation.require(
            as_str(row, "write_scope") == Some(format!("crates/{name}/**").as_str()),
            &format!("package:{name}:scope"),
            "package-only write scope",
        );
        validation.require(
            str_list(row.get("required_handoff_packages")) == expected_predecessors,
            &format!("package:{name}:handoffs"),
            "exact predecessor handoffs",
        );
        validation.require(
            str_list(row.get("required_control_records")) == EXPECTED_CHAIN,
            &format!("package:{name}:chain"),
            "complete control-record ladder",
        );
        validation.require(
            str_list(row.get("may_run_parallel_with")) == expected_parallel,
            &format!("package:{name}:parallel"),
            "closed parallelism declaration",
        );
        validation.require(
            as_str(row, "acceptance_output") == Some("package_handoff_v1"),
            &format!("package:{name}:output"),
            "package handoff only",
        );
    }
}

fn check_registry_checkpoints(registry: &toml::Value, validation: &mut Validation) {
    let checkpoint_ids: Vec<Option<&str>> = registry
        .get("checkpoint")
        .and_then(toml::Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.as_table()
                        .and_then(|table| table.get("id"))
                        .and_then(toml::Value::as_str)
                })
                .collect()
        })
        .unwrap_or_default();
    validation.require(
        checkpoint_ids.len() == EXPECTED_CHECKPOINTS.len()
            && checkpoint_ids
                .iter()
                .zip(EXPECTED_CHECKPOINTS)
                .all(|(actual, expected)| *actual == Some(expected)),
        "checkpoint-ids",
        "exact checkpoints",
    );
    if let Some(rows) = registry.get("checkpoint").and_then(toml::Value::as_array) {
        validation.require(
            rows.iter()
                .map(|row| row.get("order").and_then(toml::Value::as_integer))
                .collect::<Vec<_>>()
                == [Some(0), Some(1), Some(2), Some(3)],
            "checkpoint-order",
            "checkpoint order is closed",
        );
        validation.require(
            rows.iter().all(|row| {
                row.get("requires")
                    .and_then(toml::Value::as_array)
                    .is_some_and(|items| !items.is_empty())
            }),
            "checkpoint-requires",
            "every checkpoint has prerequisites",
        );
        validation.require(
            rows.iter().all(|row| {
                row.get("produces")
                    .and_then(toml::Value::as_array)
                    .is_some_and(|items| !items.is_empty())
            }),
            "checkpoint-produces",
            "every checkpoint has outputs",
        );
    }
}

fn check_registry_evidence(registry: &toml::Value, validation: &mut Validation) {
    let evidence_ids: Vec<Option<&str>> = registry
        .get("evidence")
        .and_then(toml::Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.as_table()
                        .and_then(|table| table.get("id"))
                        .and_then(toml::Value::as_str)
                })
                .collect()
        })
        .unwrap_or_default();
    validation.require(
        evidence_ids.len() == EXPECTED_G0.len()
            && evidence_ids
                .iter()
                .zip(EXPECTED_G0)
                .all(|(actual, expected)| *actual == Some(expected)),
        "registry-g0-evidence",
        "exact ordered G0 evidence set",
    );
    if let Some(rows) = registry.get("evidence").and_then(toml::Value::as_array) {
        for row in rows {
            let evidence_id = row
                .get("id")
                .and_then(toml::Value::as_str)
                .unwrap_or("unknown");
            validation.require(
                as_str(row, "required_state") == Some("PASS"),
                &format!("evidence:{evidence_id}:required"),
                "PASS required",
            );
            validation.require(
                as_str(row, "current_state") == Some("UNAVAILABLE"),
                &format!("evidence:{evidence_id}:current"),
                "current state remains unavailable",
            );
            validation.require(
                as_bool(row, "raw_output_required") == Some(true),
                &format!("evidence:{evidence_id}:raw"),
                "raw output required",
            );
            validation.require(
                as_bool(row, "independent_review_required") == Some(true),
                &format!("evidence:{evidence_id}:review"),
                "independent review required",
            );
        }
    }
}

fn check_registry_acceptance_tables(registry: &toml::Value, validation: &mut Validation) {
    check_w0_table(registry, validation);
    check_w1_table(registry, validation);
    check_current_state_table(registry, validation);
}

fn check_w0_table(registry: &toml::Value, validation: &mut Validation) {
    if let Some(w0) = registry
        .get("w0_acceptance")
        .filter(|value| value.is_table())
    {
        validation.require(true, "w0-table", "W0 acceptance table exists");
        validation.require(
            str_list(w0.get("required_packages")) == EXPECTED_PACKAGES,
            "w0-packages",
            "three exact packages",
        );
        validation.require(
            as_str(w0, "required_gate") == Some("G0"),
            "w0-gate",
            "G0 required",
        );
        validation.require(
            as_str(w0, "required_wave_receipt") == Some("W0"),
            "w0-receipt",
            "W0 required",
        );
        for key in [
            "raw_output_required",
            "independent_review_required",
            "requires_no_active_writer_leases",
            "requires_zero_unreviewed_submissions",
            "requires_append_only_package_handoffs",
            "package_handoff_does_not_accept_gate_or_wave",
            "receipt_and_launch_update_same_reviewed_change",
        ] {
            validation.require(
                as_bool(w0, key) == Some(true),
                &format!("w0:{key}"),
                &format!("{key} is true"),
            );
        }
    } else {
        validation.require(false, "w0-table", "W0 acceptance table exists");
    }
}

fn check_w1_table(registry: &toml::Value, validation: &mut Validation) {
    if let Some(w1) = registry.get("w1_unlock").filter(|value| value.is_table()) {
        validation.require(true, "w1-table", "W1 unlock table exists");
        validation.require(as_str(w1, "stage") == Some("W1"), "w1-stage", "W1");
        validation.require(
            as_str(w1, "current_state") == Some("BLOCKED"),
            "w1-current",
            "W1 remains blocked",
        );
        validation.require(
            as_str(w1, "requires_accepted_gate") == Some("G0"),
            "w1-gate",
            "G0 prerequisite",
        );
        validation.require(
            as_str(w1, "requires_accepted_receipt") == Some("W0"),
            "w1-receipt",
            "W0 prerequisite",
        );
        validation.require(
            as_int(w1, "requires_launch_state_active_wave") == Some(1),
            "w1-wave",
            "wave 1 after advance",
        );
        for key in [
            "configuration_or_stage_presence_authorizes",
            "package_presence_authorizes",
            "manual_workflow_authorizes",
        ] {
            validation.require(
                as_bool(w1, key) == Some(false),
                &format!("w1:{key}"),
                &format!("{key} is false"),
            );
        }
    } else {
        validation.require(false, "w1-table", "W1 unlock table exists");
    }
}

fn check_current_state_table(registry: &toml::Value, validation: &mut Validation) {
    if let Some(current) = registry
        .get("current_repository_state")
        .filter(|value| value.is_table())
    {
        validation.require(true, "current-state", "current-state table exists");
        for key in [
            "materialized_contexts",
            "issued_tickets",
            "active_writer_leases",
            "submissions",
            "accepted_reviews",
            "accepted_package_handoffs",
        ] {
            validation.require(
                as_int(current, key) == Some(0),
                &format!("current:{key}"),
                &format!("{key} remains zero"),
            );
        }
        validation.require(
            as_bool(current, "accepted_g0_receipt") == Some(false),
            "current:g0",
            "G0 absent",
        );
        validation.require(
            as_bool(current, "accepted_w0_receipt") == Some(false),
            "current:w0",
            "W0 absent",
        );
        validation.require(
            (
                as_str(current, "active_stage"),
                as_int(current, "active_wave"),
            ) == (Some("P00"), Some(0)),
            "current:launch",
            "P00/W0 remains active",
        );
    } else {
        validation.require(false, "current-state", "current-state table exists");
    }
}

// --- cross registries ---------------------------------------------------------

fn validate_cross_registries(root: &Path, validation: &mut Validation) {
    let gates = load_toml(root, "swarm/gates.toml", validation);
    let stages = load_toml(root, "swarm/stages.toml", validation);
    let launch = load_toml(root, "swarm/launch-state.toml", validation);
    let crates = load_toml(root, "swarm/crates.toml", validation);
    let functions = load_toml(root, "swarm/function-packets.toml", validation);
    let ticket_manifest = load_toml(root, "swarm/ticket-drafts/manifest.toml", validation);
    let context_manifest = load_toml(root, "swarm/context-drafts/manifest.toml", validation);
    let orchestration = load_toml(root, "swarm/orchestration.toml", validation);

    check_gate_stage(&gates, &stages, validation);
    check_launch_state(&launch, validation);
    check_draft_manifests(
        &ticket_manifest,
        &context_manifest,
        &orchestration,
        validation,
    );
    check_cross_packages(root, &crates, &functions, validation);
}

fn check_gate_stage(gates: &toml::Value, stages: &toml::Value, validation: &mut Validation) {
    let g0 = find_table(gates, "gate", "id", "G0");
    validation.require(g0.is_some(), "gate-g0", "one G0 row exists");
    if let Some(g0) = g0 {
        validation.require(
            str_list(g0.get("required_evidence")) == EXPECTED_G0,
            "gate-g0-set",
            "G0 set matches registry",
        );
        validation.require(
            (as_str(g0, "stage"), as_int(g0, "wave")) == (Some("P00"), Some(0)),
            "gate-g0-stage",
            "G0 is P00/W0",
        );
    }

    let w0 = find_table(stages, "stage", "id", "W0");
    let w1 = find_table(stages, "stage", "id", "W1");
    validation.require(
        w0.is_some() && w1.is_some(),
        "stage-w0-w1",
        "W0 and W1 rows exist",
    );
    if let Some(w0) = w0 {
        validation.require(
            str_list(w0.get("packages")) == EXPECTED_PACKAGES,
            "stage-w0-packages",
            "exact W0 packages",
        );
        validation.require(
            as_str(w0, "contributes_to_gate") == Some("G0"),
            "stage-w0-gate",
            "W0 closes G0",
        );
        validation.require(
            as_str(w0, "completion_receipt") == Some("W0"),
            "stage-w0-receipt",
            "W0 receipt",
        );
        validation.require(
            as_str(w0, "status") == Some("ACTIVE_PACKAGE_ONLY"),
            "stage-w0-status",
            "package-only active",
        );
    }
    if let Some(w1) = w1 {
        validation.require(
            as_str(w1, "status") == Some("BLOCKED"),
            "stage-w1-status",
            "W1 blocked",
        );
        validation.require(
            str_list(w1.get("requires_accepted_gates")) == ["G0"],
            "stage-w1-gate",
            "G0 prerequisite",
        );
        validation.require(
            str_list(w1.get("requires_accepted_receipts")) == ["W0"],
            "stage-w1-receipt",
            "W0 prerequisite",
        );
        validation.require(
            str_list(w1.get("packages")) == EXPECTED_W1_PACKAGES,
            "stage-w1-packages",
            "exact W1 packages",
        );
    }
}

fn check_launch_state(launch: &toml::Value, validation: &mut Validation) {
    validation.require(
        (
            as_str(launch, "active_stage"),
            as_int(launch, "active_wave"),
        ) == (Some("P00"), Some(0)),
        "launch-current",
        "launch remains P00/W0",
    );
    validation.require(
        str_list(launch.get("authorized_packages")) == ["search-contracts"],
        "launch-authorized",
        "contracts only",
    );
    validation.require(
        str_list(launch.get("conditional_packages")) == ["search-domain", "search-ports"],
        "launch-conditional",
        "domain and ports only",
    );
    if let Some(control) = launch.get("draft_control").filter(|value| value.is_table()) {
        validation.require(true, "launch-draft-control", "draft-control table exists");
        for (key, expected) in [
            ("ticket_drafts", 3),
            ("context_drafts", 3),
            ("materialized_contexts", 0),
            ("issued_tickets", 0),
            ("active_leases", 0),
            ("submissions", 0),
            ("accepted_reviews", 0),
            ("accepted_package_handoffs", 0),
        ] {
            validation.require(
                as_int(control, key) == Some(expected),
                &format!("launch:{key}"),
                &format!("{key} = {expected}"),
            );
        }
        validation.require(
            as_bool(control, "draft_presence_authorizes") == Some(false),
            "launch:draft-authority",
            "drafts do not authorize",
        );
        validation.require(
            as_bool(control, "draft_presence_satisfies_conditional_activation") == Some(false),
            "launch:draft-conditional",
            "drafts do not satisfy conditional activation",
        );
    } else {
        validation.require(false, "launch-draft-control", "draft-control table exists");
    }

    if let Some(advancement) = launch.get("advancement").filter(|value| value.is_table()) {
        validation.require(true, "launch-advancement", "advancement table exists");
        validation.require(
            as_int(advancement, "next_wave") == Some(1),
            "launch-next-wave",
            "next wave is 1",
        );
        validation.require(
            as_str(advancement, "requires_accepted_gate") == Some("G0"),
            "launch-advance-gate",
            "G0 required",
        );
        validation.require(
            as_str(advancement, "requires_accepted_wave_receipt") == Some("W0"),
            "launch-advance-receipt",
            "W0 required",
        );
        validation.require(
            str_list(advancement.get("requires_accepted_packages")) == EXPECTED_PACKAGES,
            "launch-advance-packages",
            "three handoffs",
        );
        validation.require(
            as_bool(advancement, "requires_no_active_writer_leases") == Some(true),
            "launch-no-leases",
            "no active leases",
        );
        validation.require(
            as_bool(advancement, "requires_zero_unreviewed_submissions") == Some(true),
            "launch-no-submissions",
            "no unreviewed submissions",
        );
        validation.require(
            as_bool(advancement, "requires_append_only_package_handoffs") == Some(true),
            "launch-append-only",
            "append-only handoffs",
        );
    } else {
        validation.require(false, "launch-advancement", "advancement table exists");
    }
}

fn check_draft_manifests(
    ticket_manifest: &toml::Value,
    context_manifest: &toml::Value,
    orchestration: &toml::Value,
    validation: &mut Validation,
) {
    validation.require(
        as_int(ticket_manifest, "draft_count") == Some(3),
        "ticket-manifest-count",
        "three ticket drafts",
    );
    validation.require(
        as_int(context_manifest, "draft_count") == Some(3),
        "context-manifest-count",
        "three context drafts",
    );
    validation.require(
        as_int(orchestration, "schema_version") == Some(5),
        "orchestration-version",
        "orchestration schema v5",
    );
    validation.require(
        as_bool(orchestration, "consumer_uses_branch_head") == Some(false),
        "orchestration-no-head",
        "consumers bind immutable commits",
    );
    validation.require(
        as_bool(
            orchestration,
            "consumer_requires_exact_commit_and_api_digest",
        ) == Some(true),
        "orchestration-exact-handoff",
        "exact commit/API required",
    );
}

fn check_cross_packages(
    root: &Path,
    crates: &toml::Value,
    functions: &toml::Value,
    validation: &mut Validation,
) {
    for name in EXPECTED_PACKAGES {
        let package_row = find_table(crates, "package", "name", name);
        let function_row = find_table(functions, "foundation", "package", name);
        validation.require(
            package_row.is_some(),
            &format!("crates:{name}"),
            "package registry row exists",
        );
        validation.require(
            function_row.is_some(),
            &format!("functions:{name}"),
            "foundation function row exists",
        );
        if let (Some(package_row), Some(function_row)) = (package_row, function_row) {
            validation.require(
                as_int(package_row, "wave") == Some(0),
                &format!("crates:{name}:wave"),
                "wave 0",
            );
            validation.require(
                as_int(function_row, "wave") == Some(0),
                &format!("functions:{name}:wave"),
                "wave 0",
            );
            validation.require(
                as_str(function_row, "write_scope") == Some(format!("crates/{name}/**").as_str()),
                &format!("functions:{name}:scope"),
                "package-only scope",
            );
        }

        let ticket = load_toml(
            root,
            &format!("swarm/ticket-drafts/p00/{name}.toml"),
            validation,
        );
        let context = load_toml(
            root,
            &format!("swarm/context-drafts/p00/{name}.toml"),
            validation,
        );
        let expected_class = if name == "search-contracts" {
            "AUTHORIZED"
        } else {
            "CONDITIONAL"
        };
        validation.require(
            as_int(&ticket, "schema_version") == Some(2),
            &format!("ticket:{name}:schema"),
            "ticket schema v2",
        );
        validation.require(
            as_str(&ticket, "status") == Some("DRAFT_ONLY_NOT_ISSUED"),
            &format!("ticket:{name}:status"),
            "non-issued",
        );
        validation.require(
            as_bool(&ticket, "claimable") == Some(false),
            &format!("ticket:{name}:claimable"),
            "not claimable",
        );
        validation.require(
            as_bool(&ticket, "authorizes_implementation") == Some(false),
            &format!("ticket:{name}:authority"),
            "no implementation authority",
        );
        validation.require(
            as_bool(&ticket, "creates_lease") == Some(false),
            &format!("ticket:{name}:lease"),
            "does not create lease",
        );
        validation.require(
            as_str(&ticket, "launch_class") == Some(expected_class),
            &format!("ticket:{name}:class"),
            expected_class,
        );
        validation.require(
            as_int(&context, "schema_version") == Some(2),
            &format!("context:{name}:schema"),
            "context schema v2",
        );
        validation.require(
            as_str(&context, "status") == Some("UNMATERIALIZED_DRAFT"),
            &format!("context:{name}:status"),
            "unmaterialized",
        );
        validation.require(
            as_bool(&context, "claimable") == Some(false),
            &format!("context:{name}:claimable"),
            "not claimable",
        );
        let expected_slots: &[&str] = if name == "search-contracts" {
            &[]
        } else {
            &["search-contracts::accepted_package_and_api_handoff"]
        };
        if let Some(content) = context.get("content").filter(|value| value.is_table()) {
            validation.require(
                true,
                &format!("context:{name}:content"),
                "content table exists",
            );
            validation.require(
                str_list(content.get("accepted_handoff_slots")) == expected_slots,
                &format!("context:{name}:slots"),
                "exact handoff slots",
            );
        } else {
            validation.require(
                false,
                &format!("context:{name}:content"),
                "content table exists",
            );
        }
    }
}

// --- zero state, docs and workflows -------------------------------------------

fn validate_zero_state(root: &Path, validation: &mut Validation) {
    for relative in PROTECTED_ROOTS {
        let is_dir = root.join(relative).is_dir();
        validation.require(
            is_dir,
            &format!("zero:{relative}:dir"),
            "protected root exists",
        );
        if !is_dir {
            continue;
        }
        let unexpected = machine_files(root, relative);
        validation.require(
            unexpected.is_empty(),
            &format!("zero:{relative}"),
            if unexpected.is_empty() {
                "no issued record".to_owned()
            } else {
                format!("unexpected: {}", unexpected[0])
            }
            .as_str(),
        );
    }
}

fn validate_docs_and_workflows(root: &Path, validation: &mut Validation) {
    let matrix = read_text(root, "docs/handoff/P00_FOUNDATION_ACCEPTANCE_MATRIX.md");
    let handoff_readme = read_text(root, "docs/handoff/README.md");
    let tools_readme = read_text(root, "tools/README.md");
    for token in EXPECTED_MATRIX_TOKENS {
        validation.require(
            matrix.contains(token),
            &format!("matrix-token:{token}"),
            &format!("matrix contains {token}"),
        );
    }
    validation.require(
        handoff_readme.contains("P00_FOUNDATION_ACCEPTANCE_MATRIX.md"),
        "handoff-navigation",
        "handoff README links matrix",
    );
    validation.require(
        tools_readme.contains("validate-p00-foundation-acceptance"),
        "tool-navigation",
        "tools README links validator",
    );

    let mut files: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root.join(".github/workflows")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !std::fs::metadata(&path).is_ok_and(|meta| meta.is_file()) {
                continue;
            }
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| matches!(ext, "yml" | "yaml"))
                && let Some(name) = path.file_name().and_then(|name| name.to_str())
            {
                files.push(format!(".github/workflows/{name}"));
            }
        }
    }
    files.sort();
    validation.require(
        !files.is_empty(),
        "workflow-presence",
        "workflow files exist",
    );
    for relative in &files {
        let text = read_text(root, relative);
        let manual = text
            .lines()
            .any(|line| anchored_keyword(line, 2, 2, "workflow_dispatch"));
        let no_auto = !text.lines().any(|line| {
            FORBIDDEN_WORKFLOW_TRIGGERS
                .iter()
                .any(|trigger| anchored_keyword(line, 0, 6, trigger))
        });
        let read_only = text.lines().any(contents_read);
        let no_credentials = text.contains("persist-credentials: false");
        validation.require(
            manual && no_auto && read_only && no_credentials,
            &format!("workflow:{relative}"),
            "manual/read-only/credential-free",
        );
    }
}

/// `contents:` line with exactly two leading whitespace chars and a trailing
/// `read` value (mirrors `^\s{2}contents:\s*read\s*$`).
fn contents_read(line: &str) -> bool {
    let ws = leading_ws_len(line);
    if ws != 2 {
        return false;
    }
    line[ws..]
        .strip_prefix("contents")
        .and_then(|rest| rest.strip_prefix(':'))
        .is_some_and(|rest| {
            let value = rest.trim_start_matches(|c: char| c.is_whitespace());
            value
                .strip_prefix("read")
                .is_some_and(|tail| tail.chars().all(char::is_whitespace))
        })
}

// --- entrypoint -----------------------------------------------------------------

/// Read-only P00 foundation acceptance validation against `root`.
#[must_use]
pub fn validate_p00_foundation_acceptance(root: &Path) -> AcceptanceReport {
    let mut validation = Validation {
        checks: Vec::new(),
        errors: Vec::new(),
    };
    validate_registry(root, &mut validation);
    validate_cross_registries(root, &mut validation);
    validate_zero_state(root, &mut validation);
    validate_docs_and_workflows(root, &mut validation);
    AcceptanceReport {
        passed: validation.errors.is_empty(),
        checks: validation.checks,
        errors: validation.errors,
    }
}
