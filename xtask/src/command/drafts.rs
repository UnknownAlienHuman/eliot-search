use std::path::Path;
use std::process::ExitCode;

use xtask::agent_drafts::{
    AgentDraftReport, exit_code as agent_drafts_exit_code,
    render_report_json as render_agent_drafts_json,
    render_w3_report_json, render_w4_report_json,
    validate_w1_agent_drafts, validate_w2_agent_drafts,
    validate_w3_agent_drafts, validate_w4_agent_drafts, w3_exit_code,
    w4_exit_code,
};
use xtask::ticket_drafts::{
    exit_code as ticket_exit_code,
    render_report_json as render_ticket_json,
    validate_p00_ticket_drafts,
};

pub(super) fn validate_ticket_drafts() -> ExitCode {
    let report = validate_p00_ticket_drafts(Path::new("."));
    println!("{}", render_ticket_json(&report));
    code(ticket_exit_code(&report))
}

pub(super) fn validate_w1() -> ExitCode {
    validate_common(validate_w1_agent_drafts)
}

pub(super) fn validate_w2() -> ExitCode {
    validate_common(validate_w2_agent_drafts)
}

pub(super) fn validate_w3() -> ExitCode {
    let report = validate_w3_agent_drafts(Path::new("."));
    println!("{}", render_w3_report_json(&report));
    code(w3_exit_code(&report))
}

pub(super) fn validate_w4() -> ExitCode {
    let report = validate_w4_agent_drafts(Path::new("."));
    println!("{}", render_w4_report_json(&report));
    code(w4_exit_code(&report))
}

fn validate_common(
    validator: fn(&Path) -> AgentDraftReport,
) -> ExitCode {
    let report = validator(Path::new("."));
    println!("{}", render_agent_drafts_json(&report));
    code(agent_drafts_exit_code(&report))
}

fn code(value: i32) -> ExitCode {
    ExitCode::from(u8::try_from(value).unwrap_or(1))
}
