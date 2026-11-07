//! Serviced stdio for one original query/window and one joinable owning worker.

mod worker;

use std::future::pending;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use search_provider_protocol::{
    FrameBuffer, MAX_CLIENT_FRAME_BYTES, MAX_REMAINING_MS, ProtocolError, SessionClientFrame,
    SessionOutcome, decode_session_frame,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

use super::{CLEANUP_ALLOWANCE, OutputStep, output_step};
use crate::verified_query::QueryFailure;
use worker::{Command, Job, Ready, Worker};

struct Active {
    id: u64,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
    reply: oneshot::Receiver<Result<Ready, &'static str>>,
    stopped: Option<QueryFailure>,
    cleanup: Option<Instant>,
}

struct Output {
    id: u64,
    ready: Ready,
    cursor: usize,
    deadline: Instant,
}

struct Exchange {
    input: FrameBuffer,
    frame_started: Option<Instant>,
    opened: bool,
    last_id: u64,
    expires: Option<Instant>,
    active: Option<Active>,
    output: Option<Output>,
    // A complete next request can arrive after peer decode but before local
    // flush. Keep exactly one frame until that flush commits the previous page.
    deferred: Option<(SessionClientFrame, Instant)>,
}

impl Drop for Exchange {
    fn drop(&mut self) {
        self.cancel_active();
    }
}

impl Exchange {
    fn new() -> Result<Self, &'static str> {
        Ok(Self {
            input: FrameBuffer::new(MAX_CLIENT_FRAME_BYTES).map_err(ProtocolError::code)?,
            frame_started: None,
            opened: false,
            last_id: 0,
            expires: None,
            active: None,
            output: None,
            deferred: None,
        })
    }

    fn cancel_active(&self) {
        if let Some(active) = &self.active {
            active.cancel.store(true, Ordering::Release);
        }
    }

    fn reject(&mut self, id: u64, code: &'static str) -> Result<(), &'static str> {
        let ready = worker::failure(id, QueryFailure::Code(code))?;
        self.output = Some(Output {
            id,
            ready,
            cursor: 0,
            deadline: Instant::now()
                .checked_add(CLEANUP_ALLOWANCE)
                .ok_or("QUERY_DEADLINE")?,
        });
        Ok(())
    }

    fn cancel(&mut self, id: u64) -> Result<(), &'static str> {
        if let Some(active) = &mut self.active {
            if active.id != id {
                return Err("QUERY_RESPONSE_MISMATCH");
            }
            active.cancel.store(true, Ordering::Release);
            active.stopped.get_or_insert(QueryFailure::Cancelled);
            active.cleanup.get_or_insert(
                Instant::now()
                    .checked_add(CLEANUP_ALLOWANCE)
                    .ok_or("QUERY_DEADLINE")?,
            );
            return Ok(());
        }
        if let Some(output) = &mut self.output {
            if output.id != id {
                return Err("QUERY_RESPONSE_MISMATCH");
            }
            if output.cursor != 0 {
                return Err("SESSION_PARTIAL_OUTPUT_CANCELLED");
            }
            output.ready = worker::failure(id, QueryFailure::Cancelled)?;
            return Ok(());
        }
        self.reject(id, "SESSION_NO_ACTIVE_REQUEST")
    }

    fn accept(
        &mut self,
        message: SessionClientFrame,
        started: Instant,
        commands: &mpsc::Sender<Command>,
    ) -> Result<(), &'static str> {
        let id = message.request_id();
        if let SessionClientFrame::Cancel { .. } = message {
            return self.cancel(id);
        }
        if id <= self.last_id {
            if self.active.is_some() || self.output.is_some() {
                return Err("SESSION_REQUEST_ID_INVALID");
            }
            return self.reject(id, "SESSION_REQUEST_ID_INVALID");
        }
        let close = matches!(message, SessionClientFrame::Close { .. });
        if let Some(output) = &self.output {
            if close && output.cursor == 0 {
                self.output = None;
                self.deferred = None;
            } else {
                if close && output.cursor != output.ready.bytes.len() {
                    return Err("SESSION_PARTIAL_OUTPUT_CLOSE");
                }
                if self.deferred.is_some() && !close {
                    return Err("SESSION_REQUEST_IN_FLIGHT");
                }
                self.deferred = Some((message, started));
                return Ok(());
            }
        }
        if self.active.is_some() {
            if !close {
                return Err("SESSION_REQUEST_IN_FLIGHT");
            }
            self.cancel_active();
            self.active = None;
        }
        self.last_id = id;
        let now = Instant::now();
        let (remaining, job_expiry) = match &message {
            SessionClientFrame::Open {
                remaining_ms,
                ttl_ms,
                ..
            } => {
                if self.opened {
                    return self.reject(id, "WINDOW_ALREADY_OPEN");
                }
                self.opened = true;
                // Accepted Open, before worker dispatch/root acquisition/pinning.
                let expires = now
                    .checked_add(Duration::from_millis(*ttl_ms))
                    .ok_or("QUERY_DEADLINE")?;
                self.expires = Some(expires);
                (*remaining_ms, Some(expires))
            }
            SessionClientFrame::Continue { remaining_ms, .. }
            | SessionClientFrame::Expand { remaining_ms, .. } => {
                if !self.opened {
                    return self.reject(id, "WINDOW_NOT_OPEN");
                }
                (*remaining_ms, None)
            }
            SessionClientFrame::Close { .. } => (5000, None),
            SessionClientFrame::Cancel { .. } => return Err("SESSION_CONTROL_INVALID"),
        };
        let mut deadline = started
            .checked_add(Duration::from_millis(remaining))
            .ok_or("QUERY_DEADLINE")?;
        if let Some(expires) = self.expires {
            deadline = deadline.min(expires);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let (reply, received) = oneshot::channel();
        commands
            .send(Command::Execute(Job {
                message,
                deadline,
                expires: job_expiry,
                cancel: Arc::clone(&cancel),
                reply,
            }))
            .map_err(|_| "QUERY_WORKER_PANICKED")?;
        self.active = Some(Active {
            id,
            deadline,
            cancel,
            reply: received,
            stopped: None,
            cleanup: None,
        });
        Ok(())
    }

    fn consume(
        &mut self,
        bytes: &[u8],
        commands: &mpsc::Sender<Command>,
    ) -> Result<(), &'static str> {
        let mut offset = 0;
        while offset < bytes.len() {
            if self.input.is_empty() {
                self.frame_started = Some(Instant::now());
            }
            let count = self
                .input
                .push(&bytes[offset..])
                .map_err(ProtocolError::code)?;
            if count == 0 {
                return Err("QUERY_FRAME_INVALID");
            }
            offset += count;
            if let Some(frame) = self.input.frame() {
                let message = decode_session_frame(frame).map_err(ProtocolError::code)?;
                let started = self.frame_started.take().ok_or("QUERY_FRAME_INVALID")?;
                self.input =
                    FrameBuffer::new(MAX_CLIENT_FRAME_BYTES).map_err(ProtocolError::code)?;
                self.accept(message, started, commands)?;
            }
        }
        Ok(())
    }

    fn received(&mut self, ready: Result<Ready, &'static str>) -> Result<(), &'static str> {
        let active = self.active.take().ok_or("SESSION_NO_ACTIVE_REQUEST")?;
        let mut ready = ready?;
        if let Some(failure) = active.stopped
            && !ready.end
        {
            ready = worker::failure(active.id, failure)?;
        }
        self.output = Some(Output {
            id: active.id,
            ready,
            cursor: 0,
            deadline: active.cleanup.unwrap_or(active.deadline),
        });
        Ok(())
    }

    fn deadline(&self, idle: Instant) -> Instant {
        let mut end = self.expires.unwrap_or(idle);
        if let Some(active) = &self.active {
            end = end.min(active.cleanup.unwrap_or(active.deadline));
        }
        if let Some(output) = &self.output {
            end = end.min(output.deadline);
        }
        if let Some(started) = self.frame_started
            && let Some(partial) = started.checked_add(Duration::from_millis(MAX_REMAINING_MS))
        {
            end = end.min(partial);
        }
        end
    }

    fn timed_out(&mut self) -> Result<(), &'static str> {
        if let Some(active) = &mut self.active {
            if active.cleanup.is_some() {
                return Err("QUERY_CLEANUP_EXPIRED");
            }
            active.cancel.store(true, Ordering::Release);
            active.stopped = Some(QueryFailure::Deadline);
            active.cleanup = Instant::now().checked_add(CLEANUP_ALLOWANCE);
            return Ok(());
        }
        Err("QUERY_DEADLINE")
    }

    fn flushed(&mut self, commands: &mpsc::Sender<Command>) -> Result<bool, &'static str> {
        let output = self.output.take().ok_or("QUERY_OUTPUT_IO")?;
        if let Some((expected, next)) = output.ready.commit {
            commands
                .send(Command::Commit { expected, next })
                .map_err(|_| "QUERY_WORKER_PANICKED")?;
        }
        if output.ready.end {
            return Ok(true);
        }
        if let Some((message, started)) = self.deferred.take() {
            self.accept(message, started, commands)?;
        }
        Ok(false)
    }
}

