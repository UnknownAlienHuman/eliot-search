//! Owner-, catalog-, scope-, policy-, access-, and purge-bound encrypted search.

#![deny(unsafe_op_in_unsafe_fn)]

#[path = "../sealed_access.rs"]
mod sealed_access;
#[path = "../sealed_access_codec.rs"]
mod sealed_access_codec;
#[path = "../sealed_catalog.rs"]
mod sealed_catalog;
#[path = "../sealed_digest.rs"]
mod sealed_digest;
#[path = "../sealed_exact.rs"]
mod sealed_exact;
#[path = "../sealed_owner_epoch.rs"]
mod sealed_owner_epoch;
#[path = "../sealed_recovery.rs"]
mod sealed_recovery;
#[path = "../sealed_root_identity.rs"]
mod sealed_root_identity;
#[path = "../sealed_root_lock.rs"]
mod sealed_root_lock;
#[path = "../sealed_store.rs"]
mod sealed_store;
#[path = "../sealed_transaction.rs"]
mod sealed_transaction;
#[path = "../sealed_transaction_guard.rs"]
mod sealed_transaction_guard;

use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::process::ExitCode;

use sealed_access::{
    AccessFenceMutation, AccessFenceSnapshot, ActiveAccessFence, append_fence,
    current_fence, require_active_fence,
};
use sealed_access_codec::AccessFenceState;
use sealed_catalog::{read_revision, verify_revision};
use sealed_exact::{ExactSearchResult, scan_exact};
use sealed_owner_epoch::OwnerEpochGuard;
use sealed_recovery::{SealedRecoveryReport, recover_all};
use sealed_root_identity::verify_owner_root;

fn help() -> &'static str {
    concat!(
        "eliot-search-sealed-authority\n\n",
        "USAGE:\n",
        "  eliot-search-sealed-authority allow DATA_ROOT FENCE_ID MUTATION_ID CATALOG_OBJECT_ID SOURCE_ID SOURCE_REVISION_ID SCOPE_ID SCOPE_REVISION POLICY_ID POLICY_REVISION PURGE_GENERATION\n",
        "  eliot-search-sealed-authority deny DATA_ROOT FENCE_ID MUTATION_ID CATALOG_OBJECT_ID SOURCE_ID SOURCE_REVISION_ID SCOPE_ID SCOPE_REVISION POLICY_ID POLICY_REVISION PURGE_GENERATION\n",
        "  eliot-search-sealed-authority status DATA_ROOT FENCE_ID\n",
        "  eliot-search-sealed-authority verify DATA_ROOT FENCE_ID CATALOG_OBJECT_ID SOURCE_ID SOURCE_REVISION_ID SCOPE_ID SCOPE_REVISION POLICY_ID POLICY_REVISION ACCESS_GENERATION PURGE_GENERATION\n",
        "  eliot-search-sealed-authority search DATA_ROOT FENCE_ID CATALOG_OBJECT_ID SOURCE_ID SOURCE_REVISION_ID SCOPE_ID SCOPE_REVISION POLICY_ID POLICY_REVISION ACCESS_GENERATION PURGE_GENERATION QUERY\n",
        "  eliot-search-sealed-authority search-ascii-insensitive DATA_ROOT FENCE_ID CATALOG_OBJECT_ID SOURCE_ID SOURCE_REVISION_ID SCOPE_ID SCOPE_REVISION POLICY_ID POLICY_REVISION ACCESS_GENERATION PURGE_GENERATION QUERY\n\n",
        "Windows only. Every command acquires a new monotone OwnerEpoch, proves ",
        "the exact physical root, completes startup recovery, and revalidates the ",
        "full append-only access chain. Search requires the request's exact scope, ",
        "policy, access and purge generations.\n",
    )
}

fn utf8_argument<'a>(value: &'a OsStr, code: &str) -> Result<&'a str, String> {
    value.to_str().ok_or_else(|| code.to_owned())
}

fn parse_u64(value: &OsStr, code: &str, allow_zero: bool) -> Result<u64, String> {
    let text = utf8_argument(value, code)?;
    if text.starts_with('+') || (text.starts_with('0') && text.len() > 1) {
        return Err(code.to_owned());
    }
    let parsed = text.parse::<u64>().map_err(|_| code.to_owned())?;
    if !allow_zero && parsed == 0 {
        return Err(code.to_owned());
    }
    Ok(parsed)
}

fn acquire_ready_owner(
    data_root: &Path,
) -> Result<(OwnerEpochGuard, SealedRecoveryReport), String> {
    let owner = OwnerEpochGuard::acquire(data_root)
        .map_err(|error| error.code().to_owned())?;
    verify_owner_root(data_root, &owner)
        .map_err(|error| error.code().to_owned())?;
    let recovery = recover_all(data_root, &owner)
        .map_err(|error| error.code().to_owned())?;
    if !recovery.ready {
        return Err("SEALED_RECOVERY_NOT_READY".to_owned());
    }
    Ok((owner, recovery))
}

