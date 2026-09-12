use std::path::PathBuf;
use std::process::ExitCode;

use xtask::ticket_issuance_validation::{
    exit_code, render_report_json, render_report_text,
    validate_ticket_issuance_plan,
};

use super::usage_error;

pub(super) fn validate(args: &[String]) -> ExitCode {
    let mut root = PathBuf::from(".");
    let mut json = false;
    let mut index = 0_usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return usage_error();
                };
                root = PathBuf::from(path);
            }
            "--json" => json = true,
            _ => return usage_error(),
        }
        index += 1;
    }
    let root = std::fs::canonicalize(&root).unwrap_or(root);
    let report = validate_ticket_issuance_plan(&root);
    if json {
        println!("{}", render_report_json(&report));
    } else {
        println!("{}", render_report_text(&report));
    }
    ExitCode::from(u8::try_from(exit_code(&report)).unwrap_or(1))
}
