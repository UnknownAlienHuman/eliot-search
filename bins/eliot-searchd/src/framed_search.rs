//! One serviced framed query with one joinable synchronous worker.

use std::future::pending;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use search_control_redb::publication::{PublishedView, pin_published};
use search_provider_protocol::{
    ClientFrame, FailureCode, FrameBuffer, MAX_CLIENT_FRAME_BYTES, MAX_REMAINING_MS, ProtocolError,
    QUERY_REQUEST_ID, QueryOutcome, RawExactRequest, SearchRequest, Terminal, decode_client_frame,
    encode_terminal_frame, prepare_complete, prepare_raw_exact,
};
use search_runtime_owner::{PlainRootGuard, open_plain};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

use crate::{
    raw_exact,
    verified_query::{self, QueryFailure},
};

const CLEANUP_ALLOWANCE: Duration = Duration::from_secs(5);
type Encoded = Result<Vec<u8>, ProtocolError>;

enum Request {
    Search(SearchRequest),
    RawExact(RawExactRequest),
}
impl Request {
    fn execute(
        &self,
        root: &PlainRootGuard,
        view: &PublishedView,
        endpoint: Option<&str>,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Encoded {
        match self {
            Self::Search(request) => {
                let Some(endpoint) = endpoint else {
                    return stopped(QueryFailure::Code("QUERY_ENDPOINT_REQUIRED"));
                };
                match verified_query::execute(root, view, endpoint, request, deadline, cancel) {
                    Ok(items) => {
                        prepare_complete(QUERY_REQUEST_ID, view.generation.sequence, items)
                    }
                    Err(error) => stopped(error),
                }
            }
            Self::RawExact(request) => {
                match raw_exact::execute(root, view, request, deadline, cancel) {
                    Ok(result) => prepare_raw_exact(QUERY_REQUEST_ID, result),
                    Err(error) => stopped(error),
                }
            }
        }
    }
}

struct Worker {
    release: mpsc::Sender<()>,
    join: JoinHandle<()>,
}
impl Worker {
    fn finish(self) -> Result<(), &'static str> {
        drop(self.release);
        self.join.join().map_err(|_| "QUERY_WORKER_PANICKED")
    }
}
fn stopped(failure: QueryFailure) -> Encoded {
    let outcome = match failure {
        QueryFailure::Cancelled => QueryOutcome::Cancelled,
        QueryFailure::Deadline => QueryOutcome::Failed {
            code: FailureCode::new("QUERY_DEADLINE")?,
        },
        QueryFailure::Code(code) => QueryOutcome::Failed {
            code: FailureCode::new(code)?,
        },
    };
    encode_terminal_frame(&Terminal {
        request_id: QUERY_REQUEST_ID,
        outcome,
    })
}
fn worker_result(
    root: &Path,
    endpoint: Option<&str>,
    request: &Request,
    deadline: Instant,
    cancel: &AtomicBool,
    tx: oneshot::Sender<Encoded>,
    release: &mpsc::Receiver<()>,
) {
    // Resources stay on this ordinary thread, outside the async I/O context.
    // Even after the encoded result is sent, the root/view outlive terminal flush or stream closure.
    let root = open_plain(root, deadline, cancel);
    let view = root
        .as_ref()
        .ok()
        .map(|root| pin_published(&root.control_path(), deadline, cancel));
    let encoded = match (&root, &view) {
        (Err(error), _) => stopped(QueryFailure::root(*error)),
        (Ok(_), Some(Err(error))) => stopped(QueryFailure::publication(*error)),
        (Ok(root), Some(Ok(view))) => request.execute(root, view, endpoint, deadline, cancel),
        _ => stopped(QueryFailure::Code("QUERY_INTERNAL_ERROR")),
    };
    let _ = tx.send(encoded);
    let _ = release.recv();
    drop(view);
    drop(root);
}
fn spawn_worker(
    root: PathBuf,
    endpoint: Option<String>,
    request: Request,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
) -> Result<(Worker, oneshot::Receiver<Encoded>), &'static str> {
    let (tx, rx) = oneshot::channel();
    let (release, released) = mpsc::channel();
    let join = thread::Builder::new()
        .name("eliot-query".to_owned())
        .spawn(move || {
            worker_result(
                &root,
                endpoint.as_deref(),
                &request,
                deadline,
                &cancel,
                tx,
                &released,
            );
        })
        .map_err(|_| "QUERY_WORKER_START_FAILED")?;
    Ok((Worker { release, join }, rx))
}
async fn receive_result(
    receiver: &mut Option<oneshot::Receiver<Encoded>>,
) -> Result<Encoded, oneshot::error::RecvError> {
    match receiver {
        Some(receiver) => receiver.await,
        None => pending().await,
    }
}
enum OutputStep {
    Wrote(usize),
    Flushed,
}
async fn output_step(
    output: Option<&[u8]>,
    cursor: usize,
    stdout: &mut tokio::io::Stdout,
) -> std::io::Result<OutputStep> {
    let Some(bytes) = output else {
        return pending().await;
    };
    if cursor < bytes.len() {
        stdout.write(&bytes[cursor..]).await.map(OutputStep::Wrote)
    } else {
        stdout.flush().await?;
        Ok(OutputStep::Flushed)
    }
}

