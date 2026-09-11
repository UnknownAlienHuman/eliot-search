//! `xtask`: bounded Rust validation tooling (T41).
//!
//! ```text
//! xtask validate accepted-evidence-digest
//! xtask compute accepted-evidence-digest <record> [--json-array]
//! xtask validate p00-ticket-drafts [--json]
//! xtask validate w1-agent-drafts [--json]
//! xtask validate w2-agent-drafts [--json]
//! xtask validate w3-agent-drafts [--json]
//! xtask validate w4-agent-drafts [--json]
//! xtask validate implementation-program [--json]
//! xtask validate p00-foundation-acceptance [--json]
//! xtask validate qdrant-boundary [--json]
//! ```
//!
//! Small explicit surface only; not a swarm controller.

use std::path::PathBuf;
use std::process::ExitCode;

use xtask::agent_drafts::{
    AgentDraftReport, exit_code as agent_drafts_exit_code,
    render_report_json as render_agent_drafts_json,
    render_w3_report_json, render_w4_report_json,
    validate_w1_agent_drafts, validate_w2_agent_drafts,
    validate_w3_agent_drafts, validate_w4_agent_drafts, w3_exit_code,
    w4_exit_code,
};
use xtask::compute_accepted_evidence::compute_from_record_file;
use xtask::impl_program::{
    exit_code as program_exit_code,
    render_report_json as render_program_json,
    validate_implementation_program,
};
use xtask::p00_acceptance::{
    exit_code as acceptance_exit_code,
    render_report_json as render_acceptance_json,
    validate_p00_foundation_acceptance,
};
use xtask::qdrant_boundary::{
    exit_code as qdrant_boundary_exit_code,
    render_report_json as render_qdrant_boundary_json,
    validate_qdrant_boundary,
};
use xtask::ticket_drafts::{
    exit_code as drafts_exit_code,
    render_report_json as render_drafts_json,
    validate_p00_ticket_drafts,
};
use xtask::validate_accepted_evidence::{
    exit_code, render_report_json, validate_accepted_evidence_digest,
};

const USAGE: &str = "usage:\n\
  xtask validate accepted-evidence-digest\n\
  xtask compute accepted-evidence-digest <record> [--json-array]\n\
  xtask validate p00-ticket-drafts [--json]\n\
  xtask validate w1-agent-drafts [--json]\n\
  xtask validate w2-agent-drafts [--json]\n\
  xtask validate w3-agent-drafts [--json]\n\
  xtask validate w4-agent-drafts [--json]\n\
  xtask validate implementation-program [--json]\n\
  xtask validate p00-foundation-acceptance [--json]\n\
  xtask validate qdrant-boundary [--json]\n";

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
    match compute_from_record_file(PathBuf::from(record).as_path(), json_array)
    {
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

fn run_ticket_drafts() -> ExitCode {
    let root = PathBuf::from(".");
    let report = validate_p00_ticket_drafts(&root);
    println!("{}", render_drafts_json(&report));
    ExitCode::from(u8::try_from(drafts_exit_code(&report)).unwrap_or(1))
}

fn run_agent_drafts(
    validator: fn(&std::path::Path) -> AgentDraftReport,
) -> ExitCode {
    let root = PathBuf::from(".");
    let report = validator(&root);
    println!("{}", render_agent_drafts_json(&report));
    ExitCode::from(
        u8::try_from(agent_drafts_exit_code(&report)).unwrap_or(1),
    )
}

fn run_w3_agent_drafts() -> ExitCode {
    let root = PathBuf::from(".");
    let report = validate_w3_agent_drafts(&root);
    println!("{}", render_w3_report_json(&report));
    ExitCode::from(u8::try_from(w3_exit_code(&report)).unwrap_or(1))
}

fn run_w4_agent_drafts() -> ExitCode {
    let root = PathBuf::from(".");
    let report = validate_w4_agent_drafts(&root);
    println!("{}", render_w4_report_json(&report));
    ExitCode::from(u8::try_from(w4_exit_code(&report)).unwrap_or(1))
}

fn run_impl_program() -> ExitCode {
    let root = PathBuf::from(".");
    let report = validate_implementation_program(&root);
    println!("{}", render_program_json(&report));
    ExitCode::from(u8::try_from(program_exit_code(&report)).unwrap_or(1))
}

fn run_p00_acceptance() -> ExitCode {
    let root = PathBuf::from(".");
    let report = validate_p00_foundation_acceptance(&root);
    println!("{}", render_acceptance_json(&report));
    ExitCode::from(u8::try_from(acceptance_exit_code(&report)).unwrap_or(1))
}

fn run_qdrant_boundary() -> ExitCode {
    let root = PathBuf::from(".");
    let report = validate_qdrant_boundary(&root);
    println!("{}", render_qdrant_boundary_json(&report));
    ExitCode::from(
        u8::try_from(qdrant_boundary_exit_code(&report)).unwrap_or(1),
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [command, rest @ ..] = args.as_slice() else {
        return usage_error();
    };
    if command == "validate" {
        if rest == ["accepted-evidence-digest"] {
            return run_validate();
        }
        if rest == ["p00-ticket-drafts"]
            || rest == ["p00-ticket-drafts", "--json"]
        {
            return run_ticket_drafts();
        }
        if rest == ["w1-agent-drafts"]
            || rest == ["w1-agent-drafts", "--json"]
        {
            return run_agent_drafts(validate_w1_agent_drafts);
        }
        if rest == ["w2-agent-drafts"]
            || rest == ["w2-agent-drafts", "--json"]
        {
            return run_agent_drafts(validate_w2_agent_drafts);
        }
        if rest == ["w3-agent-drafts"]
            || rest == ["w3-agent-drafts", "--json"]
        {
            return run_w3_agent_drafts();
        }
        if rest == ["w4-agent-drafts"]
            || rest == ["w4-agent-drafts", "--json"]
        {
            return run_w4_agent_drafts();
        }
        if rest == ["implementation-program"]
            || rest == ["implementation-program", "--json"]
        {
            return run_impl_program();
        }
        if rest == ["p00-foundation-acceptance"]
            || rest == ["p00-foundation-acceptance", "--json"]
        {
            return run_p00_acceptance();
        }
        if rest == ["qdrant-boundary"]
            || rest == ["qdrant-boundary", "--json"]
        {
            return run_qdrant_boundary();
        }
        return usage_error();
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
