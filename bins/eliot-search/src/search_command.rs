//! Render only one fully validated terminal after owned-child exit/reap.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_provider_client::{ClientError, QueryClient, QueryCompletion};
use search_provider_protocol::{DecodedUtf8Request, QueryOutcome, RawExactRequest, SearchRequest};

fn failure(error: ClientError) -> i32 {
    eprintln!("{{\"error\":\"{}\"}}", error.code());
    match error {
        ClientError::Deadline => 124,
        ClientError::Cancelled => 130,
        _ => 2,
    }
}
/// Builds the one owned-child client for this command.
///
/// `source_info` requests the pinned view's saved locator for each emitted
/// item. Without it a terminal carries only source UUIDs, digests and byte
/// offsets, which identifies nothing an operator can open. It stays opt-in
/// because the locator is retained historical metadata, never a live-path
/// claim, and the daemon omits it from output when the flag is absent.
fn configured_client(source_info: bool) -> Result<QueryClient, ClientError> {
    let mut client = QueryClient::new()?;
    client.enable_ctrl_c();
    client.set_include_source_info(source_info);
    Ok(client)
}

pub fn run(
    daemon: &Path,
    arguments: &[OsString],
    request: SearchRequest,
    source_info: bool,
    deadline: Instant,
) -> i32 {
    let mut client = match configured_client(source_info) {
        Ok(client) => client,
        Err(error) => return failure(error),
    };
    let cancel = AtomicBool::new(false);
    let completion = match client.query_owned_child(daemon, arguments, request, deadline, &cancel) {
        Ok(completion) => completion,
        Err(error) => return failure(error),
    };
    render(&completion)
}

pub fn run_raw_exact(
    daemon: &Path,
    arguments: &[OsString],
    request: RawExactRequest,
    source_info: bool,
    deadline: Instant,
) -> i32 {
    let mut client = match configured_client(source_info) {
        Ok(client) => client,
        Err(error) => return failure(error),
    };
    let cancel = AtomicBool::new(false);
    let completion =
        match client.raw_exact_owned_child(daemon, arguments, request, deadline, &cancel) {
            Ok(completion) => completion,
            Err(error) => return failure(error),
        };
    render(&completion)
}

pub fn run_decoded_utf8(
    daemon: &Path,
    arguments: &[OsString],
    request: DecodedUtf8Request,
    source_info: bool,
    deadline: Instant,
) -> i32 {
    let mut client = match configured_client(source_info) {
        Ok(client) => client,
        Err(error) => return failure(error),
    };
    let cancel = AtomicBool::new(false);
    let completion =
        match client.decoded_utf8_owned_child(daemon, arguments, request, deadline, &cancel) {
            Ok(completion) => completion,
            Err(error) => return failure(error),
        };
    render(&completion)
}

fn render(completion: &QueryCompletion) -> i32 {
    let exit = match &completion.terminal.outcome {
        QueryOutcome::Complete { .. }
        | QueryOutcome::RawExact { .. }
        | QueryOutcome::DecodedUtf8 { .. } => 0,
        QueryOutcome::Cancelled => 130,
        QueryOutcome::Failed { .. } => 2,
    };
    // Serde escapes untrusted controls; raw source bytes retain their validated hex encoding.
    // No child bytes or diagnostics are streamed before complete terminal validation.
    let Ok(bytes) = serde_json::to_vec(&completion.terminal) else {
        eprintln!("{{\"error\":\"QUERY_SERIALIZATION_FAILED\"}}");
        return 2;
    };
    let mut stdout = std::io::stdout().lock();
    if stdout
        .write_all(&bytes)
        .and_then(|()| stdout.write_all(b"\n"))
        .and_then(|()| stdout.flush())
        .is_err()
    {
        eprintln!("{{\"error\":\"OUTPUT_WRITE_ERROR\"}}");
        return 2;
    }
    exit
}

/// Runs the existing session command using the same typed frames as the SDK.
pub fn run_session(daemon: &Path, arguments: &[OsString], deadline: Instant) -> i32 {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return failure(ClientError::Runtime);
    };
    let result = runtime.block_on(session_stdio(daemon, arguments, deadline));
    // Tokio stdin may retain one blocking OS read. The CLI's entry exits the process;
    // all owned daemon cleanup has already completed before this shutdown.
    runtime.shutdown_background();
    match result {
        Ok(code) => code,
        Err(error) => failure(error),
    }
}

struct SessionCli {
    input: tokio::io::Stdin,
    output: tokio::io::Stdout,
    chunk: [u8; 8192],
    line: Vec<u8>,
    line_started: Option<Instant>,
    output_frame: Option<Vec<u8>>,
    output_cursor: usize,
    next_output: Option<Vec<u8>>,
    deferred: Option<(search_provider_protocol::SessionClientFrame, Instant)>,
    input_closed: bool,
    ending: bool,
    output_deadline: Option<Instant>,
    deadline: Instant,
    open_seen: bool,
    exit_code: i32,
}

