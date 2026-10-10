//! Nonpersistent stdin and file scan command handlers.

use super::output::emit_one_shot_scan;
use crate::development::{read_file_bounded, read_stdin_bounded};
use std::path::Path;
pub(super) fn cmd_scan_stdin(arguments: &[String], argument: &str) -> Result<(), String> {
    let text = read_stdin_bounded()?;
    emit_one_shot_scan(
        "stdin",
        &arguments[1],
        argument == "--scan-stdin-ascii-insensitive",
        &text,
        false,
        false,
    )
}

pub(super) fn cmd_scan_file(arguments: &[String], argument: &str) -> Result<(), String> {
    let text = read_file_bounded(Path::new(&arguments[2]))?;
    emit_one_shot_scan(
        "file",
        &arguments[1],
        argument == "--scan-file-ascii-insensitive",
        &text,
        true,
        true,
    )
}
