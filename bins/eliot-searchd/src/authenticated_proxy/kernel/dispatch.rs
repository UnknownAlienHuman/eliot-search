//! Fixed-order provider line dispatch.

use std::cell::Cell;
use std::net::TcpStream;
use std::rc::Rc;

use crate::endpoint::EndpointAction;

use super::child::DirectChild;
use super::envelope::do_envelope;
use super::hello::do_hello;
use super::key::cached_key;
use super::operation::do_op;
use super::wire::fail_with_provider_error;
use super::MAX_PROXY_COMMAND_BYTES;

pub(super) fn dispatch_provider_command(
    command: &str,
    stream: &mut TcpStream,
    child: &mut DirectChild,
    router: &mut Option<crate::provider_composition::ProviderRouter>,
    hello_counter: &mut u64,
    capabilities: &crate::provider_composition::ProviderCapabilities,
    cache: &Rc<Cell<Option<[u8; 32]>>>,
    input: Option<&mut crate::endpoint::EndpointInput>,
    completion: Option<&mut crate::endpoint::EndpointCompletion>,
) -> Result<EndpointAction, String> {
    if command.is_empty()
        || command.len() > MAX_PROXY_COMMAND_BYTES
        || command.contains('\n')
        || command.contains('\r')
    {
        return Err("LOOPBACK_DIRECT_COMMAND_INVALID".to_owned());
    }
    if command == "op\thello" || command.starts_with("op\thello\t") {
        let key = cached_key(cache)?;
        return do_hello(
            command,
            stream,
            router,
            hello_counter,
            capabilities,
            &key,
        );
    }
    if command.starts_with(crate::provider_composition::ENVELOPE_LINE_PREFIX) {
        let key = cached_key(cache)?;
        return do_envelope(command, stream, child, router, &key, input, completion);
    }
    if command.starts_with(crate::provider_composition::OP_LINE_PREFIX) {
        return do_op(command, stream, child, router, capabilities);
    }
    fail_with_provider_error(
        stream,
        crate::provider_composition::PROVIDER_UNKNOWN_COMMAND,
    )
}
