//! One concrete child and its serviced stdio; no detached timeout tasks.

use search_provider_protocol::{
    ClientFrame, DecodedUtf8Request, FrameBuffer, MAX_FRAME_BYTES, MAX_REMAINING_MS,
    QUERY_REQUEST_ID, QueryOutcome, RawExactRequest, SearchRequest, Terminal,
    decode_terminal_frame, encode_client_frame,
};
use std::ffi::OsString;
use std::future::{Future, poll_fn};
use std::io;
use std::path::Path;
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::time::{Instant as TokioInstant, MissedTickBehavior};

use crate::{ClientError, QueryCompletion};

const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024;
const CONTROL_POLL: Duration = Duration::from_millis(10);
const CLEANUP_ALLOWANCE: Duration = Duration::from_secs(2);

pub enum Request {
    Search(SearchRequest),
    RawExact(RawExactRequest),
    DecodedUtf8(DecodedUtf8Request),
}

pub async fn query(
    executable: &Path,
    args: &[OsString],
    request: Request,
    deadline: Instant,
    cancelled: &AtomicBool,
    cancel_on_ctrl_c: bool,
) -> Result<QueryCompletion, ClientError> {
    checkpoint(deadline, cancelled)?;
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    if cancel_on_ctrl_c {
        // Tokio installs the listener on its first poll. Keep this exact pinned
        // future after registration, before starting any owned child.
        let early = poll_fn(|cx| {
            Poll::Ready(match ctrl_c.as_mut().poll(cx) {
                Poll::Ready(event) => Some(event),
                Poll::Pending => None,
            })
        })
        .await;
        if let Some(event) = early {
            event.map_err(|_| ClientError::Signal)?;
            cancelled.store(true, Ordering::Release);
        }
    }
    checkpoint(deadline, cancelled)?;
    let mut command = owned_command(executable, args, cancel_on_ctrl_c);
    let mut child = command.spawn().map_err(|_| ClientError::Start)?;
    let result = async {
        let stdin = child.stdin.take().ok_or(ClientError::Pipe)?;
        let stdout = child.stdout.take().ok_or(ClientError::Pipe)?;
        let stderr = child.stderr.take().ok_or(ClientError::Pipe)?;
        exchange(
            &mut child,
            stdin,
            stdout,
            stderr,
            request,
            deadline,
            cancelled,
            ctrl_c.as_mut(),
            cancel_on_ctrl_c,
        )
        .await
    }
    .await;
    // Success already observed child exit and both output EOFs in exchange.
    // Failures still require explicit termination/reap; kill_on_drop is only
    // an unwind fallback, never evidence of OS-process exit.
    if result.is_err() {
        terminate_and_reap(&mut child).await?;
    }
    result
}

fn checkpoint(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ClientError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(ClientError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ClientError::Deadline);
    }
    Ok(())
}

pub async fn terminate_and_reap(child: &mut Child) -> Result<(), ClientError> {
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) | Err(_) => {
            // A racing exit can make start_kill fail. The following wait is the
            // actual observation and remains authoritative.
            let _ = child.start_kill();
        }
    }
    child.wait().await.map_err(|_| ClientError::Wait)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum Stop {
    Cancelled,
    Deadline,
}

