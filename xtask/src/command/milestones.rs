use std::path::Path;
use std::process::ExitCode;

use xtask::milestone_packets::{
    MilestonePacketReport, exit_code as milestone_exit_code,
    render_report_json as render_milestone_json,
    render_w3_milestone_report_json, validate_w1_milestone_packets,
    validate_w2_milestone_packets, validate_w3_milestone_packets,
    w3_milestone_exit_code,
};

pub(super) fn validate_w1() -> ExitCode {
    validate_common(validate_w1_milestone_packets)
}

pub(super) fn validate_w2() -> ExitCode {
    validate_common(validate_w2_milestone_packets)
}

pub(super) fn validate_w3() -> ExitCode {
    let report = validate_w3_milestone_packets(Path::new("."));
    println!("{}", render_w3_milestone_report_json(&report));
    code(w3_milestone_exit_code(&report))
}

fn validate_common(
    validator: fn(&Path) -> MilestonePacketReport,
) -> ExitCode {
    let report = validator(Path::new("."));
    println!("{}", render_milestone_json(&report));
    code(milestone_exit_code(&report))
}

fn code(value: i32) -> ExitCode {
    ExitCode::from(u8::try_from(value).unwrap_or(1))
}