struct Exchange {
    input: FrameBuffer,
    frame_started: Option<Instant>,
    request_deadline: Option<Instant>,
    cleanup_deadline: Option<Instant>,
    admitted: bool,
    input_closed: bool,
    output: Option<Vec<u8>>,
    cursor: usize,
    receiver: Option<oneshot::Receiver<Encoded>>,
}
impl Exchange {
    fn new() -> Result<Self, &'static str> {
        Ok(Self {
            input: FrameBuffer::new(MAX_CLIENT_FRAME_BYTES).map_err(ProtocolError::code)?,
            frame_started: None,
            request_deadline: None,
            cleanup_deadline: None,
            admitted: false,
            input_closed: false,
            output: None,
            cursor: 0,
            receiver: None,
        })
    }
    fn stop(&mut self, reason: QueryFailure, cancel: &AtomicBool) -> Result<(), &'static str> {
        cancel.store(true, Ordering::Relaxed);
        if self.cursor == 0 {
            self.output = Some(stopped(reason).map_err(ProtocolError::code)?);
        }
        Ok(())
    }
    fn accept(
        &mut self,
        message: ClientFrame,
        frame_started: Instant,
        root: &Path,
        endpoint: Option<&str>,
        cancel: &Arc<AtomicBool>,
        worker: &mut Option<Worker>,
    ) -> Result<(), &'static str> {
        let (remaining_ms, request) = match message {
            ClientFrame::Query {
                remaining_ms,
                request,
                ..
            } => (remaining_ms, Request::Search(request)),
            ClientFrame::RawExact {
                remaining_ms,
                request,
                ..
            } => (remaining_ms, Request::RawExact(request)),
            ClientFrame::Cancel { .. } => {
                if !self.admitted {
                    return Err("QUERY_CANCEL_WITHOUT_REQUEST");
                }
                return self.stop(QueryFailure::Cancelled, cancel);
            }
        };
        if self.admitted {
            return Err("QUERY_DUPLICATE_REQUEST");
        }
        self.admitted = true;
        let deadline = frame_started
            .checked_add(Duration::from_millis(remaining_ms))
            .ok_or("QUERY_DEADLINE")?;
        self.request_deadline = Some(deadline);
        if Instant::now() >= deadline {
            self.cleanup_deadline = Instant::now().checked_add(CLEANUP_ALLOWANCE);
            return self.stop(QueryFailure::Deadline, cancel);
        }
        let (handle, receiver) = spawn_worker(
            root.to_path_buf(),
            endpoint.map(str::to_owned),
            request,
            deadline,
            Arc::clone(cancel),
        )?;
        *worker = Some(handle);
        self.receiver = Some(receiver);
        Ok(())
    }
    fn consume(
        &mut self,
        bytes: &[u8],
        root: &Path,
        endpoint: Option<&str>,
        cancel: &Arc<AtomicBool>,
        worker: &mut Option<Worker>,
    ) -> Result<(), &'static str> {
        let mut offset = 0;
        while offset < bytes.len() {
            if self.input.is_empty() {
                self.frame_started = Some(Instant::now());
            }
            let consumed = self
                .input
                .push(&bytes[offset..])
                .map_err(ProtocolError::code)?;
            if consumed == 0 {
                return Err("QUERY_FRAME_INVALID");
            }
            offset += consumed;
            if let Some(frame) = self.input.frame() {
                let message = decode_client_frame(frame).map_err(ProtocolError::code)?;
                let started = self.frame_started.take().ok_or("QUERY_FRAME_INVALID")?;
                self.input =
                    FrameBuffer::new(MAX_CLIENT_FRAME_BYTES).map_err(ProtocolError::code)?;
                self.accept(message, started, root, endpoint, cancel, worker)?;
            }
        }
        Ok(())
    }
}
async fn exchange(
    root: &Path,
    endpoint: Option<&str>,
    cancel: &Arc<AtomicBool>,
    worker: &mut Option<Worker>,
) -> Result<(), &'static str> {
    let idle_deadline = Instant::now()
        .checked_add(Duration::from_millis(MAX_REMAINING_MS))
        .ok_or("QUERY_DEADLINE")?;
    let mut state = Exchange::new()?;
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut chunk = [0_u8; 8192];
    loop {
        let deadline = state
            .cleanup_deadline
            .or(state.request_deadline)
            .unwrap_or_else(|| {
                state
                    .frame_started
                    .and_then(|t| t.checked_add(Duration::from_millis(MAX_REMAINING_MS)))
                    .unwrap_or(idle_deadline)
            });
        tokio::select! {
            read = stdin.read(&mut chunk), if !state.input_closed => {
                let size = read.map_err(|_| "QUERY_INPUT_IO")?;
                if size == 0 {
                    state.input_closed = true;
                    if state.output.is_none() { return Err("QUERY_INPUT_CLOSED"); }
                } else {
                    state.consume(&chunk[..size], root, endpoint, cancel, worker)?;
                }
            }
            result = receive_result(&mut state.receiver), if state.output.is_none() => {
                state.receiver = None;
                let frame = result.map_err(|_| "QUERY_WORKER_PANICKED")?
                    .or_else(|error| stopped(QueryFailure::Code(error.code())))
                    .map_err(ProtocolError::code)?;
                state.output = Some(if cancel.load(Ordering::Relaxed) {
                    stopped(QueryFailure::Cancelled).map_err(ProtocolError::code)?
                } else { frame });
            }
            result = output_step(state.output.as_deref(), state.cursor, &mut stdout) => {
                match result.map_err(|_| "QUERY_OUTPUT_IO")? {
                    OutputStep::Wrote(0) => return Err("QUERY_OUTPUT_CLOSED"),
                    OutputStep::Wrote(size) => state.cursor += size,
                    OutputStep::Flushed => return Ok(()),
                }
            }
            () = tokio::time::sleep_until(deadline.into()) => {
                if !state.admitted || state.cleanup_deadline.is_some() || state.cursor != 0 {
                    return Err("QUERY_DEADLINE");
                }
                state.stop(QueryFailure::Deadline, cancel)?;
                state.cleanup_deadline = Instant::now().checked_add(CLEANUP_ALLOWANCE);
            }
        }
    }
}
pub fn run(root: &Path, endpoint: Option<&str>) -> ExitCode {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut worker = None;
    let result = (|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "QUERY_IO_RUNTIME_FAILED")?;
        let result = runtime.block_on(exchange(root, endpoint, &cancel, &mut worker));
        cancel.store(true, Ordering::Relaxed);
        if let Some(handle) = worker.take() {
            handle.finish()?;
        }
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
