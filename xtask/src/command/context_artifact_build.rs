use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;
use xtask::context_artifact::ARTIFACT_ROOT;
use xtask::context_artifact_builder::{build_candidate, write_candidate};

use super::usage_error;

pub(super) fn build(args: &[String]) -> ExitCode {
    let mut root = PathBuf::from(".");
    let mut package: Option<String> = None;
    let mut base_commit: Option<String> = None;
    let mut accepted_handoffs = Vec::new();
    let mut output_root = ARTIFACT_ROOT.to_owned();
    let mut print_result = false;
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
            "--accepted-handoff" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                accepted_handoffs.push(value.clone());
            }
            "--output-root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                output_root.clone_from(value);
            }
            "--print-result" => print_result = true,
            _ => return usage_error(),
        }
        index += 1;
    }
    let (Some(package), Some(base_commit)) = (package, base_commit) else {
        return usage_error();
    };
    let build = match build_candidate(
        &root,
        &package,
        &base_commit,
        &accepted_handoffs,
        &output_root,
    ) {
        Ok(build) => build,
        Err(error) => {
            eprintln!("{}: {}", error.reason(), error.message());
            return ExitCode::from(2);
        }
    };
    if let Err(error) = write_candidate(&root, &build) {
        eprintln!("{}: {}", error.reason(), error.message());
        return ExitCode::from(2);
    }
    let candidate = build.candidate();
    let result = json!({
        "candidate_id": candidate["candidate_id"],
        "bundle": build.bundle_relative_path(),
        "candidate": build.candidate_relative_path(),
        "artifact_sha256": candidate["artifact_candidate"]["sha256"],
        "candidate_sha256": candidate["candidate_sha256"],
        "status": candidate["status"],
    });
    if print_result {
        println!(
            "{}",
            serde_json::to_string(&result)
                .expect("serializing a bounded candidate result cannot fail")
        );
    } else {
        println!(
            "READY: {} -> {} and {}",
            candidate["candidate_id"].as_str().unwrap_or("UNAVAILABLE"),
            build.bundle_relative_path(),
            build.candidate_relative_path(),
        );
    }
    ExitCode::SUCCESS
}