impl Stop {
    const fn error(self) -> ClientError {
        match self {
            Self::Cancelled => ClientError::Cancelled,
            Self::Deadline => ClientError::Deadline,
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn exchange(
    child: &mut Child,
    stdin: ChildStdin,
    mut stdout: ChildStdout,
    mut stderr: ChildStderr,
    request: Request,
    deadline: Instant,
    cancelled: &AtomicBool,
    mut ctrl_c: Pin<&mut impl Future<Output = io::Result<()>>>,
    cancel_on_ctrl_c: bool,
) -> Result<QueryCompletion, ClientError> {
    let (mut query, requested_limit) = match request {
        Request::Search(request) => {
            let limit = request.limit;
            (
                ClientFrame::Query {
                    request_id: QUERY_REQUEST_ID,
                    remaining_ms: 1,
                    request,
                },
                limit,
            )
        }
        Request::RawExact(request) => {
            let limit = request.limit;
            (
                ClientFrame::RawExact {
                    request_id: QUERY_REQUEST_ID,
                    remaining_ms: 1,
                    request,
                },
                limit,
            )
        }
        Request::DecodedUtf8(request) => {
            let limit = request.limit;
            (
                ClientFrame::DecodedUtf8 {
                    request_id: QUERY_REQUEST_ID,
                    remaining_ms: 1,
                    request,
                },
                limit,
            )
        }
    };
    let mut stdin = Some(stdin);
    let mut incoming = FrameBuffer::new(MAX_FRAME_BYTES)?;
    let mut outbound = Vec::new();
    let mut written = 0;
    let mut query_sent = false;
    let mut cancel_prepared = false;
    let mut terminal: Option<Terminal> = None;
    let mut child_status: Option<ExitStatus> = None;
    let mut stdout_eof = false;
    let mut stderr_eof = false;
    let mut diagnostics = Vec::new();
    let mut diagnostics_truncated = false;
    let mut stopped = None;
    let mut cleanup_deadline: Option<Instant> = None;
    let mut stdout_chunk = io_buffer()?;
    let mut stderr_chunk = io_buffer()?;
    let mut signal_seen = false;
    let mut controls = tokio::time::interval(CONTROL_POLL);
    controls.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        let now = Instant::now();
        if now >= deadline && !matches!(stopped, Some(Stop::Deadline)) {
            if !query_sent || terminal.is_some() {
                return Err(ClientError::Deadline);
            }
            stopped = Some(Stop::Deadline);
            cleanup_deadline.get_or_insert_with(|| cleanup_end(now));
        }
        if stopped.is_none() && terminal.is_none() && cancelled.load(Ordering::Acquire) {
            // A partial query frame cannot contain Cancel. Closing this entire
            // connection and reaping its child is the safe cancellation path.
            if !query_sent {
                return Err(ClientError::Cancelled);
            }
            stopped = Some(Stop::Cancelled);
            cleanup_deadline.get_or_insert_with(|| cleanup_end(now));
        }
        if cleanup_deadline.is_some_and(|end| now >= end) {
            return Err(stopped.map_or(ClientError::Cleanup, Stop::error));
        }
        if let Some(status) = child_status.as_ref().filter(|_| stdout_eof && stderr_eof) {
            let terminal = terminal.ok_or(ClientError::Truncated)?;
            if matches!(stopped, Some(Stop::Deadline)) {
                return Err(ClientError::Deadline);
            }
            if matches!(
                terminal.outcome,
                QueryOutcome::Complete { .. }
                    | QueryOutcome::RawExact { .. }
                    | QueryOutcome::DecodedUtf8 { .. }
            ) && !status.success()
            {
                return Err(ClientError::ChildExit);
            }
            return Ok(QueryCompletion {
                terminal,
                diagnostics,
                diagnostics_truncated,
            });
        }
        if stopped.is_some() && !cancel_prepared && terminal.is_none() && stdin.is_some() {
            // Query has completed its local write before any control bytes.
            outbound = encode_client_frame(&ClientFrame::Cancel {
                request_id: QUERY_REQUEST_ID,
            })?;
            written = 0;
            cancel_prepared = true;
        }
        if !query_sent && written == 0 && stopped.is_none() && stdin.is_some() {
            // Re-sample only while no query byte has been submitted. Once a
            // write advances, preserve exactly that frame and byte cursor.
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(ClientError::Deadline)?;
            let remaining_ms = u64::try_from(remaining.as_millis())
                .unwrap_or(u64::MAX)
                .min(MAX_REMAINING_MS);
            if remaining_ms == 0 {
                return Err(ClientError::Deadline);
            }
            if let ClientFrame::Query {
                remaining_ms: wire, ..
            }
            | ClientFrame::RawExact {
                remaining_ms: wire, ..
            }
            | ClientFrame::DecodedUtf8 {
                remaining_ms: wire, ..
            } = &mut query
            {
                *wire = remaining_ms;
            }
            outbound = encode_client_frame(&query)?;
        }
        let next_deadline = if stopped.is_some() {
            cleanup_deadline.unwrap_or(deadline)
        } else {
            cleanup_deadline.map_or(deadline, |end| end.min(deadline))
        };

        tokio::select! {
            read = stdout.read(&mut stdout_chunk), if !stdout_eof => {
                let count = read.map_err(|_| ClientError::Io)?;
                if count == 0 {
                    stdout_eof = true;
                    if terminal.is_none() { return Err(ClientError::Truncated); }
                } else {
                    if terminal.is_some() { return Err(ClientError::TrailingOutput); }
                    let consumed = incoming.push(&stdout_chunk[..count])?;
                    if consumed != count { return Err(ClientError::TrailingOutput); }
                    if let Some(frame) = incoming.frame() {
                        if !query_sent { return Err(ClientError::Protocol(search_provider_protocol::ProtocolError::InvalidMessage)); }
                        let decoded = decode_terminal_frame(frame, requested_limit)?;
                        match (&query, &decoded.outcome) {
                            (ClientFrame::Query { .. }, QueryOutcome::Complete { .. })
                            | (_, QueryOutcome::Cancelled | QueryOutcome::Failed { .. }) => {}
                            (ClientFrame::RawExact { request, .. }, QueryOutcome::RawExact { result }) => {
                                result.validate_request(request)?;
                            }
                            (ClientFrame::DecodedUtf8 { request, .. }, QueryOutcome::DecodedUtf8 { result }) => {
                                result.validate_request(request)?;
                            }
                            _ => return Err(ClientError::Protocol(search_provider_protocol::ProtocolError::InvalidMessage)),
                        }
                        terminal = Some(decoded);
                        // EOF releases the daemon's one process-scoped input
                        // reader before we wait for its owned worker to join.
                        drop(stdin.take());
                        cleanup_deadline.get_or_insert_with(|| cleanup_end(Instant::now()));
                    }
                }
            }
            read = stderr.read(&mut stderr_chunk), if !stderr_eof => {
                let count = read.map_err(|_| ClientError::Io)?;
                if count == 0 {
                    stderr_eof = true;
                } else {
                    let keep = count.min(MAX_DIAGNOSTIC_BYTES - diagnostics.len());
                    diagnostics.try_reserve(keep).map_err(|_| ClientError::Allocation)?;
                    diagnostics.extend_from_slice(&stderr_chunk[..keep]);
                    diagnostics_truncated |= keep != count;
                    // Excess bytes are deliberately drained, preventing stderr
                    // backpressure from blocking terminal output or child exit.
                }
            }
            result = write_pending(&mut stdin, &outbound[written..]),
                if stdin.is_some() && written < outbound.len() && terminal.is_none() => {
                let count = result.map_err(|_| ClientError::Io)?;
                if count == 0 { return Err(ClientError::Io); }
                written += count;
                if written == outbound.len() && !query_sent {
                    query_sent = true;
                    outbound.clear();
                    written = 0;
                }
            }
            status = child.wait(), if child_status.is_none() => {
                child_status = Some(status.map_err(|_| ClientError::Wait)?);
                drop(stdin.take());
                cleanup_deadline.get_or_insert_with(|| cleanup_end(Instant::now()));
            }
            _ = controls.tick() => {}
            signal = &mut ctrl_c, if cancel_on_ctrl_c && !signal_seen => {
                signal.map_err(|_| ClientError::Signal)?;
                signal_seen = true;
                cancelled.store(true, Ordering::Release);
            }
            () = tokio::time::sleep_until(TokioInstant::from_std(next_deadline)) => {}
        }
    }
}

fn io_buffer() -> Result<Box<[u8]>, ClientError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(8192)
        .map_err(|_| ClientError::Allocation)?;
    bytes.resize(8192, 0_u8);
    Ok(bytes.into_boxed_slice())
}

async fn write_pending(stdin: &mut Option<ChildStdin>, bytes: &[u8]) -> io::Result<usize> {
    match stdin.as_mut() {
        Some(stream) => stream.write(bytes).await,
        None => Err(io::Error::other("owned stdin closed")),
    }
}

fn cleanup_end(now: Instant) -> Instant {
    now.checked_add(CLEANUP_ALLOWANCE).unwrap_or(now)
}

fn owned_command(executable: &Path, args: &[OsString], cancel_on_ctrl_c: bool) -> Command {
    let mut command = Command::new(executable);
    if cancel_on_ctrl_c {
        // The parent translates terminal Ctrl-C into the correlated wire Cancel.
        #[cfg(unix)]
        command.process_group(0);
        #[cfg(windows)]
        command.creation_flags(0x0000_0200); // CREATE_NEW_PROCESS_GROUP
    }
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}

use search_provider_protocol::{
    SessionClientFrame, SessionOutcome, SessionReply, WindowEntry, WindowQuery, WindowSummary,
    decode_session_reply, encode_session_frame,
};

struct PendingSessionRequest {
    message: SessionClientFrame,
    deadline: Instant,
    sent: bool,
}

/// Concrete state of the one session child and its original emitted prefix.
pub struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    incoming: FrameBuffer,
    stdout_chunk: Box<[u8]>,
    stderr_chunk: Box<[u8]>,
    read_used: usize,
    read_cursor: usize,
    outbound: Vec<u8>,
    written: usize,
    writing_control: Option<SessionClientFrame>,
    queued_control: Option<SessionClientFrame>,
    active: Option<PendingSessionRequest>,
    closing: Option<PendingSessionRequest>,
    cancelled_request: Option<u64>,
    last_id: u64,
    query: Option<WindowQuery>,
    summary: Option<WindowSummary>,
    emitted: Vec<WindowEntry>,
    lifetime_end: Option<Instant>,
    exit_deadline: Option<Instant>,
    child_status: Option<ExitStatus>,
    stdout_eof: bool,
    stderr_eof: bool,
    diagnostics: Vec<u8>,
    diagnostics_truncated: bool,
    ctrl_c: Option<Pin<Box<dyn Future<Output = io::Result<()>> + Send>>>,
    controls: tokio::time::Interval,
}

