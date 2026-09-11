//! `xtask`: bounded Rust validation tooling (T41).
//!
//! ```text
//! xtask validate accepted-evidence-digest
//! xtask compute accepted-evidence-digest <record> [--json-array]
//! ```
//!
//! Small explicit surface only; not a swarm controller.

use std::path::PathBuf;
use std::process::ExitCode;

use xtask::compute_accepted_evidence::compute_from_record_file;
use xtask::validate_accepted_evidence::{
    exit_code, render_report_json, validate_accepted_evidence_digest,
};

const USAGE: &str = "usage:\n  xtask validate accepted-evidence-digest\n  xtask compute accepted-evidence-digest <record> [--json-array]\n";

fn usage_error() -> ExitCode {
    eprint!("{USAGE}");
    ExitCode::from(2)
}

fn run_validate() -> ExitCode {
    let root = PathBuf::from(".");
    let report = validate_accepted_evidence_digest(&root);
    println!("{}", render_report_json(&report));
    ExitCode::from(u8::try_from(exit_code(&report)).unwrap_or(1))
}

fn run_compute(args: &[String]) -> ExitCode {
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
    match compute_from_record_file(PathBuf::from(record).as_path(), json_array) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("ACCEPTED_EVIDENCE_DIGEST_INVALID: {err}");
            ExitCode::from(2)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [command, rest @ ..] = args.as_slice() else {
        return usage_error();
    };
    if command == "validate" && rest == ["accepted-evidence-digest"] {
        return run_validate();
    }
    if command == "compute" {
        let [target, rest @ ..] = rest else {
            return usage_error();
        };
        if target == "accepted-evidence-digest" {
            return run_compute(rest);
        }
    }
    usage_error()
}