impl SessionCli {
    fn new(deadline: Instant) -> Self {
        Self {
            input: tokio::io::stdin(),
            output: tokio::io::stdout(),
            chunk: [0; 8192],
            line: Vec::new(),
            line_started: None,
            output_frame: None,
            output_cursor: 0,
            next_output: None,
            deferred: None,
            input_closed: false,
            ending: false,
            output_deadline: None,
            deadline,
            open_seen: false,
            exit_code: 0,
        }
    }

    fn submit_line(
        &mut self,
        session: &mut search_provider_client::OwnedQuerySession,
    ) -> Result<(), ClientError> {
        use search_provider_protocol::SessionClientFrame;
        let mut frame: SessionClientFrame = serde_json::from_slice(&self.line)
            .map_err(|_| search_provider_protocol::ProtocolError::InvalidMessage)?;
        if let SessionClientFrame::Open {
            include_summary, ..
        } = &mut frame
        {
            *include_summary = true;
        }
        let started = self.line_started.take().ok_or(ClientError::Io)?;
        self.line.clear();
        if let Err(error) = frame.validate() {
            return self.local_rejection(frame.request_id(), error.code());
        }
        let deadline = match &frame {
            SessionClientFrame::Open { remaining_ms, .. }
            | SessionClientFrame::Continue { remaining_ms, .. }
            | SessionClientFrame::Expand { remaining_ms, .. } => started
                .checked_add(std::time::Duration::from_millis(*remaining_ms))
                .ok_or(ClientError::Deadline)?,
            _ => self.deadline,
        };
        let data = matches!(
            frame,
            SessionClientFrame::Open { .. }
                | SessionClientFrame::Continue { .. }
                | SessionClientFrame::Expand { .. }
        );
        if data && (session.has_active_request() || self.output_frame.is_some()) {
            if self.deferred.is_some() {
                return Err(search_provider_protocol::ProtocolError::InvalidMessage.into());
            }
            self.deferred = Some((frame, deadline));
            return Ok(());
        }
        self.submit_frame(session, frame, deadline)
    }

    fn submit_frame(
        &mut self,
        session: &mut search_provider_client::OwnedQuerySession,
        frame: search_provider_protocol::SessionClientFrame,
        deadline: Instant,
    ) -> Result<(), ClientError> {
        use search_provider_protocol::SessionClientFrame;
        let ttl = if let SessionClientFrame::Open { ttl_ms, .. } = &frame {
            Some(*ttl_ms)
        } else {
            None
        };
        let close = matches!(frame, SessionClientFrame::Close { .. });
        match session.submit(frame, deadline) {
            Ok(()) => {}
            Err(ClientError::CommandRejected { request_id, code }) => {
                return self.local_rejection(request_id, code);
            }
            Err(error) => return Err(error),
        }
        if let Some(ttl) = ttl
            && !self.open_seen
        {
            self.open_seen = true;
            self.deadline = Instant::now()
                .checked_add(std::time::Duration::from_millis(ttl))
                .and_then(|end| end.checked_add(std::time::Duration::from_secs(2)))
                .ok_or(ClientError::Deadline)?;
        }
        if close {
            self.input_closed = true;
            self.deferred = None;
        }
        Ok(())
    }

    fn input_bytes(
        &mut self,
        count: usize,
        session: &mut search_provider_client::OwnedQuerySession,
    ) -> Result<(), ClientError> {
        for index in 0..count {
            if self.input_closed {
                break;
            }
            let byte = self.chunk[index];
            self.line_started.get_or_insert_with(Instant::now);
            if byte == b'\n' {
                self.submit_line(session)?;
            } else {
                if self.line.len()
                    >= search_provider_protocol::MAX_CLIENT_FRAME_BYTES
                        - search_provider_protocol::FRAME_PREFIX_BYTES
                {
                    return Err(search_provider_protocol::ProtocolError::FrameTooLarge.into());
                }
                self.line
                    .try_reserve(1)
                    .map_err(|_| ClientError::Allocation)?;
                self.line.push(byte);
            }
        }
        Ok(())
    }

    fn received(
        &mut self,
        reply: &search_provider_protocol::SessionReply,
    ) -> Result<(), ClientError> {
        use search_provider_protocol::SessionOutcome;
        match &reply.outcome {
            SessionOutcome::Closed {} | SessionOutcome::Expired {} => {
                self.input_closed = true;
                self.ending = true;
                self.deferred = None;
                self.output_deadline =
                    Instant::now().checked_add(std::time::Duration::from_secs(2));
            }
            SessionOutcome::Failed { .. } => self.exit_code = 2,
            SessionOutcome::Cancelled {} if self.exit_code == 0 => self.exit_code = 130,
            _ => {}
        }
        let bytes = serde_json::to_vec(reply)
            .map_err(|_| search_provider_protocol::ProtocolError::Serialization)?;
        self.queue_output(bytes)
    }

