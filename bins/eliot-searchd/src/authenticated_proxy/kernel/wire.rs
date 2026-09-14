//! Provider line write and fail-closed error framing.

use std::net::TcpStream;

use crate::endpoint::EndpointAction;

pub(super) fn write_provider_line(
    stream: &mut TcpStream,
    line: &str,
) -> Result<(), String> {
    use std::io::Write;

    stream
        .write_all(line.as_bytes())
        .and_then(|()| stream.write_all(b"\n"))
        .and_then(|()| stream.flush())
        .map_err(|_| "LOOPBACK_PROXY_WRITE_ERROR".to_owned())
}

pub(super) fn fail_with_provider_error(
    stream: &mut TcpStream,
    reason: &str,
) -> Result<EndpointAction, String> {
    let _ = write_provider_line(
        stream,
        &crate::provider_composition::render_provider_error(reason),
    );
    Err(reason.to_owned())
}
