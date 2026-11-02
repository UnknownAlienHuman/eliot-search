//! Explicit native initialization, inspection and owner-only lifecycle check.

use std::ffi::OsString;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::owner_composition::{
    NativeRootAdmission, NativeRootLimits, NativeRootRequest, admit_existing,
};
use search_runtime_owner::{DataRootOpenMode, DrainReason, OwnerError, RootOpenState};

pub fn run_command(
    arguments: &[OsString],
    mode: DataRootOpenMode,
    started: Instant,
) -> ExitCode {
    let result = (|| {
        let expected = if mode == DataRootOpenMode::InitializeNew {
            3
        } else {
            2
        };
        if arguments.len() != expected {
            return Err(OwnerError::OwnerOperationConflict);
        }
        let deadline = started
            .checked_add(Duration::from_secs(120))
            .ok_or(OwnerError::OwnerDeadlineInvalid)?;
        let request = NativeRootRequest::new(
            mode,
            arguments,
            deadline,
            Arc::new(AtomicBool::new(false)),
            NativeRootLimits::new([256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256])?,
        )?;
        if mode == DataRootOpenMode::InitializeNew {
            return crate::owner_composition::initialize_direct(request)?.emit(&mut std::io::stdout().lock());
        }
        match admit_existing(request)? {
            NativeRootAdmission::Unavailable(state, request) => {
                let state_name = match state {
                    RootOpenState::Absent => "absent",
                    RootOpenState::Initialized => "initialized",
                    RootOpenState::Quarantined => "quarantined",
                    RootOpenState::OutcomeUnknown => "outcome_unknown",
                    RootOpenState::Corrupt => "corrupt",
                    RootOpenState::ForeignOwner => "foreign_owner",
                    RootOpenState::Unsupported => "unsupported",
                };
                request.emit(
                    &mut std::io::stdout().lock(),
                    &format!("{{\"owner_state\":\"{state_name}\",\"owner_capability\":false}}"),
                )
            }
            NativeRootAdmission::Inspection(mut owner) => {
                let metadata = owner.inspect()?;
                owner.emit(
                    &mut std::io::stdout().lock(),
                    &format!(
                        "{{\"owner_state\":\"initialized\",\"owner_epoch\":{},\"generation\":{},\"request_id\":\"{}\",\"request_digest\":\"{}\"}}",
                        metadata.owner_epoch.get(),
                        metadata.generation,
                        metadata.request_id,
                        search_contracts::Blake3Digest32::from_bytes(metadata.request_digest)
                    ),
                )
            }
            NativeRootAdmission::Mutation(mut owner) => {
                owner.begin_drain(DrainReason::Shutdown)?;
                let receipt = owner.release_cleanly()?;
                owner.emit(
                    &mut std::io::stdout().lock(),
                    &format!(
                        "{{\"owner_state\":\"released\",\"owner_epoch\":{},\"generation\":{}}}",
                        receipt.epoch.get(),
                        receipt.generation
                    ),
                )
            }
        }
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", error.code());
            ExitCode::from(2)
        }
    }
}
