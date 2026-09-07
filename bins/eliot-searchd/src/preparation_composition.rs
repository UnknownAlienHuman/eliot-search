//! Explicit preparation/reconstruction from a retained revision, never a live path.

use std::path::Path;
use std::process::ExitCode;
use crate::{development::DataRootGuard, direct_store::DirectStore, direct_preparation, sha256};
use crate::service_output::{json_string, write_line};

pub(crate) fn maybe_run() -> Option<ExitCode> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().and_then(|arg| arg.to_str()) != Some("--prepare-revision") { return None; }
    let result = (|| -> Result<(), String> {
        if args.len() != 3 { return Err("USAGE_ERROR".to_owned()); }
        let revision = args[2].to_str().ok_or_else(|| "DIRECT_REVISION_ID_INVALID".to_owned())?;
        if sha256::decode_digest(revision).is_none() { return Err("DIRECT_REVISION_ID_INVALID".to_owned()); }
        let owner = DataRootGuard::acquire(Path::new(&args[1]))?;
        crate::catalog_presence::require_existing(owner.canonical_root())?;
        let mut store = DirectStore::open(owner.canonical_root())?;
        store.prepare_revision(revision)?;
        write_line(&mut std::io::stdout().lock(), &format!(
            "{{\"event\":\"revision_preparation_stored\",\"revision_id\":{},\"profile_sha256\":{}}}",
            json_string(revision), json_string(&sha256::hex(&direct_preparation::profile_digest())),
        ))
    })();
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            ExitCode::from(2)
        }
    })
}
