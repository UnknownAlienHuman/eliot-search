use std::fmt::Write as _;

use super::model::ProgramReport;

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
