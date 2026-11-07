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

async fn terminate_and_reap(child: &mut Child) -> Result<(), ClientError> {
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
