//! Bounded line-oriented daemon control protocol.

use std::io::{self, BufRead, Write};

use crate::development::Health;

use super::spec::{
    Command, MAX_COMMAND_BYTES, MAX_RESPONSE_BYTES, PROTOCOL_VERSION,
    version_json,
};

fn write_response(output: &mut impl Write, value: &str) -> io::Result<()> {
    if value.len() > MAX_RESPONSE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "RESPONSE_TOO_LARGE",
        ));
    }
    output.write_all(value.as_bytes())?;
    output.write_all(b"\n")?;
    output.flush()
}

pub(super) fn serve_stdio(health: Health) -> io::Result<()> {
    serve_control(
        health,
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
    )
}

pub(super) fn serve_control(
    health: Health,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<()> {
    write_response(
        output,
        &format!(
            concat!(
                "{{\"event\":\"ready\",\"protocol_version\":{},",
                "\"runtime_owner_ready\":{},",
                "\"direct_store_ready\":{},",
                "\"source_backed_search_available\":{},",
                "\"search_available\":{},",
                "\"indexed_search_available\":{}}}"
            ),
            PROTOCOL_VERSION,
            health.composition.runtime_owner_ready,
            health.stores.direct_store_ready,
            health.capabilities.source_backed_search_available,
            health.capabilities.search_available,
            health.capabilities.indexed_search_available,
        ),
    )?;

    loop {
        let line = match crate::protocol_io::read_line(input, MAX_COMMAND_BYTES) {
            Ok(Some(line)) => line,
            Ok(None) => return Ok(()),
            Err(error) => {
                write_response(
                    output,
                    &format!("{{\"error\":\"{}\"}}", error.code()),
                )?;
                // Do not drain an unbounded frame or execute its unread suffix.
                return Err(io::Error::new(io::ErrorKind::InvalidData, error));
            }
        };
        match Command::parse(&line) {
            Ok(Command::Health) => write_response(output, &health.json())?,
            Ok(Command::Version) => write_response(output, &version_json())?,
            Ok(Command::Shutdown) => {
                write_response(
                    output,
                    "{\"status\":\"draining\",\"accepted\":true}",
                )?;
                write_response(
                    output,
                    "{\"status\":\"stopped\",\"clean\":true}",
                )?;
                return Ok(());
            }
            Err(code) => write_response(
                output,
                &format!("{{\"error\":\"{code}\"}}"),
            )?,
        }
    }
}
