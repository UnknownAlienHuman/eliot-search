use std::path::PathBuf;
use std::process::ExitCode;

use xtask::control_record_enum_bindings::validate_control_record_enum_bindings;

use super::usage_error;

pub(super) fn validate(args: &[String]) -> ExitCode {
    let mut root = PathBuf::from(".");
    let mut root_seen = false;
    let mut json = false;
    let mut json_seen = false;
    let mut index = 0_usize;

    while index < args.len() {
        match args[index].as_str() {
            "--root" if !root_seen => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return usage_error();
                };
                root = PathBuf::from(path);
                root_seen = true;
            }
            "--json" if !json_seen => {
                json = true;
                json_seen = true;
            }
            _ => return usage_error(),
        }
        index += 1;
    }

    match validate_control_record_enum_bindings(&root) {
        Ok(report) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "validation": "PASS",
                        "authority": report.authority,
                        "scope": report.scope,
                        "max_total_input_bytes": report.max_total_input_bytes,
                        "declared_type_count": report.declared_type_count,
                        "enum_count": report.enum_count,
                        "binding_count": report.binding_count,
                    })
                );
            } else {
                println!(
                    "PASS NON_AUTHORITATIVE: {} validated; types={}, enums={}, bindings={}, max input {} bytes",
                    report.scope,
                    report.declared_type_count,
                    report.enum_count,
                    report.binding_count,
                    report.max_total_input_bytes
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "validation": "FAIL",
                        "authority": "NON_AUTHORITATIVE",
                        "scope": xtask::control_record_enum_bindings::VALIDATION_SCOPE,
                        "max_total_input_bytes": xtask::control_record_enum_bindings::MAX_TOTAL_INPUT_BYTES,
                        "error_code": error.code(),
                    })
                );
            } else {
                println!(
                    "FAIL NON_AUTHORITATIVE: {} (scope={}, max input {} bytes)",
                    error.code(),
                    xtask::control_record_enum_bindings::VALIDATION_SCOPE,
                    xtask::control_record_enum_bindings::MAX_TOTAL_INPUT_BYTES
                );
            }
            ExitCode::FAILURE
        }
    }
}