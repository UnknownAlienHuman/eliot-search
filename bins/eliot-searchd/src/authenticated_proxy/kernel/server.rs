//! Loopback endpoint lifetime and truthful provider capabilities.

use std::cell::Cell;
use std::env;
use std::net::TcpStream;
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
    let endpoint_result = endpoint::serve_loopback_with_handler(
        port,
        &mut source,
        ProxyConnection {
            child: &mut child,
            router: None,
            hello_counter: 0,
            capabilities: &capabilities,
            cache: &cache,
        },
    );
    if endpoint_result.is_err() {
        child.abort();
    }
    let child_result = child.finish();
    endpoint_result?;
    child_result
}

// The child remains the single data-root owner across connections. Only the
// provider session and its cached pairing material are connection-local.
struct ProxyConnection<'a> {
    child: &'a mut DirectChild,
    router: Option<crate::provider_composition::ProviderRouter>,
    hello_counter: u64,
    capabilities: &'a crate::provider_composition::ProviderCapabilities,
    cache: &'a Rc<Cell<Option<[u8; 32]>>>,
}

impl endpoint::EndpointConnectionHandler for ProxyConnection<'_> {
    fn command(
        &mut self,
        command: &str,
        stream: &mut TcpStream,
    ) -> Result<endpoint::EndpointAction, String> {
        dispatch_provider_command(
            command,
            stream,
            self.child,
            &mut self.router,
            &mut self.hello_counter,
            self.capabilities,
            self.cache,
        )
    }

    fn disconnected(&mut self) {
        if let Some(mut router) = self.router.take() {
            let _ = router.disconnect();
        }
        self.cache.set(None);
    }
}