async fn receive(
    active: &mut Option<Active>,
) -> Result<Result<Ready, &'static str>, oneshot::error::RecvError> {
    match active {
        Some(active) => (&mut active.reply).await,
        None => pending().await,
    }
}

async fn expire(
    state: &Exchange,
    commands: &mpsc::Sender<Command>,
    done: &mut oneshot::Receiver<()>,
    stdout: &mut tokio::io::Stdout,
) -> Result<(), &'static str> {
    state.cancel_active();
    let _ = commands.send(Command::Shutdown);
    tokio::time::timeout(CLEANUP_ALLOWANCE, done)
        .await
        .map_err(|_| "QUERY_CLEANUP_EXPIRED")?
        .map_err(|_| "QUERY_WORKER_PANICKED")?;
    if state
        .output
        .as_ref()
        .is_some_and(|output| output.cursor != 0)
    {
        return Err("WINDOW_EXPIRED_PARTIAL_OUTPUT");
    }
    let id = state.active.as_ref().map_or_else(
        || state.output.as_ref().map_or(0, |output| output.id),
        |active| active.id,
    );
    let ready = worker::outcome(id, SessionOutcome::Expired {}, true)?;
    tokio::time::timeout(CLEANUP_ALLOWANCE, async {
        stdout.write_all(&ready.bytes).await?;
        stdout.flush().await
    })
    .await
    .map_err(|_| "QUERY_CLEANUP_EXPIRED")?
    .map_err(|_| "QUERY_OUTPUT_IO")
}

