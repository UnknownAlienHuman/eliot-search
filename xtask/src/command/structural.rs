use std::path::Path;
use std::process::ExitCode;

use xtask::architecture_coverage_contracts::{
    exit_code as architecture_exit_code,
    render_report_json as render_architecture_json,
    validate_architecture_coverage_contracts as validate_architecture,
};
use xtask::context_materialization_validation::{
    exit_code as materialization_exit_code,
    render_report_json as render_materialization_json,
    validate_context_materialization_plan as validate_materialization,
};
use xtask::impl_program::{
    exit_code as program_exit_code,
    render_report_json as render_program_json,
    validate_implementation_program as validate_program,
};
use xtask::p00_acceptance::{
    exit_code as acceptance_exit_code,
    render_report_json as render_acceptance_json,
    validate_p00_foundation_acceptance,
};
use xtask::qdrant_boundary::{
    exit_code as qdrant_exit_code,
    render_report_json as render_qdrant_json,
    validate_qdrant_boundary as validate_qdrant,
};

pub(super) fn validate_architecture_coverage_contracts() -> ExitCode {
    let report = validate_architecture(Path::new("."));
    println!("{}", render_architecture_json(&report));
    code(architecture_exit_code(&report))
}

pub(super) fn validate_context_materialization_plan() -> ExitCode {
    let report = validate_materialization(Path::new("."));
    println!("{}", render_materialization_json(&report));
    code(materialization_exit_code(&report))
}

pub(super) fn validate_implementation_program() -> ExitCode {
    let report = validate_program(Path::new("."));
    println!("{}", render_program_json(&report));
    code(program_exit_code(&report))
}

pub(super) fn validate_p00_acceptance() -> ExitCode {
    let report = validate_p00_foundation_acceptance(Path::new("."));
    println!("{}", render_acceptance_json(&report));
    code(acceptance_exit_code(&report))
}

pub(super) fn validate_qdrant_boundary() -> ExitCode {
    let report = validate_qdrant(Path::new("."));
    println!("{}", render_qdrant_json(&report));
    code(qdrant_exit_code(&report))
}

fn code(value: i32) -> ExitCode {
    ExitCode::from(u8::try_from(value).unwrap_or(1))
}
