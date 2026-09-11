//! Port of `tools/validate-accepted-evidence-digest.py` (T41, family E).
//!
//! Same causal invariants and exit codes. Migration delta: the `REQUIRED`
//! tool paths now name the kept Python library plus the Rust sources and
//! parity tests instead of the retired Python entrypoints (see T41 report).

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

/// Files that must exist for the digest profile closure.
pub const REQUIRED: [&str; 8] = [
    "swarm/accepted-evidence-digest-v1.toml",
    "swarm/type-rule-profiles-v1.toml",
    "docs/handoff/ACCEPTED_EVIDENCE_DIGEST_V1.md",
    "tools/accepted_evidence_digest_v1.py",
    "xtask/src/accepted_evidence.rs",
    "xtask/src/main.rs",
    "qualification/accepted-evidence/cases-v1.toml",
    "xtask/tests/accepted_evidence_parity.rs",
];

const WORKFLOW: &str = ".github/workflows/accepted-evidence-digest.yml";

/// Validation outcome mirroring the Python result dict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    /// `"PASS"` or `"FAIL"`.
    pub status: &'static str,
    /// Number of required files checked.
    pub required_files: usize,
    /// Number of type-rule bindings found.
    pub bindings: usize,
    /// Number of corpus cases found.
    pub cases: usize,
    /// Causal failure details.
    pub errors: Vec<String>,
}

/// Exit code mirroring the Python validator (`0` pass, `1` fail).
#[must_use]
pub const fn exit_code(report: &ValidationReport) -> i32 {
    (!report.errors.is_empty()) as i32
}

fn append_json_string_ascii(out: &mut String, value: &str) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{09}' => out.push_str("\\t"),
            '\u{0A}' => out.push_str("\\n"),
            '\u{0C}' => out.push_str("\\f"),
            '\u{0D}' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u00{:02x}", c as u8);
            }
            c if (c as u32) < 0x7F => out.push(c),
            c => {
                let code = c as u32;
                if code < 0x1_0000 {
                    let _ = write!(out, "\\u{code:04x}");
                } else {
                    let v = code - 0x1_0000;
                    let high = 0xD800 + (v >> 10);
                    let low = 0xDC00 + (v & 0x3FF);
                    let _ = write!(out, "\\u{high:04x}\\u{low:04x}");
                }
            }
        }
    }
    out.push('"');
}

/// Render the report exactly like
/// `json.dumps(result, indent=2, sort_keys=True)`.
#[must_use]
pub fn render_report_json(report: &ValidationReport) -> String {
    let mut out = String::from("{\n");
    let _ = writeln!(out, "  \"bindings\": {},", report.bindings);
    let _ = writeln!(out, "  \"cases\": {},", report.cases);
    if report.errors.is_empty() {
        out.push_str("  \"errors\": [],\n");
    } else {
        out.push_str("  \"errors\": [\n");
        for (index, error) in report.errors.iter().enumerate() {
            out.push_str("    ");
            append_json_string_ascii(&mut out, error);
            if index + 1 < report.errors.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  ],\n");
    }
    let _ = writeln!(out, "  \"required_files\": {},", report.required_files);
    let _ = writeln!(out, "  \"status\": \"{}\"", report.status);
    out.push('}');
    out
}

fn parse_toml_file(root: &Path, relative: &str, errors: &mut Vec<String>) -> Option<toml::Value> {
    let path = root.join(relative);
    match fs::read_to_string(&path) {
        Ok(text) => match text.parse::<toml::Value>() {
            Ok(value) => Some(value),
            Err(err) => {
                errors.push(format!("TOML parse: {err}"));
                None
            }
        },
        Err(err) => {
            errors.push(format!("TOML parse: {err}"));
            None
        }
    }
}

fn check_profile_identity(profile: Option<&toml::Value>, errors: &mut Vec<String>) {
    if profile
        .and_then(|v| v.get("profile"))
        .and_then(toml::Value::as_str)
        != Some("accepted_evidence_digest_v1")
    {
        errors.push("profile identity mismatch".to_owned());
    }
    if profile
        .and_then(|v| v.get("self_referential_digest_allowed"))
        .and_then(toml::Value::as_bool)
        != Some(false)
    {
        errors.push("self-referential digest must remain false".to_owned());
    }
}

