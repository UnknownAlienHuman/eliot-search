//! Concrete owned-child query client for the local CLI.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod child;

use search_provider_protocol::{ProtocolError, RawExactRequest, SearchRequest, Terminal};
use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tokio::runtime::{Builder, Handle, Runtime};

/// One reusable parent I/O runtime. Each call owns exactly one query child.
pub struct QueryClient {
    runtime: Runtime,
    cancel_on_ctrl_c: bool,
}

/// Validated application terminal after the owned child has exited and been reaped.
pub struct QueryCompletion {
    /// One fully decoded and validated terminal.
    pub terminal: Terminal,
    /// Retained untrusted stderr bytes, at most 64 KiB; render safely.
    pub diagnostics: Vec<u8>,
    /// Whether excess diagnostic bytes were drained and discarded.
    pub diagnostics_truncated: bool,
}

/// Content-free transport/process failures; application failures remain terminals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientError {
    /// A complete frame or DTO failed validation.
    Protocol(ProtocolError),
    /// Synchronous entry was invoked inside an async runtime.
    AsyncContext,
    /// The one parent I/O runtime could not be constructed.
    Runtime,
    /// The explicitly requested OS Ctrl-C listener could not be installed.
    Signal,
    /// The selected native child could not be started.
    Start,
    /// A required owned-child pipe was unavailable.
    Pipe,
    /// A pipe operation failed.
    Io,
    /// A finite local allocation failed.
    Allocation,
    /// The caller's original absolute deadline expired.
    Deadline,
    /// Cancellation closed a partially submitted or unresponsive request.
    Cancelled,
    /// Child stdout ended before its complete terminal.
    Truncated,
    /// The sole terminal was followed by unexpected stdout bytes.
    TrailingOutput,
    /// A successful terminal was followed by a failed child exit.
    ChildExit,
    /// Child exit could not be observed/reaped.
    Wait,
    /// The child or its output did not finish within the cleanup allowance.
    Cleanup,
}

impl ClientError {
    /// Stable diagnostic without query, source text or raw OS errors.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Protocol(error) => error.code(),
            Self::AsyncContext => "QUERY_CLIENT_ASYNC_CONTEXT",
            Self::Runtime => "QUERY_CLIENT_RUNTIME_UNAVAILABLE",
            Self::Signal => "QUERY_CTRL_C_UNAVAILABLE",
            Self::Start => "QUERY_CHILD_START_FAILED",
            Self::Pipe => "QUERY_CHILD_PIPE_UNAVAILABLE",
            Self::Io => "QUERY_CHILD_IO_FAILED",
            Self::Allocation => "QUERY_ALLOCATION_FAILED",
            Self::Deadline => "QUERY_DEADLINE_EXPIRED",
            Self::Cancelled => "QUERY_CANCELLED",
            Self::Truncated => "QUERY_TERMINAL_TRUNCATED",
            Self::TrailingOutput => "QUERY_TRAILING_OUTPUT",
            Self::ChildExit => "QUERY_CHILD_EXIT_FAILED",
            Self::Wait => "QUERY_CHILD_REAP_FAILED",
            Self::Cleanup => "QUERY_CHILD_CLEANUP_EXPIRED",
        }
    }
}

impl From<ProtocolError> for ClientError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for ClientError {}

impl QueryClient {
    /// Creates the single parent runtime outside any async runtime context.
    pub fn new() -> Result<Self, ClientError> {
        synchronous()?;
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ClientError::Runtime)?;
        Ok(Self {
            runtime,
            cancel_on_ctrl_c: false,
        })
    }

    /// Enables maintained OS Ctrl-C handling for the one-shot CLI process.
    ///
    /// The listener is registered before spawning and is retained by the same
    /// I/O loop through correlated cancellation and observed child cleanup.
    pub const fn enable_ctrl_c(&mut self) {
        self.cancel_on_ctrl_c = true;
    }

    /// Owns one native child, serviced framed stdio, cancellation and observed reap.
    ///
    /// The caller starts deadline before setup and supplies a process-level
    /// cancellation flag. No pipe or child handle escapes this call. Diagnostics
    /// are drained concurrently even after their retention allowance is full.
    pub fn query_owned_child(
        &mut self,
        executable: &Path,
        args: &[OsString],
        request: SearchRequest,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<QueryCompletion, ClientError> {
        synchronous()?;
        request.validate()?;
        self.runtime.block_on(child::query(
            executable,
            args,
            child::Request::Search(request),
            deadline,
            cancelled,
            self.cancel_on_ctrl_c,
        ))
    }

    /// Owns one raw literal query child using the same serviced stdio and reap path.
    ///
    /// The complete terminal is validated against the literal, source selection and
    /// item limit before return. Raw source bytes remain in their hex representation.
    pub fn raw_exact_owned_child(
        &mut self,
        executable: &Path,
        args: &[OsString],
        request: RawExactRequest,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<QueryCompletion, ClientError> {
        synchronous()?;
        request.validate()?;
        self.runtime.block_on(child::query(
            executable,
            args,
            child::Request::RawExact(request),
            deadline,
            cancelled,
            self.cancel_on_ctrl_c,
        ))
    }
}

fn synchronous() -> Result<(), ClientError> {
    if Handle::try_current().is_ok() {
        return Err(ClientError::AsyncContext);
    }
    Ok(())
}