impl Session {
    pub(super) async fn open(
        executable: &Path,
        args: &[OsString],
        deadline: Instant,
        cancelled: &AtomicBool,
        cancel_on_ctrl_c: bool,
    ) -> Result<Self, ClientError> {
        checkpoint(deadline, cancelled)?;
        let mut ctrl_c: Pin<Box<dyn Future<Output = io::Result<()>> + Send>> =
            Box::pin(tokio::signal::ctrl_c());
        if cancel_on_ctrl_c {
            let early = poll_fn(|cx| {
                Poll::Ready(match ctrl_c.as_mut().poll(cx) {
                    Poll::Ready(result) => Some(result),
                    Poll::Pending => None,
                })
            })
            .await;
            if let Some(result) = early {
                result.map_err(|_| ClientError::Signal)?;
                cancelled.store(true, Ordering::Release);
            }
        }
        checkpoint(deadline, cancelled)?;
        let incoming = FrameBuffer::new(MAX_FRAME_BYTES)?;
        let stdout_chunk = io_buffer()?;
        let stderr_chunk = io_buffer()?;
        // Session controls own cancellation even when its caller services the OS signal.
        let mut child = owned_command(executable, args, true)
            .spawn()
            .map_err(|_| ClientError::Start)?;
        let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(stderr)) = pipes else {
            terminate_and_reap(&mut child).await?;
            return Err(ClientError::Pipe);
        };
        let mut controls = tokio::time::interval(CONTROL_POLL);
        controls.set_missed_tick_behavior(MissedTickBehavior::Skip);
        Ok(Self {
            child,
            stdin: Some(stdin),
            stdout,
            stderr,
            incoming,
            stdout_chunk,
            stderr_chunk,
            read_used: 0,
            read_cursor: 0,
            outbound: Vec::new(),
            written: 0,
            writing_control: None,
            queued_control: None,
            active: None,
            closing: None,
            cancelled_request: None,
            last_id: 0,
            query: None,
            summary: None,
            emitted: Vec::new(),
            lifetime_end: None,
            exit_deadline: None,
            child_status: None,
            stdout_eof: false,
            stderr_eof: false,
            diagnostics: Vec::new(),
            diagnostics_truncated: false,
            ctrl_c: cancel_on_ctrl_c.then_some(ctrl_c),
            controls,
        })
    }

    pub(super) const fn active(&self) -> bool {
        self.active.is_some()
    }

    pub const fn is_finished(&self) -> bool {
        self.child_status.is_some() && self.stdout_eof && self.stderr_eof
    }

    pub(super) fn submit(
        &mut self,
        message: SessionClientFrame,
        deadline: Instant,
    ) -> Result<(), ClientError> {
        message.validate()?;
        if self.is_finished() || self.stdin.is_none() || Instant::now() >= deadline {
            return Err(ClientError::Deadline);
        }
        match &message {
            SessionClientFrame::Cancel { request_id } => {
                if self.active.as_ref().map(|p| p.message.request_id()) != Some(*request_id)
                    || self.closing.is_some()
                    || self.cancelled_request == Some(*request_id)
                {
                    return Err(search_provider_protocol::ProtocolError::InvalidCorrelation.into());
                }
                self.cancelled_request = Some(*request_id);
                self.queued_control = Some(message);
                return Ok(());
            }
            SessionClientFrame::Close { request_id } => {
                if *request_id <= self.last_id || self.closing.is_some() {
                    return Err(search_provider_protocol::ProtocolError::InvalidCorrelation.into());
                }
                self.last_id = *request_id;
                self.closing = Some(PendingSessionRequest {
                    message: message.clone(),
                    deadline,
                    sent: false,
                });
                // Replacing an unsent Cancel is safe; a partial frame remains in outbound.
                self.queued_control = Some(message);
                return Ok(());
            }
            _ => {}
        }
        if self.active.is_some() || self.closing.is_some() || message.request_id() <= self.last_id {
            return Err(search_provider_protocol::ProtocolError::InvalidCorrelation.into());
        }
        match &message {
            SessionClientFrame::Open { query, ttl_ms, .. } if self.query.is_none() => {
                self.query = Some(query.clone());
                self.lifetime_end = Some(
                    Instant::now()
                        .checked_add(Duration::from_millis(*ttl_ms))
                        .ok_or(ClientError::Deadline)?,
                );
                self.emitted
                    .try_reserve(usize::from(query.limit()))
                    .map_err(|_| ClientError::Allocation)?;
            }
            SessionClientFrame::Continue { .. } if self.summary.is_some() => {}
            SessionClientFrame::Expand {
                item_index,
                before_bytes,
                after_bytes,
                ..
            } if self.summary.is_some() => {
                let item = self
                    .emitted
                    .get(usize::from(*item_index))
                    .ok_or(search_provider_protocol::ProtocolError::InvalidMessage)?;
                let (start, end) = match &item.item {
                    search_provider_protocol::WindowItem::Search(
                        search_provider_protocol::SearchItem::Hit(h),
                    ) => (h.evidence.start, h.evidence.end),
                    search_provider_protocol::WindowItem::Raw(
                        search_provider_protocol::RawExactItem::Match(h),
                    ) => (h.start, h.end),
                    search_provider_protocol::WindowItem::DecodedUtf8(
                        search_provider_protocol::DecodedUtf8Item::Match(h),
                    ) => (h.representation_start, h.representation_end),
                    _ => return Err(search_provider_protocol::ProtocolError::InvalidMessage.into()),
                };
                if end
                    .checked_sub(start)
                    .and_then(|n| n.checked_add(*before_bytes))
                    .and_then(|n| n.checked_add(*after_bytes))
                    .is_none_or(|n| n > search_provider_protocol::MAX_EXPANSION_BYTES)
                {
                    return Err(search_provider_protocol::ProtocolError::InvalidMessage.into());
                }
            }
            _ => return Err(search_provider_protocol::ProtocolError::InvalidMessage.into()),
        }
        let remaining_ms = match &message {
            SessionClientFrame::Open { remaining_ms, .. }
            | SessionClientFrame::Continue { remaining_ms, .. }
            | SessionClientFrame::Expand { remaining_ms, .. } => *remaining_ms,
            _ => return Err(search_provider_protocol::ProtocolError::InvalidMessage.into()),
        };
        let sampled = Instant::now()
            .checked_add(Duration::from_millis(remaining_ms))
            .ok_or(ClientError::Deadline)?;
        self.last_id = message.request_id();
        self.cancelled_request = None;
        self.active = Some(PendingSessionRequest {
            message,
            deadline: deadline.min(sampled),
            sent: false,
        });
        Ok(())
    }

    fn prepare_write(&mut self) -> Result<(), ClientError> {
        if !self.outbound.is_empty() && (self.written != 0 || self.writing_control.is_some()) {
            return Ok(());
        }
        self.written = 0;
        if let Some(pending) = self.active.as_mut().filter(|p| !p.sent) {
            let remaining = pending
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or(ClientError::Deadline)?;
            let ms = u64::try_from(remaining.as_millis())
                .unwrap_or(u64::MAX)
                .min(MAX_REMAINING_MS);
            if ms == 0 {
                return Err(ClientError::Deadline);
            }
            match &mut pending.message {
                SessionClientFrame::Open { remaining_ms, .. }
                | SessionClientFrame::Continue { remaining_ms, .. }
                | SessionClientFrame::Expand { remaining_ms, .. } => *remaining_ms = ms,
                _ => return Err(search_provider_protocol::ProtocolError::InvalidMessage.into()),
            }
            self.outbound = encode_session_frame(&pending.message)?;
        } else if self.outbound.is_empty()
            && let Some(control) = self.queued_control.take()
        {
            self.outbound = encode_session_frame(&control)?;
            self.writing_control = Some(control);
        }
        Ok(())
    }

    fn wrote(&mut self, count: usize) -> Result<(), ClientError> {
        if count == 0 {
            return Err(ClientError::Io);
        }
        self.written += count;
        if self.written == self.outbound.len() {
            if let Some(control) = self.writing_control.take() {
                if matches!(control, SessionClientFrame::Close { .. })
                    && let Some(closing) = &mut self.closing
                {
                    closing.sent = true;
                }
            } else if let Some(pending) = &mut self.active {
                pending.sent = true;
            }
            self.outbound.clear();
            self.written = 0;
        }
        Ok(())
    }

    fn accept_reply(&mut self, reply: &SessionReply) -> Result<(), ClientError> {
        let id = reply.request_id;
        match &reply.outcome {
            SessionOutcome::Closed {}
                if self
                    .closing
                    .as_ref()
                    .is_some_and(|closing| closing.sent && closing.message.request_id() == id) =>
            {
                self.active = None;
                return Ok(());
            }
            SessionOutcome::Expired {}
                if self.query.is_some()
                    && (id == 0
                        || self
                            .active
                            .as_ref()
                            .is_some_and(|p| p.sent && p.message.request_id() == id)
                        || self.closing.as_ref().is_some_and(|closing| {
                            closing.sent && closing.message.request_id() == id
                        })) =>
            {
                self.active = None;
                return Ok(());
            }
            _ => {}
        }
        let pending = self
            .active
            .as_ref()
            .filter(|p| p.sent && p.message.request_id() == id)
            .ok_or(search_provider_protocol::ProtocolError::InvalidCorrelation)?;
        match (&pending.message, &reply.outcome) {
            (
                SessionClientFrame::Open { page_size, .. }
                | SessionClientFrame::Continue { page_size, .. },
                SessionOutcome::Page { page },
            ) => {
                if self.summary.as_ref().is_some_and(|s| s != &page.summary) {
                    return Err(search_provider_protocol::ProtocolError::InvalidMessage.into());
                }
                page.validate_request(
                    self.query
                        .as_ref()
                        .ok_or(search_provider_protocol::ProtocolError::InvalidMessage)?,
                    &self.emitted,
                    *page_size,
                )?;
                self.summary = Some(page.summary.clone());
                self.emitted.extend(page.items.iter().cloned());
            }
            (
                SessionClientFrame::Expand { item_index, .. },
                SessionOutcome::Expanded { expansion },
            ) => {
                let item = self
                    .emitted
                    .get(usize::from(*item_index))
                    .ok_or(search_provider_protocol::ProtocolError::InvalidMessage)?;
                expansion.validate_request(&pending.message, &item.item)?;
            }
            (_, SessionOutcome::Failed { .. } | SessionOutcome::Cancelled {}) => {}
            _ => return Err(search_provider_protocol::ProtocolError::InvalidMessage.into()),
        }
        self.active = None;
        if matches!(self.queued_control, Some(SessionClientFrame::Cancel { .. })) {
            self.queued_control = None;
        }
        if self.written == 0
            && matches!(
                self.writing_control,
                Some(SessionClientFrame::Cancel { .. })
            )
        {
            self.writing_control = None;
            self.outbound.clear();
        }
        Ok(())
    }

    fn buffered_reply(&mut self) -> Result<Option<SessionReply>, ClientError> {
        if self.read_cursor == self.read_used {
            return Ok(None);
        }
        let consumed = self
            .incoming
            .push(&self.stdout_chunk[self.read_cursor..self.read_used])?;
        self.read_cursor += consumed;
        let Some(frame) = self.incoming.frame() else {
            return Ok(None);
        };
        let reply = decode_session_reply(frame)?;
        self.accept_reply(&reply)?;
        self.incoming = FrameBuffer::new(MAX_FRAME_BYTES)?;
        Ok(Some(reply))
    }

    fn retain_stderr(&mut self, count: usize) -> Result<(), ClientError> {
        if count == 0 {
            self.stderr_eof = true;
            return Ok(());
        }
        let keep = count.min(MAX_DIAGNOSTIC_BYTES - self.diagnostics.len());
        self.diagnostics
            .try_reserve(keep)
            .map_err(|_| ClientError::Allocation)?;
        self.diagnostics
            .extend_from_slice(&self.stderr_chunk[..keep]);
        self.diagnostics_truncated |= keep != count;
        Ok(())
    }

    pub(super) async fn receive(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SessionReply, ClientError> {
        loop {
            if let Some(reply) = self.buffered_reply()? {
                return Ok(reply);
            }
            if self.stdout_eof {
                return Err(ClientError::Truncated);
            }
            let mut end = self
                .active
                .as_ref()
                .map_or(deadline, |p| p.deadline.min(deadline));
            if let Some(expiry) = self.lifetime_end {
                end = end.min(cleanup_end(expiry));
            }
            if let Some(exit) = self.exit_deadline {
                end = end.min(exit);
            }
            if Instant::now() >= end {
                return Err(ClientError::Deadline);
            }
            if cancelled.load(Ordering::Acquire) && self.cancelled_request.is_none() {
                let pending = self
                    .active
                    .as_ref()
                    .filter(|p| p.sent)
                    .ok_or(ClientError::Cancelled)?;
                let id = pending.message.request_id();
                self.submit(SessionClientFrame::Cancel { request_id: id }, end)?;
            }
            self.prepare_write()?;
            tokio::select! {
                read = self.stdout.read(&mut self.stdout_chunk), if self.read_cursor == self.read_used => {
                    self.read_used = read.map_err(|_| ClientError::Io)?;
                    self.read_cursor = 0;
                    self.stdout_eof = self.read_used == 0;
                }
                read = self.stderr.read(&mut self.stderr_chunk), if !self.stderr_eof => {
                    self.retain_stderr(read.map_err(|_| ClientError::Io)?)?;
                }
                wrote = write_pending(&mut self.stdin, &self.outbound[self.written..]),
                    if self.stdin.is_some() && self.written < self.outbound.len() => {
                    self.wrote(wrote.map_err(|_| ClientError::Io)?)?;
                }
                status = self.child.wait(), if self.child_status.is_none() => {
                    self.child_status = Some(status.map_err(|_| ClientError::Wait)?);
                    self.exit_deadline = Some(cleanup_end(Instant::now()));
                }
                event = async {
                    match &mut self.ctrl_c {
                        Some(signal) => signal.await,
                        None => std::future::pending().await,
                    }
                } => {
                    event.map_err(|_| ClientError::Signal)?;
                    self.ctrl_c = None;
                    cancelled.store(true, Ordering::Release);
                }
                _ = self.controls.tick() => {}
                () = tokio::time::sleep_until(TokioInstant::from_std(end)) => return Err(ClientError::Deadline),
            }
        }
    }

    async fn drain(&mut self, strict: bool, end: Instant) -> Result<(), ClientError> {
        let mut trailing =
            strict && (self.read_cursor < self.read_used || !self.incoming.is_empty());
        self.read_cursor = self.read_used;
        while self.child_status.is_none() || !self.stdout_eof || !self.stderr_eof {
            tokio::select! {
                read = self.stdout.read(&mut self.stdout_chunk), if !self.stdout_eof => {
                    let n = read.map_err(|_| ClientError::Io)?;
                    self.stdout_eof = n == 0;
                    trailing |= strict && n != 0;
                }
                read = self.stderr.read(&mut self.stderr_chunk), if !self.stderr_eof => {
                    self.retain_stderr(read.map_err(|_| ClientError::Io)?)?;
                }
                status = self.child.wait(), if self.child_status.is_none() => {
                    self.child_status = Some(status.map_err(|_| ClientError::Wait)?);
                }
                () = tokio::time::sleep_until(TokioInstant::from_std(end)) => return Err(ClientError::Cleanup),
            }
        }
        if trailing {
            Err(ClientError::TrailingOutput)
        } else {
            Ok(())
        }
    }

    pub(super) async fn finish(
        &mut self,
        deadline: Instant,
        strict: bool,
    ) -> Result<crate::SessionCompletion, ClientError> {
        drop(self.stdin.take());
        let now = Instant::now();
        let end = if deadline > now {
            deadline.min(cleanup_end(now))
        } else {
            cleanup_end(now)
        };
        let drained = self.drain(strict, end).await;
        if drained.is_err() {
            terminate_and_reap(&mut self.child).await?;
            self.child_status = self.child.try_wait().map_err(|_| ClientError::Wait)?;
            // Draining after a kill is cleanup, never success proof for the earlier error.
            let _ = self.drain(false, cleanup_end(Instant::now())).await;
            return Err(drained.err().unwrap_or(ClientError::Cleanup));
        }
        let status = self.child_status.ok_or(ClientError::Wait)?;
        Ok(crate::SessionCompletion {
            status,
            diagnostics: self.diagnostics.clone(),
            diagnostics_truncated: self.diagnostics_truncated,
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.is_finished() {
            drop(self.stdin.take());
            let _ = self.child.start_kill();
            // Unwind cleanup is not a reported successful session completion.
            let end = cleanup_end(Instant::now());
            while Instant::now() < end {
                match self.child.try_wait() {
                    Ok(Some(_)) | Err(_) => break,
                    Ok(None) => std::thread::sleep(Duration::from_millis(2)),
                }
            }
        }
    }
}