fn check_bindings(mapping: Option<&toml::Value>, errors: &mut Vec<String>) -> usize {
    let Some(rows) = mapping
        .and_then(|v| v.get("binding"))
        .and_then(toml::Value::as_array)
    else {
        errors.push("type-rule profile binding missing or duplicate".to_owned());
        return 0;
    };
    let exact = rows
        .iter()
        .filter(|row| {
            row.get("type").and_then(toml::Value::as_str) == Some("OrderedAcceptedPackageHandoff")
                && row.get("field").and_then(toml::Value::as_str) == Some("evidence_digest")
        })
        .count();
    let profile_ok = rows.iter().any(|row| {
        row.get("type").and_then(toml::Value::as_str) == Some("OrderedAcceptedPackageHandoff")
            && row.get("field").and_then(toml::Value::as_str) == Some("evidence_digest")
            && row.get("profile").and_then(toml::Value::as_str)
                == Some("accepted_evidence_digest_v1")
    });
    if exact != 1 || !profile_ok {
        errors.push("type-rule profile binding missing or duplicate".to_owned());
    }
    rows.len()
}

fn check_cases(cases: Option<&toml::Value>, errors: &mut Vec<String>) -> usize {
    let Some(case_rows) = cases
        .and_then(|v| v.get("case"))
        .and_then(toml::Value::as_array)
    else {
        errors.push("accepted evidence case inventory must contain exactly ten cases".to_owned());
        return 0;
    };
    let declared = cases
        .and_then(|v| v.get("case_count"))
        .and_then(toml::Value::as_integer);
    if declared != Some(10) || case_rows.len() != 10 {
        errors.push("accepted evidence case inventory must contain exactly ten cases".to_owned());
    }
    let mut ids: Vec<Option<&str>> = case_rows
        .iter()
        .map(|row| row.get("id").and_then(toml::Value::as_str))
        .collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() != total {
        errors.push("accepted evidence case IDs must be unique".to_owned());
    }
    case_rows.len()
}

fn check_workflow(root: &Path, errors: &mut Vec<String>) {
    let workflow_path = root.join(WORKFLOW);
    if !workflow_path.is_file() {
        return;
    }
    match fs::read_to_string(&workflow_path) {
        Ok(text) => {
            for token in [
                "workflow_dispatch:",
                "contents: read",
                "persist-credentials: false",
            ] {
                if !text.contains(token) {
                    errors.push(format!("workflow missing {token}"));
                }
            }
            for forbidden in ["\n  push:", "\n  pull_request:", "\n  schedule:"] {
                if text.contains(forbidden) {
                    errors.push(format!("automatic workflow trigger: {}", forbidden.trim()));
                }
            }
        }
        Err(err) => errors.push(format!("TOML parse: {err}")),
    }
}

/// Run the registry and workflow closure validation against `root`.
#[must_use]
pub fn validate_accepted_evidence_digest(root: &Path) -> ValidationReport {
    let mut errors: Vec<String> = Vec::new();
    for relative in REQUIRED {
        if !root.join(relative).is_file() {
            errors.push(format!("missing: {relative}"));
        }
    }
    let profile = parse_toml_file(root, REQUIRED[0], &mut errors);
    let mapping = parse_toml_file(root, REQUIRED[1], &mut errors);
    let cases = parse_toml_file(root, REQUIRED[6], &mut errors);

    check_profile_identity(profile.as_ref(), &mut errors);
    let bindings = check_bindings(mapping.as_ref(), &mut errors);
    let case_count = check_cases(cases.as_ref(), &mut errors);
    check_workflow(root, &mut errors);

    let status = if errors.is_empty() { "PASS" } else { "FAIL" };
    ValidationReport {
        status,
        required_files: REQUIRED.len(),
        bindings,
        cases: case_count,
        errors,
    }
}
