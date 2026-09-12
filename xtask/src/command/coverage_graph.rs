use std::path::Path;
use std::process::ExitCode;

use xtask::coverage_graph_generation::{
    CoverageGraphGenerationMode, generate_coverage_graph,
    generation_exit_code, render_generation_report_json,
};

use super::usage_error;

pub(super) fn generate(options: &[String]) -> ExitCode {
    let mut mode = CoverageGraphGenerationMode::Write;
    for option in options {
        match option.as_str() {
            "--check" => mode = CoverageGraphGenerationMode::Check,
            "--json" => {}
            _ => return usage_error(),
        }
    }
    let report = generate_coverage_graph(Path::new("."), mode);
    println!("{}", render_generation_report_json(&report));
    ExitCode::from(u8::try_from(generation_exit_code(&report)).unwrap_or(1))
}
