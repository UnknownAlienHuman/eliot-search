//! One joinable synchronous owner of the session's root, snapshot and result vector.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use search_provider_protocol::{
    FailureCode, SessionClientFrame, SessionOutcome, SessionReply, encode_session_reply,
};
use tokio::sync::oneshot;

use super::super::window::Window;
use crate::exact_literal::check;
use crate::verified_query::QueryFailure;

pub struct Ready {
    pub bytes: Vec<u8>,
    pub commit: Option<(u8, u8)>,
    pub end: bool,
}

pub struct Job {
    pub message: SessionClientFrame,
    pub deadline: Instant,
    pub expires: Option<Instant>,
    pub cancel: Arc<AtomicBool>,
    pub reply: oneshot::Sender<Result<Ready, &'static str>>,
}

pub enum Command {
    Execute(Job),
    // Issued only by OutputStep::Flushed, ordered before any next operation.
    Commit { expected: u8, next: u8 },
    Shutdown,
}

pub struct Worker {
    pub commands: mpsc::Sender<Command>,
    pub done: oneshot::Receiver<()>,
    join: JoinHandle<()>,
}

pub fn outcome(request_id: u64, outcome: SessionOutcome, end: bool) -> Result<Ready, &'static str> {
    let bytes = encode_session_reply(&SessionReply {
        request_id,
        outcome,
    })
    .map_err(search_provider_protocol::ProtocolError::code)?;
    Ok(Ready {
        bytes,
        commit: None,
        end,
    })
}

pub fn failure(request_id: u64, failure: QueryFailure) -> Result<Ready, &'static str> {
    let outcome = match failure {
        QueryFailure::Cancelled => SessionOutcome::Cancelled {},
        QueryFailure::Deadline => SessionOutcome::Failed {
            code: FailureCode::new("QUERY_DEADLINE")
                .map_err(search_provider_protocol::ProtocolError::code)?,
        },
        QueryFailure::Code(code) => SessionOutcome::Failed {
            code: FailureCode::new(code).map_err(search_provider_protocol::ProtocolError::code)?,
        },
    };
    self::outcome(request_id, outcome, false)
}

impl Worker {
    pub fn spawn(root: PathBuf, endpoint: Option<String>) -> Result<Self, &'static str> {
        let (commands, received) = mpsc::channel();
        let (finished, done) = oneshot::channel();
        let join = thread::Builder::new()
            .name("eliot-window".to_owned())
            .spawn(move || {
                run(&root, endpoint.as_deref(), &received);
                // run has dropped the window/root before exposing completion.
                let _ = finished.send(());
            })
            .map_err(|_| "QUERY_WORKER_START_FAILED")?;
        Ok(Self {
            commands,
            done,
            join,
        })
    }

    pub fn finish(self) -> Result<(), &'static str> {
        let _ = self.commands.send(Command::Shutdown);
        self.join.join().map_err(|_| "QUERY_WORKER_PANICKED")
    }
}

fn execute(
    root: &std::path::Path,
    endpoint: Option<&str>,
    window: &mut Option<Window>,
    job: &Job,
) -> Result<Ready, QueryFailure> {
    check(job.deadline, &job.cancel)?;
    let request_id = job.message.request_id();
    match &job.message {
        SessionClientFrame::Open {
            query,
            page_size,
            include_summary,
            include_source_info,
            ..
        } => {
            let expires = job.expires.ok_or(QueryFailure::Code("WINDOW_NOT_OPEN"))?;
            *window = Some(Window::open(
                root,
                endpoint,
                query,
                (*include_summary, *include_source_info),
                expires,
                job.deadline,
                &job.cancel,
            )?);
            page(window.as_ref(), job, *page_size)
        }
        SessionClientFrame::Continue { page_size, .. } => page(window.as_ref(), job, *page_size),
        SessionClientFrame::Expand {
            item_index,
            before_bytes,
            after_bytes,
            ..
        } => {
            let window = window
                .as_ref()
                .ok_or(QueryFailure::Code("WINDOW_NOT_OPEN"))?;
            let expansion = window.expand(
                *item_index,
                *before_bytes,
                *after_bytes,
                job.deadline,
                &job.cancel,
            )?;
            outcome(request_id, SessionOutcome::Expanded { expansion }, false)
                .map_err(QueryFailure::Code)
        }
        SessionClientFrame::Close { .. } => {
            drop(window.take());
            outcome(request_id, SessionOutcome::Closed {}, true).map_err(QueryFailure::Code)
        }
        SessionClientFrame::Cancel { .. } => Err(QueryFailure::Code("SESSION_CONTROL_INVALID")),
    }
}

fn page(window: Option<&Window>, job: &Job, size: u8) -> Result<Ready, QueryFailure> {
    let window = window.ok_or(QueryFailure::Code("WINDOW_NOT_OPEN"))?;
    let page = window.page(job.message.request_id(), size, job.deadline, &job.cancel)?;
    Ok(Ready {
        bytes: page.bytes,
        commit: Some((page.expected_cursor, page.next_cursor)),
        end: false,
    })
}

fn run(root: &Path, endpoint: Option<&str>, commands: &mpsc::Receiver<Command>) {
    let mut window: Option<Window> = None;
    let mut expires: Option<Instant> = None;
    loop {
        let remaining = expires.map_or(Duration::from_secs(120), |end| {
            end.saturating_duration_since(Instant::now())
        });
        let Ok(command) = commands.recv_timeout(remaining) else {
            break;
        };
        match command {
            Command::Shutdown => break,
            Command::Commit { expected, next } => {
                if expires.is_some_and(|end| Instant::now() >= end) {
                    break;
                }
                if window
                    .as_mut()
                    .is_none_or(|window| window.commit_page(expected, next).is_err())
                {
                    break;
                }
            }
            Command::Execute(job) => {
                if expires.is_some_and(|end| Instant::now() >= end) {
                    drop(window.take());
                    let _ = job.reply.send(outcome(
                        job.message.request_id(),
                        SessionOutcome::Expired {},
                        true,
                    ));
                    break;
                }
                if job.expires.is_some() {
                    expires = job.expires;
                }
                let mut result = execute(root, endpoint, &mut window, &job);
                let lifetime_ended = expires.is_some_and(|end| Instant::now() >= end);
                if lifetime_ended {
                    drop(window.take());
                    result = outcome(job.message.request_id(), SessionOutcome::Expired {}, true)
                        .map_err(QueryFailure::Code);
                } else if job.cancel.load(Ordering::Relaxed)
                    && !matches!(job.message, SessionClientFrame::Close { .. })
                {
                    result = Err(QueryFailure::Cancelled);
                }
                let result = result
                    .or_else(|error| {
                        failure(job.message.request_id(), error).map_err(QueryFailure::Code)
                    })
                    .map_err(|error| match error {
                        QueryFailure::Code(code) => code,
                        QueryFailure::Cancelled => "QUERY_CANCELLED",
                        QueryFailure::Deadline => "QUERY_DEADLINE",
                    });
                let end = result.as_ref().is_ok_and(|ready| ready.end);
                let _ = job.reply.send(result);
                if lifetime_ended || end {
                    break;
                }
            }
        }
    }
    drop(window);
}
