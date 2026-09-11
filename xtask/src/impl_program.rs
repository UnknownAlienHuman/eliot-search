//! Port of `tools/validate-implementation-program.py` (T41 slice).
//!
//! Read-only validator over the non-authoritative implementation program,
//! launch/stage/gate/package registries, product-pulse metrics, coverage
//! manifest, structural qualification cases and the manual-only workflow.
//! Parsing, validation rules and byte-exact report rendering are separated so
//! one concern cannot grow back into a validator monolith.

mod model;
mod parse;
mod report;
mod rules;

pub use model::{
    EXPECTED_BASELINE_REQUIRES, EXPECTED_GATE_IDS, EXPECTED_INTEGRATION_ORDER,
    EXPECTED_NEXT_ORDER, EXPECTED_PATHS, EXPECTED_STAGE_IDS, EXPECTED_TARGETS,
    ProgramReport, WORKFLOW, WORKFLOW_XTASK_TOKEN,
};
pub use report::{exit_code, render_report_json};
pub use rules::validate_implementation_program;
