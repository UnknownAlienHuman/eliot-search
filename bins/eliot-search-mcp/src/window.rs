//! One asynchronous SDK window owner, including idle expiry and separate control.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use search_provider_client::{ClientError, OwnedQuerySession};
use search_provider_protocol::{
    SESSION_WINDOW_ID, SessionClientFrame, SessionOutcome, WindowQuery,
};
use serde_json::json;
use tokio::sync::{OwnedSemaphorePermit, mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

use crate::Config;
use crate::server::{Outcome, remaining, reply_result, text_reply};

pub struct Open {
    pub query: WindowQuery,
    pub page_size: u8,
    pub ttl_ms: u64,
    pub include_source_info: bool,
    pub deadline: Instant,
    pub permit: OwnedSemaphorePermit,
}

pub enum Data {
    Continue {
        page_size: u8,
    },
    Expand {
        item_index: u8,
        before_bytes: u64,
        after_bytes: u64,
    },
}

struct OpenCommand {
    open: Open,
    reply: oneshot::Sender<Outcome>,
    close: CancellationToken,
    done: watch::Sender<Option<Outcome>>,
}

enum Command {
    Open(OpenCommand),
    Data {
        handle: String,
        data: Data,
        deadline: Instant,
        reply: oneshot::Sender<Outcome>,
    },
}

struct Cancel {
    handle: String,
    deadline: Instant,
    reply: oneshot::Sender<Outcome>,
}

#[derive(Clone)]
struct Slot {
    handle: String,
    close: CancellationToken,
    done: watch::Receiver<Option<Outcome>>,
}

#[derive(Clone)]
pub struct DeliveryBinding {
    pub close: CancellationToken,
    done: watch::Receiver<Option<Outcome>>,
}

impl DeliveryBinding {
    pub async fn finish(&self, shutdown: &CancellationToken) -> Result<(), &'static str> {
        self.close.cancel();
        ended(self.done.clone(), shutdown).await.map(|_| ())
    }
}

pub struct Hub {
    data: mpsc::Sender<Command>,
    control: mpsc::Sender<Cancel>,
    slot: Mutex<Option<Slot>>,
    shutdown: CancellationToken,
}

struct CloseOnDrop {
    token: CancellationToken,
    armed: bool,
}
impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.token.cancel();
        }
    }
}

async fn ended(
    mut done: watch::Receiver<Option<Outcome>>,
    shutdown: &CancellationToken,
) -> Outcome {
    loop {
        let observed = done.borrow().clone();
        if let Some(result) = observed {
            return result;
        }
        done.changed().await.map_err(|_| {
            shutdown.cancel();
            "MCP_WINDOW_OWNER_STOPPED"
        })?;
    }
}

async fn answer(
    mut receiver: oneshot::Receiver<Outcome>,
    context: &CancellationToken,
    deadline: Instant,
    close: CancellationToken,
    done: watch::Receiver<Option<Outcome>>,
    shutdown: &CancellationToken,
) -> Outcome {
    let mut guard = CloseOnDrop {
        token: close.clone(),
        armed: true,
    };
    let cause = tokio::select! {
        biased;
        () = context.cancelled() => "MCP_REQUEST_CANCELLED",
        () = tokio::time::sleep_until(deadline.into()) => "QUERY_DEADLINE",
        value = &mut receiver => {
            let value = value.map_err(|_| {
                shutdown.cancel();
                "MCP_WINDOW_OWNER_STOPPED"
            })?;
            // A received local refusal or validated reply is settled by the owner;
            // unlike an abandoned RPC it does not implicitly close a healthy view.
            guard.armed = false;
            return value;
        }
    };
    close.cancel();
    ended(done, shutdown).await?;
    Err(cause)
}

impl Hub {
    pub fn start(
        config: Arc<Config>,
        shutdown: CancellationToken,
        epoch: String,
    ) -> (Arc<Self>, tokio::task::JoinHandle<Result<(), &'static str>>) {
        // Data and Cancel have separate bounded slots. Close uses a token and
        // cannot be blocked by a full data/control queue.
        let (data, mut requests) = mpsc::channel(1);
        let (control, mut controls) = mpsc::channel(1);
        let hub = Arc::new(Self {
            data,
            control,
            slot: Mutex::new(None),
            shutdown,
        });
        let owner = hub.clone();
        let task = tokio::spawn(async move {
            let result = async {
            let mut sequence = 0_u64;
            loop {
                tokio::select! {
                    biased;
                    () = owner.shutdown.cancelled() => break,
                    Some(cancel) = controls.recv() => {
                        let _ = cancel.reply.send(Err("WINDOW_NOT_OPEN"));
                    }
                    Some(command) = requests.recv() => match command {
                        Command::Open(command) => {
                            sequence = sequence.checked_add(1).ok_or("MCP_WINDOW_ID_EXHAUSTED")?;
                            let handle = format!("w{epoch}-{sequence}");
                            let slot = Slot { handle: handle.clone(), close: command.close.clone(), done: command.done.subscribe() };
                            *owner.slot.lock().map_err(|_| "MCP_OWNER_STATE_CORRUPT")? = Some(slot);
                            drive(&owner, &config, handle, command, &mut requests, &mut controls).await?;
                        }
                        Command::Data { reply, .. } => { let _ = reply.send(Err("WINDOW_NOT_OPEN")); }
                    }
                }
            }
            Ok(())
            }.await;
            if result.is_err() {
                owner.shutdown.cancel();
            }
            result
        });
        (hub, task)
    }

