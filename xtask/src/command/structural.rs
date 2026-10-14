use std::path::Path;
use std::process::ExitCode;

use xtask::architecture_coverage::{
    exit_code as full_architecture_exit_code,
    render_report_json as render_full_architecture_json,
    validate_architecture_coverage as validate_full_architecture,
};
use xtask::architecture_coverage_contracts::{
    exit_code as architecture_exit_code,
    render_report_json as render_architecture_json,
    validate_architecture_coverage_contracts as validate_architecture,
};
use xtask::coverage_graph_validation::{
    exit_code as coverage_graph_exit_code,
    render_report_json as render_coverage_graph_json,
    validate_coverage_graph as validate_coverage,
};
use xtask::impl_program::{
    exit_code as program_exit_code,
    render_report_json as render_program_json,
    validate_implementation_program as validate_program,
};
use xtask::package_maps::{
    exit_code as package_maps_exit_code,
    render_report_json as render_package_maps_json,
    validate_package_maps,
};
use xtask::qdrant_boundary::{
    exit_code as qdrant_exit_code,
    render_report_json as render_qdrant_json,
    validate_qdrant_boundary as validate_qdrant,
};

pub(super) fn validate_architecture_coverage() -> ExitCode {
    let report = validate_full_architecture(Path::new("."));
    println!("{}", render_full_architecture_json(&report));
    code(full_architecture_exit_code(&report))
}

pub(super) fn validate_architecture_coverage_contracts() -> ExitCode {
    let report = validate_architecture(Path::new("."));
    println!("{}", render_architecture_json(&report));
    code(architecture_exit_code(&report))
}

pub(super) fn validate_coverage_graph() -> ExitCode {
    let report = validate_coverage(Path::new("."));
    println!("{}", render_coverage_graph_json(&report));
    code(coverage_graph_exit_code(&report))
}

pub(super) fn validate_implementation_program() -> ExitCode {
    let report = validate_program(Path::new("."));
    println!("{}", render_program_json(&report));
    code(program_exit_code(&report))
}

pub(super) fn validate_package_map_closure() -> ExitCode {
    let report = validate_package_maps(Path::new("."));
    println!("{}", render_package_maps_json(&report));
    code(package_maps_exit_code(&report))
}

pub(super) fn validate_qdrant_boundary() -> ExitCode {
    let report = validate_qdrant(Path::new("."));
    match render_qdrant_json(&report) {
        Ok(json) => {
            println!("{json}");
            code(qdrant_exit_code(&report))
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn code(value: i32) -> ExitCode {
    ExitCode::from(u8::try_from(value).unwrap_or(1))
}
