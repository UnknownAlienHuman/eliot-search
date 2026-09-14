//! Loopback endpoint lifetime and truthful provider capabilities.

use std::cell::Cell;
use std::env;
use std::path::Path;
use std::rc::Rc;

use crate::endpoint;

use super::child::DirectChild;
use super::dispatch::dispatch_provider_command;
use super::key::ShimKeySource;

fn provider_capabilities(
) -> Result<crate::provider_composition::ProviderCapabilities, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let (_, cli) = crate::config_composition::parse_cli_config_args(&args)?;
    let effective = crate::config_composition::effective_from_process(&cli)?;
    let readiness = crate::config_composition::derive_readiness(
        &effective,
        crate::config_composition::direct_dependencies(),
        crate::config_composition::AcceptedReceipts::default(),
    );
    let evidence = crate::provider_composition::CapabilityEvidence::from_parts(
        readiness.capabilities.source_backed_search_available,
        readiness.capabilities.search_available,
        readiness.capabilities.indexed_search_available,
        readiness.blockers,
    )
    .map_err(|_| "DAEMON_CONFIG_INVALID".to_owned())?;
    Ok(crate::provider_composition::negotiate_capabilities(&evidence))
}

pub(super) fn run_proxy(
    root: &Path,
    port: u16,
    token_file: &Path,
) -> Result<(), String> {
    let key = crate::provider_composition::read_shim_key_file(token_file)?;
    let cache = Rc::new(Cell::new(None));
    let mut source = ShimKeySource::new(key, Rc::clone(&cache));
    let capabilities = provider_capabilities()?;
    let mut child = DirectChild::spawn(root)?;
    println!(concat!(
        "{{\"event\":\"direct_child_ready\",",
        "\"runtime_owner_ready\":true,",
        "\"source_backed_search_available\":true}}"
    ));
    let mut router: Option<crate::provider_composition::ProviderRouter> = None;
    let mut hello_counter = 0_u64;
    let endpoint_result = endpoint::serve_loopback_with_source(
        port,
        &mut source,
        |command, stream| {
            dispatch_provider_command(
                command,
                stream,
                &mut child,
                &mut router,
                &mut hello_counter,
                &capabilities,
                &cache,
            )
        },
    );
    if endpoint_result.is_err() {
        child.abort();
    }
    let child_result = child.finish();
    endpoint_result?;
    child_result
}