    // A missing/different slot means this exact epoch+sequence was already
    // removed by drive after explicit SDK finish/reap; never close a new window.
    pub fn delivery_binding(&self, handle: &str) -> Result<Option<DeliveryBinding>, &'static str> {
        let slot = self.slot.lock().map_err(|_| "MCP_OWNER_STATE_CORRUPT")?;
        Ok(slot
            .as_ref()
            .filter(|slot| slot.handle == handle)
            .map(|slot| DeliveryBinding {
                close: slot.close.clone(),
                done: slot.done.clone(),
            }))
    }

    fn selected(&self, handle: &str) -> Result<Slot, &'static str> {
        if self.shutdown.is_cancelled() {
            return Err("MCP_SHUTTING_DOWN");
        }
        self.slot
            .lock()
            .map_err(|_| "MCP_OWNER_STATE_CORRUPT")?
            .as_ref()
            .filter(|slot| slot.handle == handle)
            .cloned()
            .ok_or("WINDOW_NOT_OPEN")
    }

    pub async fn open(&self, open: Open, context: &CancellationToken) -> Outcome {
        if self.shutdown.is_cancelled() || context.is_cancelled() {
            return Err("MCP_REQUEST_CANCELLED");
        }
        let (reply, receiver) = oneshot::channel();
        let (done, ended_rx) = watch::channel(None);
        let close = CancellationToken::new();
        let deadline = open.deadline;
        self.data
            .try_send(Command::Open(OpenCommand {
                open,
                reply,
                close: close.clone(),
                done,
            }))
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => "MCP_WINDOW_REQUEST_IN_FLIGHT",
                mpsc::error::TrySendError::Closed(_) => {
                    self.shutdown.cancel();
                    "MCP_WINDOW_OWNER_STOPPED"
                }
            })?;
        answer(receiver, context, deadline, close, ended_rx, &self.shutdown).await
    }

    pub async fn data(
        &self,
        handle: &str,
        data: Data,
        deadline: Instant,
        context: &CancellationToken,
    ) -> Outcome {
        let slot = self.selected(handle)?;
        if slot.close.is_cancelled() {
            return Err("WINDOW_CLOSING");
        }
        let (reply, receiver) = oneshot::channel();
        self.data
            .try_send(Command::Data {
                handle: handle.to_owned(),
                data,
                deadline,
                reply,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => "MCP_WINDOW_REQUEST_IN_FLIGHT",
                mpsc::error::TrySendError::Closed(_) => {
                    self.shutdown.cancel();
                    "MCP_WINDOW_OWNER_STOPPED"
                }
            })?;
        answer(
            receiver,
            context,
            deadline,
            slot.close,
            slot.done,
            &self.shutdown,
        )
        .await
    }

    pub async fn cancel(
        &self,
        handle: &str,
        deadline: Instant,
        context: &CancellationToken,
    ) -> Outcome {
        let slot = self.selected(handle)?;
        if slot.close.is_cancelled() {
            return Err("WINDOW_CLOSING");
        }
        let (reply, receiver) = oneshot::channel();
        self.control
            .try_send(Cancel {
                handle: handle.to_owned(),
                deadline,
                reply,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => "MCP_WINDOW_CONTROL_IN_FLIGHT",
                mpsc::error::TrySendError::Closed(_) => {
                    self.shutdown.cancel();
                    "MCP_WINDOW_OWNER_STOPPED"
                }
            })?;
        answer(
            receiver,
            context,
            deadline,
            slot.close,
            slot.done,
            &self.shutdown,
        )
        .await
    }

    pub async fn close(
        &self,
        handle: &str,
        deadline: Instant,
        context: &CancellationToken,
    ) -> Outcome {
        let slot = self.selected(handle)?;
        slot.close.cancel();
        let outcome = ended(slot.done, &self.shutdown).await;
        if context.is_cancelled() {
            return Err("MCP_REQUEST_CANCELLED");
        }
        if Instant::now() >= deadline {
            return Err("QUERY_DEADLINE");
        }
        outcome
    }
}

