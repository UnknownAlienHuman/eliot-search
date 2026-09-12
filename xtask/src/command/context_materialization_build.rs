use std::path::PathBuf;
use std::process::ExitCode;

use xtask::context_materialization::DECISION_COMMIT;
use xtask::context_materialization_builder::{build_plan, write_plan};

use super::usage_error;

pub(super) fn build(args: &[String]) -> ExitCode {
    let mut root = PathBuf::from(".");
    let mut candidate: Option<String> = None;
    let mut bundle: Option<String> = None;
    let mut selection: Option<String> = None;
    let mut output_root = "artifacts/context-materialization-plans".to_owned();
    let mut write = false;
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
            "--candidate" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                candidate = Some(value.clone());
            }
            "--bundle" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                bundle = Some(value.clone());
            }
            "--selection" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                selection = Some(value.clone());
            }
            "--output-root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return usage_error();
                };
                output_root.clone_from(value);
            }
            "--write" => write = true,
            "--require-ready" => require_ready = true,
            _ => return usage_error(),
        }
        index += 1;
    }
    let Some(candidate) = candidate else {
        return usage_error();
    };
    let build = match build_plan(
        &root,
        &candidate,
        bundle.as_deref(),
        selection.as_deref(),
        &output_root,
    ) {
        Ok(build) => build,
        Err(error) => {
            eprintln!("{}: {}", error.reason(), error.message());
            return ExitCode::from(2);
        }
    };
    if write {
        if let Err(error) = write_plan(&root, &build) {
            eprintln!("{}: {}", error.reason(), error.message());
            return ExitCode::from(2);
        }
    }
    print!(
        "{}",
        std::str::from_utf8(build.plan_bytes())
            .expect("canonical plan JSON is UTF-8")
    );
    if require_ready
        && build
            .plan()
            .get("decision")
            .and_then(serde_json::Value::as_str)
            != Some(DECISION_COMMIT)
    {
        return ExitCode::from(3);
    }
    ExitCode::SUCCESS
}
