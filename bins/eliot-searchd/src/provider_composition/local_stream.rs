//! Single local byte-stream boundary shared by pairing and typed provider I/O.
//!
//! Endpoint naming, listener/connect effects and admission belong to platform
//! adapters. This module owns only nonblocking finite `Read`/`Write` mechanics.
//! A local stream never authenticates its peer by itself. Closing is represented
//! by dropping the single stream owner; local sockets and named pipes have no
//! portable socket-style shutdown operation.

use std::io::{self, Read, Write};
use std::net::TcpStream;

/// One owned or borrowed local transport stream.
///
/// Implementations must not create hidden readers, writers, retry loops or
/// alternate framing. `configure_nonblocking` may change only the I/O mode of
/// the already-connected stream. Pairing and typed-session owners retain every
/// deadline, cancellation, sequence and authentication decision.
pub(crate) trait LocalByteStream: Read + Write {
    /// Configure bounded nonblocking polling on the already-connected stream.
    fn configure_nonblocking(&self) -> io::Result<()>;
}

impl LocalByteStream for TcpStream {
    fn configure_nonblocking(&self) -> io::Result<()> {
        self.set_nonblocking(true)
    }
}