    fn local_rejection(&mut self, request_id: u64, code: &'static str) -> Result<(), ClientError> {
        // Explicit local origin: this record is never claimed as a daemon reply.
        let reply = serde_json::json!({
            "origin": "client",
            "request_id": request_id,
            "outcome": search_provider_protocol::SessionOutcome::Failed {
                code: search_provider_protocol::FailureCode::new(code)?,
            },
        });
        let bytes = serde_json::to_vec(&reply)
            .map_err(|_| search_provider_protocol::ProtocolError::Serialization)?;
        self.exit_code = 2;
        self.queue_output(bytes)
    }

    fn queue_output(&mut self, mut bytes: Vec<u8>) -> Result<(), ClientError> {
        bytes.push(b'\n');
        if self.output_frame.is_none() {
            self.output_frame = Some(bytes);
            self.output_cursor = 0;
        } else if self.next_output.replace(bytes).is_some() {
            return Err(search_provider_protocol::ProtocolError::InvalidMessage.into());
        }
        Ok(())
    }

    async fn end(
        &mut self,
        session: &mut search_provider_client::OwnedQuerySession,
    ) -> Result<(), ClientError> {
        self.input_closed = true;
        self.ending = true;
        self.deferred = None;
        let completion = session.finish(self.deadline).await?;
        if !completion.status.success() && self.exit_code == 0 {
            return Err(ClientError::ChildExit);
        }
        self.output_deadline = Instant::now().checked_add(std::time::Duration::from_secs(2));
        Ok(())
    }

    async fn drive(
        &mut self,
        session: &mut search_provider_client::OwnedQuerySession,
        cancelled: &AtomicBool,
    ) -> Result<i32, ClientError> {
        use std::sync::atomic::Ordering;
        use tokio::io::AsyncReadExt;
        let signal = tokio::signal::ctrl_c();
        tokio::pin!(signal);
        let mut signal_seen = false;
        loop {
            if self.ending && self.output_frame.is_none() && self.next_output.is_none() {
                return Ok(self.exit_code);
            }
            if !self.ending
                && self.output_frame.is_none()
                && !session.has_active_request()
                && let Some((frame, deadline)) = self.deferred.take()
            {
                self.submit_frame(session, frame, deadline)?;
            }
            let receive = !session.is_finished() && self.next_output.is_none();
            let deadline = self.output_deadline.unwrap_or(self.deadline);
            tokio::select! {
                read = self.input.read(&mut self.chunk), if !self.input_closed => {
                    let n = read.map_err(|_| ClientError::Io)?;
                    if n == 0 {
                        if !self.line.is_empty() { self.submit_line(session)?; }
                        self.end(session).await?;
                    } else { self.input_bytes(n, session)?; }
                }
                reply = session.next_reply(self.deadline, cancelled), if receive => {
                    let reply = reply?;
                    self.received(&reply)?;
                    if cancelled.load(Ordering::Acquire) {
                        self.exit_code = 130;
                        if !session.is_finished() { self.end(session).await?; }
                    }
                }
                step = session_output_step(&mut self.output, self.output_frame.as_deref(), self.output_cursor), if self.output_frame.is_some() => {
                    match step.map_err(|_| ClientError::Io)? {
                        SessionOutput::Wrote(0) => return Err(ClientError::Io),
                        SessionOutput::Wrote(n) => self.output_cursor += n,
                        SessionOutput::Flushed => {
                            self.output_frame = self.next_output.take();
                            self.output_cursor = 0;
                        }
                    }
                }
                event = &mut signal, if !signal_seen => {
                    event.map_err(|_| ClientError::Signal)?;
                    signal_seen = true;
                    self.exit_code = 130;
                    self.input_closed = true;
                    cancelled.store(true, Ordering::Release);
                    if !session.has_active_request() { self.end(session).await?; }
                }
                () = tokio::time::sleep_until(deadline.into()) => return Err(ClientError::Deadline),
            }
        }
    }
}

enum SessionOutput {
    Wrote(usize),
    Flushed,
}

async fn session_output_step(
    stdout: &mut tokio::io::Stdout,
    frame: Option<&[u8]>,
    cursor: usize,
) -> std::io::Result<SessionOutput> {
    use tokio::io::AsyncWriteExt;
    let Some(frame) = frame else {
        return std::future::pending().await;
    };
    if cursor < frame.len() {
        stdout
            .write(&frame[cursor..])
            .await
            .map(SessionOutput::Wrote)
    } else {
        stdout.flush().await?;
        Ok(SessionOutput::Flushed)
    }
}

async fn session_stdio(
    daemon: &Path,
    arguments: &[OsString],
    deadline: Instant,
) -> Result<i32, ClientError> {
    let cancelled = AtomicBool::new(false);
    let mut io = SessionCli::new(deadline);
    // The session listener is installed before the owned child starts.
    let mut session = search_provider_client::OwnedQuerySession::open(
        daemon, arguments, deadline, &cancelled, true,
    )
    .await?;
    let result = io.drive(&mut session, &cancelled).await;
    let completion = session.finish(io.deadline).await?;
    match result {
        Ok(0) if !completion.status.success() => Err(ClientError::ChildExit),
        other => other,
    }
}
