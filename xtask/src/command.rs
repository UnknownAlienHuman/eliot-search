mod bootstrap;
mod context_artifact;
mod context_artifact_build;
mod context_materialization_build;
mod drafts;
mod evidence;
mod milestones;
mod package_maps;
mod structural;
mod ticket_issuance;

use std::process::ExitCode;

const USAGE: &str = "usage:\n\
  xtask validate accepted-evidence-digest\n\
  xtask compute accepted-evidence-digest <record> [--json-array]\n\
  xtask build context-artifact-candidate --package <name> --base-commit <algorithm:oid> [--root <path>] [--accepted-handoff <path>]... [--output-root <path>] [--print-result]\n\
  xtask build context-materialization-plan --candidate <path> [--root <path>] [--bundle <path>] [--selection <path>] [--output-root <path>] [--write] [--require-ready]\n\
  xtask generate package-maps [--check] [--json]\n\
  xtask validate p00-ticket-drafts [--json]\n\
  xtask validate w1-agent-drafts [--json]\n\
  xtask validate w2-agent-drafts [--json]\n\
  xtask validate w3-agent-drafts [--json]\n\
  xtask validate w4-agent-drafts [--json]\n\
  xtask validate w1-milestone-packets [--json]\n\
  xtask validate w2-milestone-packets [--json]\n\
  xtask validate w3-milestone-packets [--json]\n\
  xtask validate integration-bootstrap [--root <path>] [--allow-missing-lock] [--json]\n\
  xtask validate architecture-coverage [--json]\n\
  xtask validate architecture-coverage-contracts [--json]\n\
  xtask validate coverage-graph [--json]\n\
  xtask validate package-maps [--json]\n\
  xtask validate context-artifact-candidate [--root <path>] [--json]\n\
  xtask validate context-materialization-plan [--json]\n\
  xtask validate ticket-issuance-plan [--root <path>] [--json]\n\
  xtask validate implementation-program [--json]\n\
  xtask validate p00-foundation-acceptance [--json]\n\
  xtask validate qdrant-boundary [--json]\n";

pub(super) fn run(args: &[String]) -> ExitCode {
    let Some((command, rest)) = args.split_first() else {
        return usage_error();
    };
    if command == "validate" {
        return run_validate(rest);
    }
    if command == "compute" {
        return run_compute(rest);
    }
    if command == "build" {
        return run_build(rest);
    }
    if command == "generate" {
        return run_generate(rest);
    }
    usage_error()
}

fn run_validate(args: &[String]) -> ExitCode {
    if args == ["accepted-evidence-digest"] {
        return evidence::validate_digest();
    }
    if is_optional_json(args, "p00-ticket-drafts") {
        return drafts::validate_ticket_drafts();
    }
    if is_optional_json(args, "w1-agent-drafts") {
        return drafts::validate_w1();
    }
    if is_optional_json(args, "w2-agent-drafts") {
        return drafts::validate_w2();
    }
    if is_optional_json(args, "w3-agent-drafts") {
        return drafts::validate_w3();
    }
    if is_optional_json(args, "w4-agent-drafts") {
        return drafts::validate_w4();
    }
    if is_optional_json(args, "w1-milestone-packets") {
        return milestones::validate_w1();
    }
    if is_optional_json(args, "w2-milestone-packets") {
        return milestones::validate_w2();
    }
    if is_optional_json(args, "w3-milestone-packets") {
        return milestones::validate_w3();
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
    if let Some(options) = options_for(args, "context-artifact-candidate") {
        return context_artifact::validate(options);
    }
    if is_optional_json(args, "context-materialization-plan") {
        return structural::validate_context_materialization_plan();
    }
    if let Some(options) = options_for(args, "ticket-issuance-plan") {
        return ticket_issuance::validate(options);
    }
    if is_optional_json(args, "implementation-program") {
        return structural::validate_implementation_program();
    }
    if is_optional_json(args, "p00-foundation-acceptance") {
        return structural::validate_p00_acceptance();
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

fn run_build(args: &[String]) -> ExitCode {
    let Some((target, options)) = args.split_first() else {
        return usage_error();
    };
    if target == "context-artifact-candidate" {
        return context_artifact_build::build(options);
    }
    if target == "context-materialization-plan" {
        return context_materialization_build::build(options);
    }
    usage_error()
}

fn run_generate(args: &[String]) -> ExitCode {
    let Some((target, options)) = args.split_first() else {
        return usage_error();
    };
    if target == "package-maps" {
        return package_maps::generate(options);
    }
    usage_error()
}

pub(super) fn usage_error() -> ExitCode {
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
