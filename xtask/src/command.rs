mod bootstrap;
mod coverage_graph;
mod evidence;
mod package_maps;
mod structural;

use std::process::ExitCode;

const USAGE: &str = "usage:\n\
  xtask validate accepted-evidence-digest\n\
  xtask compute accepted-evidence-digest <record> [--json-array]\n\
  xtask generate coverage-graph [--check] [--json]\n\
  xtask generate package-maps [--check] [--json]\n\
  xtask validate integration-bootstrap [--root <path>] [--allow-missing-lock] [--json]\n\
  xtask validate architecture-coverage [--json]\n\
  xtask validate architecture-coverage-contracts [--json]\n\
  xtask validate coverage-graph [--json]\n\
  xtask validate package-maps [--json]\n\
  xtask validate qdrant-boundary [--json]\n\
  xtask validate canonical-digest-guard\n";

pub fn run(args: &[String]) -> ExitCode {
    let Some((command, rest)) = args.split_first() else {
        return usage_error();
    };
    if command == "validate" {
        return run_validate(rest);
    }
    if command == "compute" {
        return run_compute(rest);
    }
    if command == "generate" {
        return run_generate(rest);
    }
    usage_error()
}

fn run_validate(args: &[String]) -> ExitCode {
    if args == ["canonical-digest-guard", "--inventory"] {
        return match xtask::canonical_digest_guard::inventory(std::path::Path::new(".")) {
            Ok(value) => { println!("{value}"); ExitCode::SUCCESS }
            Err(error) => { eprintln!("{error}"); ExitCode::FAILURE }
        };
    }
    if args == ["canonical-digest-guard"] {
        return match xtask::canonical_digest_guard::validate(std::path::Path::new(".")) {
            Ok(()) => {
                println!("canonical-digest-guard: PASS (source classification only)");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("canonical-digest-guard: FAIL\n{error}");
                ExitCode::FAILURE
            }
        };
    }
    if args == ["accepted-evidence-digest"] {
        return evidence::validate_digest();
    }
    if let Some(options) = options_for(args, "integration-bootstrap") {
        return bootstrap::validate(options);
    }
    if is_optional_json(args, "architecture-coverage") {
        return structural::validate_architecture_coverage();
    }
    if is_optional_json(args, "architecture-coverage-contracts") {
        return structural::validate_architecture_coverage_contracts();
    }
    if is_optional_json(args, "coverage-graph") {
        return structural::validate_coverage_graph();
    }
    if is_optional_json(args, "package-maps") {
        return structural::validate_package_map_closure();
    }
    if is_optional_json(args, "qdrant-boundary") {
        return structural::validate_qdrant_boundary();
    }
    usage_error()
}

fn run_compute(args: &[String]) -> ExitCode {
    let Some((target, options)) = args.split_first() else {
        return usage_error();
    };
    if target == "accepted-evidence-digest" {
        return evidence::compute_digest(options);
    }
    usage_error()
}

fn run_generate(args: &[String]) -> ExitCode {
    let Some((target, options)) = args.split_first() else {
        return usage_error();
    };
    if target == "coverage-graph" {
        return coverage_graph::generate(options);
    }
    if target == "package-maps" {
        return package_maps::generate(options);
    }
    usage_error()
}

pub fn usage_error() -> ExitCode {
    eprint!("{USAGE}");
    ExitCode::from(2)
}

fn is_optional_json(args: &[String], target: &str) -> bool {
    args == [target] || args == [target, "--json"]
}

fn options_for<'a>(args: &'a [String], target: &str) -> Option<&'a [String]> {
    let (actual, options) = args.split_first()?;
    (actual == target).then_some(options)
}
