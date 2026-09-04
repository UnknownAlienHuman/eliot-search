//! Bounded startup reconciliation for the Windows sealed data root.

#![deny(unsafe_op_in_unsafe_fn)]

#[path = "../sealed_digest.rs"]
mod sealed_digest;
#[path = "../sealed_owner_epoch.rs"]
mod sealed_owner_epoch;
#[path = "../sealed_recovery.rs"]
mod sealed_recovery;
#[path = "../sealed_root_lock.rs"]
mod sealed_root_lock;
#[path = "../sealed_store.rs"]
mod sealed_store;
#[path = "../sealed_transaction.rs"]
mod sealed_transaction;
#[path = "../sealed_transaction_guard.rs"]
mod sealed_transaction_guard;

use std::path::Path;
use std::process::ExitCode;

use sealed_owner_epoch::OwnerEpochGuard;
use sealed_recovery::recover_all;

fn help() -> &'static str {
    concat!(
        "eliot-search-sealed-recover\n\n",
        "USAGE:\n",
        "  eliot-search-sealed-recover DATA_ROOT\n\n",
        "Acquires a new monotone OwnerEpoch, holds the exclusive Windows root ",
        "lock, inspects every bounded V2 transaction, verifies committed objects, ",
        "reconciles exact lost acknowledgements, and reports unresolved intents ",
        "without recreating missing source bytes.\n",
    )
}

fn run() -> Result<bool, String> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() == 1 && matches!(arguments[0].to_str(), Some("--help" | "-h")) {
        print!("{}", help());
        return Ok(true);
    }
    if arguments.len() != 1 {
        return Err("SEALED_RECOVERY_USAGE_ERROR".to_owned());
    }
    let data_root = Path::new(&arguments[0]);
    let owner = OwnerEpochGuard::acquire(data_root)
        .map_err(|error| error.code().to_owned())?;
    let report = recover_all(data_root, &owner)
        .map_err(|error| error.code().to_owned())?;
    println!(
        concat!(
            "{{\"event\":\"recovery_summary\",\"owner_epoch\":{},",
            "\"root_lock_held\":{},\"scanned_operations\":{},",
            "\"verified_committed\":{},\"reconciled_operations\":{},",
            "\"removed_temporary_files\":{},\"issues\":{},",
            "\"omitted_issue_count\":{},\"ready\":{}}}"
        ),
        report.owner_epoch,
        owner.root_lock_held(),
        report.scanned_operations,
        report.verified_committed,
        report.reconciled_operations,
        report.removed_temporary_files,
        report.issues.len(),
        report.omitted_issue_count,
        report.ready,
    );
    for issue in &report.issues {
        println!(
            "{{\"event\":\"recovery_issue\",\"operation_id\":\"{}\",\"code\":\"{}\"}}",
            issue.operation_id,
            issue.code.as_str(),
        );
    }
    Ok(report.ready)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(3),
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", error.replace('"', "'"));
            ExitCode::from(2)
        }
    }
}