fn mutation_from_arguments(
    arguments: &[std::ffi::OsString],
    state: AccessFenceState,
) -> Result<AccessFenceMutation, String> {
    Ok(AccessFenceMutation {
        fence_id: utf8_argument(&arguments[2], "SEALED_ACCESS_IDENTIFIER_INVALID")?
            .to_owned(),
        mutation_id: utf8_argument(
            &arguments[3],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?
        .to_owned(),
        catalog_object_id: utf8_argument(
            &arguments[4],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?
        .to_owned(),
        source_id: utf8_argument(
            &arguments[5],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?
        .to_owned(),
        source_revision_id: utf8_argument(
            &arguments[6],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?
        .to_owned(),
        scope_id: utf8_argument(
            &arguments[7],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?
        .to_owned(),
        scope_revision: parse_u64(
            &arguments[8],
            "SEALED_ACCESS_SCOPE_REVISION_INVALID",
            false,
        )?,
        policy_id: utf8_argument(
            &arguments[9],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?
        .to_owned(),
        policy_revision: parse_u64(
            &arguments[10],
            "SEALED_ACCESS_POLICY_REVISION_INVALID",
            false,
        )?,
        purge_generation: parse_u64(
            &arguments[11],
            "SEALED_ACCESS_PURGE_GENERATION_INVALID",
            true,
        )?,
        state,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpectedFence<'a> {
    catalog_object_id: &'a str,
    source_id: &'a str,
    source_revision_id: &'a str,
    scope_id: &'a str,
    scope_revision: u64,
    policy_id: &'a str,
    policy_revision: u64,
    access_generation: u64,
    purge_generation: u64,
}

fn expected_fence<'a>(
    arguments: &'a [std::ffi::OsString],
) -> Result<ExpectedFence<'a>, String> {
    Ok(ExpectedFence {
        catalog_object_id: utf8_argument(
            &arguments[3],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?,
        source_id: utf8_argument(
            &arguments[4],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?,
        source_revision_id: utf8_argument(
            &arguments[5],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?,
        scope_id: utf8_argument(
            &arguments[6],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?,
        scope_revision: parse_u64(
            &arguments[7],
            "SEALED_ACCESS_SCOPE_REVISION_INVALID",
            false,
        )?,
        policy_id: utf8_argument(
            &arguments[8],
            "SEALED_ACCESS_IDENTIFIER_INVALID",
        )?,
        policy_revision: parse_u64(
            &arguments[9],
            "SEALED_ACCESS_POLICY_REVISION_INVALID",
            false,
        )?,
        access_generation: parse_u64(
            &arguments[10],
            "SEALED_ACCESS_GENERATION_INVALID",
            false,
        )?,
        purge_generation: parse_u64(
            &arguments[11],
            "SEALED_ACCESS_PURGE_GENERATION_INVALID",
            true,
        )?,
    })
}

fn require_expected_fence(
    active: &ActiveAccessFence,
    expected: &ExpectedFence<'_>,
) -> Result<(), String> {
    let record = active.record();
    if record.catalog_object_id != expected.catalog_object_id
        || record.source_id != expected.source_id
        || record.source_revision_id != expected.source_revision_id
        || record.scope_id != expected.scope_id
        || record.scope_revision != expected.scope_revision
        || record.policy_id != expected.policy_id
        || record.policy_revision != expected.policy_revision
        || record.access_generation != expected.access_generation
        || record.purge_generation != expected.purge_generation
    {
        return Err("SEALED_AUTHORITY_FENCE_MISMATCH".to_owned());
    }
    Ok(())
}

fn emit_fence(
    event: &str,
    snapshot: &AccessFenceSnapshot,
    owner: &OwnerEpochGuard,
    recovery: &SealedRecoveryReport,
    disposition: Option<&str>,
) {
    println!(
        concat!(
            "{{\"event\":\"{}\",\"disposition\":{},",
            "\"fence_id\":\"{}\",\"mutation_id\":\"{}\",",
            "\"fence_generation\":{},\"state\":\"{}\",",
            "\"source_id\":\"{}\",\"source_revision_id\":\"{}\",",
            "\"catalog_object_id\":\"{}\",",
            "\"scope_id\":\"{}\",\"scope_revision\":{},",
            "\"policy_id\":\"{}\",\"policy_revision\":{},",
            "\"access_generation\":{},\"purge_generation\":{},",
            "\"admitted_owner_epoch\":{},\"current_owner_epoch\":{},",
            "\"fence_sha256\":\"{}\",",
            "\"startup_recovery_scanned\":{},",
            "\"startup_recovery_reconciled\":{},",
            "\"sealed_object_backed\":true,\"catalog_bound\":true,",
            "\"owner_epoch_bound\":true,\"scope_bound\":true,",
            "\"policy_bound\":true,\"access_generation_bound\":true,",
            "\"purge_generation_bound\":true,",
            "\"production_ready\":false}}"
        ),
        event,
        disposition
            .map(|value| format!("\"{value}\""))
            .unwrap_or_else(|| "null".to_owned()),
        snapshot.record.fence_id,
        snapshot.record.mutation_id,
        snapshot.record.generation,
        snapshot.record.state.as_str(),
        snapshot.record.source_id,
        snapshot.record.source_revision_id,
        snapshot.record.catalog_object_id,
        snapshot.record.scope_id,
        snapshot.record.scope_revision,
        snapshot.record.policy_id,
        snapshot.record.policy_revision,
        snapshot.record.access_generation,
        snapshot.record.purge_generation,
        snapshot.record.admitted_owner_epoch,
        owner.epoch(),
        snapshot.record_sha256,
        recovery.scanned_operations,
        recovery.reconciled_operations,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_search(
    active: &ActiveAccessFence,
    owner: &OwnerEpochGuard,
    recovery: &SealedRecoveryReport,
    content_sha256: &str,
    result: &ExactSearchResult,
    ascii_insensitive: bool,
) {
    let fence = active.record();
    println!(
        concat!(
            "{{\"event\":\"search_started\",",
            "\"fence_id\":\"{}\",\"fence_generation\":{},",
            "\"scope_id\":\"{}\",\"scope_revision\":{},",
            "\"policy_id\":\"{}\",\"policy_revision\":{},",
            "\"access_generation\":{},\"purge_generation\":{},",
            "\"source_id\":\"{}\",\"source_revision_id\":\"{}\",",
            "\"catalog_object_id\":\"{}\",\"content_sha256\":\"{}\",",
            "\"admitted_owner_epoch\":{},\"current_owner_epoch\":{},",
            "\"mode\":\"{}\",\"input_bytes\":{},",
            "\"startup_recovery_scanned\":{},",
            "\"startup_recovery_reconciled\":{},",
            "\"sealed_object_backed\":true,\"catalog_bound\":true,",
            "\"transaction_authority_bound\":true,",
            "\"owner_epoch_bound\":true,\"scope_bound\":true,",
            "\"policy_bound\":true,\"access_generation_bound\":true,",
            "\"purge_generation_bound\":true,",
            "\"production_ready\":false}}"
        ),
        fence.fence_id,
        fence.generation,
        fence.scope_id,
        fence.scope_revision,
        fence.policy_id,
        fence.policy_revision,
        fence.access_generation,
        fence.purge_generation,
        fence.source_id,
        fence.source_revision_id,
        fence.catalog_object_id,
        content_sha256,
        fence.admitted_owner_epoch,
        owner.epoch(),
        if ascii_insensitive {
            "ascii_insensitive"
        } else {
            "sensitive"
        },
        result.input_bytes,
        recovery.scanned_operations,
        recovery.reconciled_operations,
    );
    for item in &result.matches {
        println!(
            concat!(
                "{{\"event\":\"match\",\"byte_start\":{},",
                "\"byte_end\":{},\"line\":{},\"column_bytes\":{}}}"
            ),
            item.byte_start,
            item.byte_end,
            item.line,
            item.column_bytes,
        );
    }
    println!(
        concat!(
            "{{\"event\":\"search_complete\",\"matches\":{},",
            "\"match_limit_reached\":{},\"complete\":{},",
            "\"scope_bound\":true,\"policy_bound\":true,",
            "\"access_generation_bound\":true,",
            "\"purge_generation_bound\":true,",
            "\"production_ready\":false}}"
        ),
        result.matches.len(),
        result.match_limit_reached,
        result.complete,
    );
}

fn run() -> Result<(), String> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let Some(raw_command) = arguments.first() else {
        print!("{}", help());
        return Ok(());
    };
    let command = utf8_argument(raw_command, "SEALED_AUTHORITY_COMMAND_INVALID")?;
    if matches!(command, "--help" | "-h") {
        if arguments.len() != 1 {
            return Err("SEALED_AUTHORITY_USAGE_ERROR".to_owned());
        }
        print!("{}", help());
        return Ok(());
    }

    match command {
        "allow" | "deny" if arguments.len() == 12 => {
            let data_root = Path::new(&arguments[1]);
            let (owner, recovery) = acquire_ready_owner(data_root)?;
            let mutation = mutation_from_arguments(
                &arguments,
                if command == "allow" {
                    AccessFenceState::Allow
                } else {
                    AccessFenceState::Deny
                },
            )?;
            if mutation.state == AccessFenceState::Allow {
                let _ = verify_revision(
                    data_root,
                    &owner,
                    &mutation.catalog_object_id,
                    &mutation.source_id,
                    &mutation.source_revision_id,
                )
                .map_err(|error| error.code().to_owned())?;
            }
            let receipt = append_fence(data_root, &owner, mutation)
                .map_err(|error| error.code().to_owned())?;
            emit_fence(
                "fence_mutation",
                &receipt.affected,
                &owner,
                &recovery,
                Some(receipt.disposition.as_str()),
            );
            if receipt.affected.record.generation != receipt.current.record.generation {
                emit_fence(
                    "fence_current",
                    &receipt.current,
                    &owner,
                    &recovery,
                    None,
                );
            }
        }
        "status" if arguments.len() == 3 => {
            let data_root = Path::new(&arguments[1]);
            let (owner, recovery) = acquire_ready_owner(data_root)?;
            let fence_id = utf8_argument(
                &arguments[2],
                "SEALED_ACCESS_IDENTIFIER_INVALID",
            )?;
            let snapshot = current_fence(data_root, &owner, fence_id)
                .map_err(|error| error.code().to_owned())?;
            emit_fence("fence_status", &snapshot, &owner, &recovery, None);
        }
        "verify" if arguments.len() == 12 => {
            let data_root = Path::new(&arguments[1]);
            let (owner, recovery) = acquire_ready_owner(data_root)?;
            let fence_id = utf8_argument(
                &arguments[2],
                "SEALED_ACCESS_IDENTIFIER_INVALID",
            )?;
            let expected = expected_fence(&arguments)?;
            let active = require_active_fence(
                data_root,
                &owner,
                fence_id,
                expected.source_id,
                expected.source_revision_id,
                expected.catalog_object_id,
            )
            .map_err(|error| error.code().to_owned())?;
            require_expected_fence(&active, &expected)?;
            let receipt = verify_revision(
                data_root,
                &owner,
                expected.catalog_object_id,
                expected.source_id,
                expected.source_revision_id,
            )
            .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"AUTHORITY_VERIFIED\",",
                    "\"fence_id\":\"{}\",\"fence_generation\":{},",
                    "\"scope_id\":\"{}\",\"scope_revision\":{},",
                    "\"policy_id\":\"{}\",\"policy_revision\":{},",
                    "\"access_generation\":{},\"purge_generation\":{},",
                    "\"source_id\":\"{}\",\"source_revision_id\":\"{}\",",
                    "\"catalog_object_id\":\"{}\",\"content_sha256\":\"{}\",",
                    "\"current_owner_epoch\":{},",
                    "\"startup_recovery_scanned\":{},",
                    "\"scope_bound\":true,\"policy_bound\":true,",
                    "\"access_generation_bound\":true,",
                    "\"purge_generation_bound\":true,",
                    "\"production_ready\":false}}"
                ),
                active.record().fence_id,
                active.record().generation,
                active.record().scope_id,
                active.record().scope_revision,
                active.record().policy_id,
                active.record().policy_revision,
                active.record().access_generation,
                active.record().purge_generation,
                receipt.source_id,
                receipt.source_revision_id,
                receipt.catalog_object_id,
                receipt.content_sha256,
                owner.epoch(),
                recovery.scanned_operations,
            );
        }
        "search" | "search-ascii-insensitive" if arguments.len() == 13 => {
            let data_root = Path::new(&arguments[1]);
            let (owner, recovery) = acquire_ready_owner(data_root)?;
            let fence_id = utf8_argument(
                &arguments[2],
                "SEALED_ACCESS_IDENTIFIER_INVALID",
            )?;
            let expected = expected_fence(&arguments)?;
            let query = utf8_argument(
                &arguments[12],
                "SEALED_EXACT_QUERY_INVALID_UTF8",
            )?;
            let active = require_active_fence(
                data_root,
                &owner,
                fence_id,
                expected.source_id,
                expected.source_revision_id,
                expected.catalog_object_id,
            )
            .map_err(|error| error.code().to_owned())?;
            require_expected_fence(&active, &expected)?;
            let revision = read_revision(
                data_root,
                &owner,
                expected.catalog_object_id,
                expected.source_id,
                expected.source_revision_id,
            )
            .map_err(|error| error.code().to_owned())?;
            let result = scan_exact(
                revision.content.expose(),
                query,
                command == "search-ascii-insensitive",
            )
            .map_err(|error| error.code().to_owned())?;
            emit_search(
                &active,
                &owner,
                &recovery,
                &revision.binding.content_sha256.to_hex(),
                &result,
                command == "search-ascii-insensitive",
            );
        }
        _ => return Err("SEALED_AUTHORITY_USAGE_ERROR".to_owned()),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", error.replace('"', "'"));
            ExitCode::from(2)
        }
    }
}
