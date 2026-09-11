use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xtask::compute_accepted_evidence::compute_from_record_file;
use xtask::validate_accepted_evidence::{
    exit_code, render_report_json, validate_accepted_evidence_digest,
};

use super::usage_error;

pub(super) fn validate_digest() -> ExitCode {
    let report = validate_accepted_evidence_digest(Path::new("."));
    println!("{}", render_report_json(&report));
    ExitCode::from(u8::try_from(exit_code(&report)).unwrap_or(1))
}

pub(super) fn compute_digest(args: &[String]) -> ExitCode {
    let mut record: Option<&str> = None;
    let mut json_array = false;
    for arg in args {
        if arg == "--json-array" {
            json_array = true;
        } else if record.is_none() {
            record = Some(arg.as_str());
        } else {
            return usage_error();
        }
    }
    let Some(record) = record else {
        return usage_error();
    };
    match compute_from_record_file(PathBuf::from(record).as_path(), json_array)
    {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ACCEPTED_EVIDENCE_DIGEST_INVALID: {error}");
            ExitCode::from(2)
        }
    }
}