async fn exchange(
    commands: &mpsc::Sender<Command>,
    done: &mut oneshot::Receiver<()>,
) -> Result<(), &'static str> {
    let idle = Instant::now()
        .checked_add(Duration::from_millis(MAX_REMAINING_MS))
        .ok_or("QUERY_DEADLINE")?;
    let mut state = Exchange::new()?;
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut chunk = [0_u8; 8192];
    loop {
        if state
            .expires
            .is_some_and(|expires| Instant::now() >= expires)
        {
            return expire(&state, commands, done, &mut stdout).await;
        }
        let deadline = state.deadline(idle);
        let bytes = state
            .output
            .as_ref()
            .map(|output| output.ready.bytes.as_slice());
        let cursor = state.output.as_ref().map_or(0, |output| output.cursor);
        tokio::select! {
            read = stdin.read(&mut chunk) => {
                let count = read.map_err(|_| "QUERY_INPUT_IO")?;
                if count == 0 {
                    return if state.input.is_empty() { Ok(()) } else { Err("QUERY_FRAME_INVALID") };
                }
                state.consume(&chunk[..count], commands)?;
            }
            reply = receive(&mut state.active), if state.output.is_none() => {
                state.received(reply.map_err(|_| "QUERY_WORKER_PANICKED")?)?;
            }
            result = output_step(bytes, cursor, &mut stdout) => {
                match result.map_err(|_| "QUERY_OUTPUT_IO")? {
                    OutputStep::Wrote(0) => return Err("QUERY_OUTPUT_CLOSED"),
                    OutputStep::Wrote(count) => state.output.as_mut().ok_or("QUERY_OUTPUT_IO")?.cursor += count,
                    OutputStep::Flushed => {
                        if state.expires.is_some_and(|expires| Instant::now() >= expires) {
                            return expire(&state, commands, done, &mut stdout).await;
                        }
                        if state.flushed(commands)? { return Ok(()); }
                    }
                }
            }
            result = &mut *done, if state.active.is_none() && state.output.is_none() => {
                result.map_err(|_| "QUERY_WORKER_PANICKED")?;
                if state.expires.is_none_or(|expires| Instant::now() < expires) {
                    return Err("QUERY_WORKER_PANICKED");
                }
                let ready = worker::outcome(0, SessionOutcome::Expired {}, true)?;
                return tokio::time::timeout(CLEANUP_ALLOWANCE, async {
                    stdout.write_all(&ready.bytes).await?;
                    stdout.flush().await
                }).await.map_err(|_| "QUERY_CLEANUP_EXPIRED")?.map_err(|_| "QUERY_OUTPUT_IO");
            }
            () = tokio::time::sleep_until(deadline.into()) => {
                if state.expires.is_some_and(|expires| Instant::now() >= expires) {
                    return expire(&state, commands, done, &mut stdout).await;
                }
                state.timed_out()?;
            }
        }
    }
}

pub fn run(root: &Path, endpoint: Option<&str>) -> ExitCode {
    let result = (|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "QUERY_IO_RUNTIME_FAILED")?;
        let mut worker = Worker::spawn(root.to_path_buf(), endpoint.map(str::to_owned))?;
        let result = runtime.block_on(exchange(&worker.commands, &mut worker.done));
        let joined = worker.finish();
        // The root-owning worker is joined. A maintained stdio blocking task
        // cannot hold runtime destruction behind a pipe; this owned process ends.
        runtime.shutdown_timeout(Duration::ZERO);
        joined?;
        result
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            eprintln!("{{\"error\":\"{code}\"}}");
            ExitCode::from(2)
        }
    }
}
