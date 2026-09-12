use std::path::Path;
use std::process::ExitCode;

use xtask::package_maps::{
    PackageMapGenerationMode, generate_package_maps, generation_exit_code,
    render_generation_report_json,
};

use super::usage_error;

pub(super) fn generate(options: &[String]) -> ExitCode {
    let mut mode = PackageMapGenerationMode::Write;
    for option in options {
        match option.as_str() {
            "--check" => mode = PackageMapGenerationMode::Check,
            "--json" => {}
            _ => return usage_error(),
        }
    }
    let report = generate_package_maps(Path::new("."), mode);
    println!("{}", render_generation_report_json(&report));
    ExitCode::from(u8::try_from(generation_exit_code(&report)).unwrap_or(1))
}