struct Pending {
    id: u64,
    open: bool,
    deadline: Instant,
    reply: oneshot::Sender<Outcome>,
}

fn cleanup_deadline() -> Instant {
    // This is a cleanup allowance, never additional query work or a TTL renewal.
    Instant::now() + Duration::from_secs(2)
}

fn next_id(sequence: &mut u64) -> Result<u64, &'static str> {
    *sequence = sequence
        .checked_add(1)
        .ok_or("SESSION_REQUEST_ID_INVALID")?;
    Ok(*sequence)
}

const fn observed_kind(outcome: &SessionOutcome) -> &'static str {
    match outcome {
        SessionOutcome::Page { .. } => "page",
        SessionOutcome::Expanded { .. } => "expanded",
        SessionOutcome::Closed {} => "closed",
        SessionOutcome::Expired {} => "expired",
        SessionOutcome::Cancelled {} => "cancelled",
        SessionOutcome::Failed { .. } => "failed",
    }
}

#[allow(clippy::too_many_lines)]
async fn drive(
    hub: &Hub,
    config: &Config,
    handle: String,
    command: OpenCommand,
    requests: &mut mpsc::Receiver<Command>,
    controls: &mut mpsc::Receiver<Cancel>,
) -> Result<(), &'static str> {
    let OpenCommand {
        open,
        reply,
        close,
        done,
    } = command;
    let Open {
        query,
        page_size,
        ttl_ms,
        include_source_info,
        deadline,
        permit,
    } = open;
    // Kept until explicit finish/reap and completion publication below.
    let _permit = permit;
    let mut initial_reply = Some(reply);
    let mut session = None;
    let mut pending: Option<Pending> = None;
    let mut cancel_waiter: Option<oneshot::Sender<Outcome>> = None;
    let mut delayed_open: Option<(oneshot::Sender<Outcome>, Outcome)> = None;
    let false_cancel = AtomicBool::new(false);
    let mut sequence = 1_u64;
    let mut closing: Option<Instant> = None;
    let outcome: Outcome = async {
        if close.is_cancelled() || hub.shutdown.is_cancelled() { return Err("MCP_REQUEST_CANCELLED"); }
        let ranked = matches!(query, WindowQuery::Search(_));
        let args = config.arguments(true, ranked)?;
        session = Some(OwnedQuerySession::open(&config.daemon, &args, deadline, &false_cancel, false)
            .await.map_err(ClientError::code)?);
        let session = session.as_mut().ok_or("MCP_WINDOW_OWNER_STOPPED")?;
        session.submit(SessionClientFrame::Open {
            request_id: 1, remaining_ms: remaining(deadline)?, query,
            page_size, ttl_ms, include_summary: true, include_source_info,
        }, deadline).map_err(ClientError::code)?;
        let idle_end = Instant::now() + Duration::from_millis(ttl_ms) + Duration::from_secs(2);
        pending = Some(Pending { id: 1, open: true, deadline, reply: initial_reply.take().ok_or("MCP_WINDOW_OWNER_STOPPED")? });
        loop {
            let receive_end = closing.unwrap_or_else(|| pending.as_ref().map_or(idle_end, |p| p.deadline.min(idle_end)));
            tokio::select! {
                biased;
                () = hub.shutdown.cancelled(), if closing.is_none() => {
                    let end = cleanup_deadline();
                    session.submit(SessionClientFrame::Close { request_id: next_id(&mut sequence)? }, end).map_err(ClientError::code)?;
                    closing = Some(end);
                }
                () = close.cancelled(), if closing.is_none() => {
                    let end = cleanup_deadline();
                    session.submit(SessionClientFrame::Close { request_id: next_id(&mut sequence)? }, end).map_err(ClientError::code)?;
                    closing = Some(end);
                }
                Some(cancel) = controls.recv(), if closing.is_none() => {
                    if cancel.handle != handle {
                        let _ = cancel.reply.send(Err("WINDOW_NOT_OPEN"));
                    } else if let Some(active) = &pending {
                        match session.submit(SessionClientFrame::Cancel { request_id: active.id }, cancel.deadline) {
                            Ok(()) => { cancel_waiter = Some(cancel.reply); }
                            Err(ClientError::CommandRejected { code, .. }) => { let _ = cancel.reply.send(Err(code)); }
                            Err(error) => { let _ = cancel.reply.send(Err(error.code())); return Err(error.code()); }
                        }
                    } else {
                        let _ = cancel.reply.send(text_reply(&json!({
                            "window_handle":handle,"cancel_requested":false,"reason":"no_active_request"
                        }), false, Some(&handle)));
                    }
                }
                Some(command) = requests.recv(), if closing.is_none() => {
                    match command {
                        Command::Open(command) => { let _ = command.reply.send(Err("MCP_ROOT_BUSY")); }
                        Command::Data { handle: requested, data, deadline, reply } => {
                            if requested != handle {
                                let _ = reply.send(Err("WINDOW_NOT_OPEN"));
                                continue;
                            }
                            if pending.is_some() {
                                let _ = reply.send(Err("SESSION_REQUEST_IN_FLIGHT"));
                                continue;
                            }
                            let id = next_id(&mut sequence)?;
                            let remaining_ms = match remaining(deadline) {
                                Ok(left) => left,
                                Err(code) => { let _ = reply.send(Err(code)); continue; }
                            };
                            let frame = match data {
                                Data::Continue { page_size } => SessionClientFrame::Continue { request_id:id,remaining_ms,window_id:SESSION_WINDOW_ID,page_size },
                                Data::Expand { item_index,before_bytes,after_bytes } => SessionClientFrame::Expand { request_id:id,remaining_ms,window_id:SESSION_WINDOW_ID,item_index,before_bytes,after_bytes },
                            };
                            match session.submit(frame,deadline) {
                                Ok(()) => pending=Some(Pending {id,open:false,deadline,reply}),
                                Err(ClientError::CommandRejected {code,..}) => {let _=reply.send(Err(code));}
                                Err(error) => {let _=reply.send(Err(error.code()));return Err(error.code());}
                            }
                        }
                    }
                }
                received = session.next_reply(receive_end,&false_cancel) => {
                    let reply=received.map_err(ClientError::code)?;
                    if let Some(waiter)=cancel_waiter.take() {
                        let _=waiter.send(text_reply(&json!({
                            "window_handle":handle,"cancel_requested":true,
                            "request_id":reply.request_id,"observed_outcome":observed_kind(&reply.outcome)
                        }), false, Some(&handle)));
                    }
                    if matches!(reply.outcome,SessionOutcome::Closed {}|SessionOutcome::Expired {}) {
                        if let Some(active)=pending.take() {
                            let value=if active.open {reply_result(None,reply.clone())}else{Err("WINDOW_CLOSED")};
                            delayed_open=Some((active.reply,value));
                        }
                        return reply_result(Some(&handle),reply);
                    }
                    let active=pending.take().ok_or("MCP_WINDOW_REPLY_WITHOUT_REQUEST")?;
                    if active.open && !matches!(reply.outcome,SessionOutcome::Page {..}) {
                        delayed_open=Some((active.reply,reply_result(None,reply)));
                        close.cancel();
                    } else {
                        if active.reply.is_closed() {close.cancel();}
                        let _=active.reply.send(reply_result(Some(&handle),reply));
                    }
                }
            }
        }
    }.await;

    let cleaned = if let Some(session) = &mut session {
        if session.is_finished() {
            Ok(())
        } else {
            session
                .finish(cleanup_deadline())
                .await
                .map_err(ClientError::code)
                .and_then(|completion| {
                    if completion.status.success() {
                        Ok(())
                    } else {
                        Err("QUERY_CHILD_EXIT_FAILED")
                    }
                })
        }
    } else {
        Ok(())
    };
    let outcome = match cleaned {
        Ok(()) => outcome,
        Err(code) => Err(code),
    };
    if let Some(reply) = initial_reply {
        let _ = reply.send(outcome.clone());
    }
    if let Some(active) = pending {
        let _ = active.reply.send(Err(outcome
            .as_ref()
            .err()
            .copied()
            .unwrap_or("WINDOW_CLOSED")));
    }
    if let Some(reply) = cancel_waiter {
        let _ = reply.send(Err(outcome
            .as_ref()
            .err()
            .copied()
            .unwrap_or("WINDOW_CLOSED")));
    }
    if let Some((reply, held)) = delayed_open {
        let settled = if let Err(code) = &outcome {
            Err(*code)
        } else {
            held
        };
        let _ = reply.send(settled);
    }
    {
        let mut slot = hub.slot.lock().map_err(|_| "MCP_OWNER_STATE_CORRUPT")?;
        if slot.as_ref().is_some_and(|slot| slot.handle == handle) {
            *slot = None;
        }
    }
    // No live slot or capability is advertised after this settled cleanup.
    let _ = done.send_replace(Some(outcome));
    cleaned
}
