use std::path::PathBuf;
use std::process::ExitCode;

use xtask::ticket_issuance_builder::{
    TicketIssuanceBuildOptions, build_plan, write_plan,
};
use xtask::ticket_issuance_validation::{
    exit_code, render_report_json, render_report_text,
    validate_ticket_issuance_plan,
};
use xtask::ticket_planner::{DECISION_INVALID, DECISION_READY};

use super::usage_error;

pub(super) fn validate(args: &[String]) -> ExitCode {
    let mut root = PathBuf::from(".");
    let mut json = false;
    let mut index = 0_usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return usage_error();
                };
                root = PathBuf::from(path);
            }
            "--json" => json = true,
            _ => return usage_error(),
        }
        index += 1;
    }
    let root = std::fs::canonicalize(&root).unwrap_or(root);
    let report = validate_ticket_issuance_plan(&root);
    if json {
        println!("{}", render_report_json(&report));
    } else {
        println!("{}", render_report_text(&report));
    }
    ExitCode::from(u8::try_from(exit_code(&report)).unwrap_or(1))
}

pub(super) fn build(args: &[String]) -> ExitCode {
    let mut root = PathBuf::from(".");
    let mut package: Option<String> = None;
    let mut base_commit: Option<String> = None;
    let mut writer: Option<String> = None;
    let mut reviewer: Option<String> = None;
    let mut accepted_handoffs = Vec::new();
    let mut output = "-".to_owned();
    let mut require_ready = false;
    let mut index = 0_usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                root = PathBuf::from(value);
            }
            "--package" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                package = Some(value.clone());
            }
            "--base-commit" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                base_commit = Some(value.clone());
            }
            "--writer" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                writer = Some(value.clone());
            }
            "--reviewer" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                reviewer = Some(value.clone());
            }
            "--accepted-handoff" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                accepted_handoffs.push(value.clone());
            }
            "--output" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                output.clone_from(value);
            }
            "--require-ready" => require_ready = true,
            _ => return usage_error(),
        }
        index += 1;
    }
    let Some(package) = package else {
        return usage_error();
    };
    let options = TicketIssuanceBuildOptions {
        package,
        base_commit,
        writer,
        reviewer,
        accepted_handoffs,
        output,
    };
    let build = match build_plan(&root, &options) {
        Ok(build) => build,
        Err(error) => {
            eprintln!("{}: {}", error.reason(), error.message());
            return ExitCode::from(2);
        }
    };
    let writes_file = build.output_target().is_some();
    if let Err(error) = write_plan(&build) {
        eprintln!("{}: {}", error.reason(), error.message());
        return ExitCode::from(2);
    }
    if !writes_file {
        print!(
            "{}",
            std::str::from_utf8(build.plan_bytes())
                .expect("canonical plan JSON is UTF-8")
        );
    }
    let decision = build
        .plan()
        .get("decision")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(DECISION_INVALID);
    if require_ready && decision != DECISION_READY {
        return ExitCode::from(3);
    }
    if decision == DECISION_INVALID {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}
