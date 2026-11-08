//! Standard SDK transport with independent EOF and exact outbound frame accounting.

use std::fmt::Write as _;
use std::io::{self, Write};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use rmcp::RoleServer;
use rmcp::model::{CallToolResult, JsonRpcMessage, ProtocolVersion, RequestId, ServerResult};
use rmcp::service::{RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::transport::async_rw::{AsyncRwTransport, DEFAULT_MAX_LINE_LENGTH};
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};
use tokio_util::sync::CancellationToken;

use crate::window::DeliveryBinding;

pub const MAX_RPC_FRAME: usize = DEFAULT_MAX_LINE_LENGTH;
const INPUT_CHUNK: usize = 4096;
pub const STDIO_TEARDOWN: Duration = Duration::from_secs(2);

struct Count {
    bytes: usize,
    exceeded: bool,
}
impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_RPC_FRAME.saturating_sub(self.bytes) {
            self.exceeded = true;
            return Err(io::Error::other("MCP_RESPONSE_TOO_LARGE"));
        }
        self.bytes += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Same serializer as rmcp's codec, including the final newline.
fn frame_fits(message: &impl Serialize) -> io::Result<bool> {
    let mut count = Count {
        bytes: 1,
        exceeded: false,
    };
    match serde_json::to_writer(&mut count, message) {
        Ok(()) => Ok(true),
        Err(_) if count.exceeded => Ok(false),
        Err(_) => Err(io::Error::other("MCP_RPC_SERIALIZATION_FAILED")),
    }
}

pub fn result_fits(
    result: &CallToolResult,
    id: &RequestId,
    version: Option<ProtocolVersion>,
) -> io::Result<bool> {
    let mut result = ServerResult::CallToolResult(result.clone());
    if version.is_some_and(|v| v < ProtocolVersion::V_2026_07_28) {
        result.strip_result_type_for_legacy_peer();
    }
    let message: TxJsonRpcMessage<RoleServer> = JsonRpcMessage::response(result, id.clone());
    frame_fits(&message)
}

pub fn fresh_epoch() -> Result<String, &'static str> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "MCP_EPOCH_UNAVAILABLE")?;
    let mut epoch = String::with_capacity(32);
    for byte in bytes {
        write!(&mut epoch, "{byte:02x}").map_err(|_| "MCP_EPOCH_UNAVAILABLE")?;
    }
    Ok(epoch)
}

struct Delivery {
    id: RequestId,
    binding: DeliveryBinding,
    cancelled: CancellationToken,
    send_started: bool,
    _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
struct Sending {
    binding: DeliveryBinding,
    cancelled: CancellationToken,
}

pub struct Wire {
    pub shutdown: CancellationToken,
    failure: Mutex<Option<&'static str>>,
    reaped: watch::Sender<Option<Result<(), &'static str>>>,
    delivery: Mutex<Option<Delivery>>,
    admission: Arc<Semaphore>,
}

impl Wire {
    pub fn new() -> Self {
        let (reaped, _) = watch::channel(None);
        Self {
            shutdown: CancellationToken::new(),
            failure: Mutex::new(None),
            reaped,
            delivery: Mutex::new(None),
            admission: Arc::new(Semaphore::new(1)),
        }
    }

    pub fn fail(&self, cause: &'static str) {
        match self.failure.lock() {
            Ok(mut failure) => {
                if failure.is_none() {
                    *failure = Some(cause);
                }
            }
            Err(mut poisoned) => {
                **poisoned.get_mut() = Some("MCP_WIRE_STATE_CORRUPT");
            }
        }
        self.shutdown.cancel();
    }

    pub fn failure(&self) -> Result<Option<&'static str>, &'static str> {
        self.failure
            .lock()
            .map(|failure| *failure)
            .map_err(|_| "MCP_WIRE_STATE_CORRUPT")
    }

    pub fn cleanup_complete(&self, result: Result<(), &'static str>) {
        let _ = self.reaped.send_replace(Some(result));
    }

    async fn wait_reaped(&self) -> io::Result<()> {
        let mut state = self.reaped.subscribe();
        loop {
            let current = *state.borrow();
            if let Some(result) = current {
                return result.map_err(io::Error::other);
            }
            state
                .changed()
                .await
                .map_err(|_| io::Error::other("MCP_OWNER_STATE_LOST"))?;
        }
    }

    pub fn acquire_delivery(&self) -> Result<OwnedSemaphorePermit, &'static str> {
        self.admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| "MCP_WINDOW_DELIVERY_IN_FLIGHT")
    }

    pub fn bind(
        &self,
        id: RequestId,
        binding: DeliveryBinding,
        permit: OwnedSemaphorePermit,
    ) -> Result<(), &'static str> {
        let mut delivery = self.delivery.lock().map_err(|_| "MCP_WIRE_STATE_CORRUPT")?;
        if delivery.is_some() {
            return Err("MCP_WINDOW_DELIVERY_IN_FLIGHT");
        }
        *delivery = Some(Delivery {
            id,
            binding,
            cancelled: CancellationToken::new(),
            send_started: false,
            _permit: permit,
        });
        drop(delivery);
        Ok(())
    }

    /// Only a real MCP cancellation notification, not rmcp's normal request-token
    /// retirement, cancels an already prepared response.
    pub async fn cancel_delivery(&self, id: &RequestId) {
        let unsent = {
            let Ok(mut pending) = self.delivery.lock() else {
                self.fail("MCP_WIRE_STATE_CORRUPT");
                return;
            };
            let Some(delivery) = pending.as_mut().filter(|d| &d.id == id) else {
                return;
            };
            delivery.binding.close.cancel();
            delivery.cancelled.cancel();
            if delivery.send_started {
                // The transport may already have written part of this response.
                // It keeps the permit through settlement or connection fail-stop.
                return;
            }
            // rmcp can suppress ToSink entirely after cancellation. Transfer the
            // never-started delivery to this callback, not to a future send.
            pending.take()
        };
        if let Some(delivery) = unsent {
            if delivery.binding.finish(&self.shutdown).await.is_err() {
                self.fail("MCP_OWNER_CLEANUP_FAILED");
                let _ = self.wait_reaped().await;
            }
            // Release admission only after actual window cleanup has settled;
            // cleanup failure has already stopped this entire connection.
            drop(delivery);
        }
    }

    fn begin_send(&self, id: Option<&RequestId>) -> io::Result<Option<Sending>> {
        let mut delivery = self
            .delivery
            .lock()
            .map_err(|_| io::Error::other("MCP_WIRE_STATE_CORRUPT"))?;
        Ok(delivery.as_mut().filter(|d| Some(&d.id) == id).map(|d| {
            d.send_started = true;
            Sending {
                binding: d.binding.clone(),
                cancelled: d.cancelled.clone(),
            }
        }))
    }

    fn complete(&self, id: Option<&RequestId>) -> io::Result<()> {
        let mut delivery = self
            .delivery
            .lock()
            .map_err(|_| io::Error::other("MCP_WIRE_STATE_CORRUPT"))?;
        if delivery.as_ref().is_some_and(|d| Some(&d.id) == id) {
            drop(delivery.take());
        }
        Ok(())
    }
}

pub struct Input {
    receiver: mpsc::Receiver<Vec<u8>>,
    current: Vec<u8>,
    offset: usize,
}
impl AsyncRead for Input {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let input = self.get_mut();
        if output.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            if input.offset < input.current.len() {
                let count = output.remaining().min(input.current.len() - input.offset);
                output.put_slice(&input.current[input.offset..input.offset + count]);
                input.offset += count;
                return Poll::Ready(Ok(()));
            }
            match input.receiver.poll_recv(cx) {
                Poll::Ready(Some(bytes)) => {
                    input.current = bytes;
                    input.offset = 0;
                }
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// Read raw stdin independently of the SDK writer. There is no JSON parsing here.
/// Queue exhaustion fails the connection; bytes are never dropped and continued.
pub fn input(wire: Arc<Wire>) -> (Input, tokio::task::JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel(MAX_RPC_FRAME / INPUT_CHUNK);
    let task = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut bytes = [0_u8; INPUT_CHUNK];
        loop {
            let read = tokio::select! {
                biased;
                () = wire.shutdown.cancelled() => return,
                result = stdin.read(&mut bytes) => result,
            };
            match read {
                Ok(0) => {
                    wire.shutdown.cancel();
                    return;
                }
                Ok(count) => match sender.try_send(bytes[..count].to_vec()) {
                    Ok(()) => {}
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        wire.shutdown.cancel();
                        return;
                    }
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        wire.fail("MCP_INPUT_BACKPRESSURE");
                        return;
                    }
                },
                Err(_) => {
                    wire.fail("MCP_INPUT_READ_FAILED");
                    return;
                }
            }
        }
    });
    (
        Input {
            receiver,
            current: Vec::new(),
            offset: 0,
        },
        task,
    )
}

pub struct RpcTransport {
    inner: AsyncRwTransport<RoleServer, Input, tokio::io::Stdout>,
    wire: Arc<Wire>,
}
impl RpcTransport {
    pub fn new(input: Input, wire: Arc<Wire>) -> Self {
        Self {
            inner: AsyncRwTransport::new(input, tokio::io::stdout()),
            wire,
        }
    }
}
impl Transport<RoleServer> for RpcTransport {
    type Error = io::Error;

    fn send(
        &mut self,
        message: TxJsonRpcMessage<RoleServer>,
    ) -> impl std::future::Future<Output = io::Result<()>> + Send + 'static {
        let fits = frame_fits(&message);
        let id = match &message {
            JsonRpcMessage::Response(response) => Some(response.id.clone()),
            JsonRpcMessage::Error(error) => error.id.clone(),
            _ => None,
        };
        let pending = self.wire.begin_send(id.as_ref());
        let writing = self.inner.send(message);
        let wire = self.wire.clone();
        async move {
            if !matches!(fits, Ok(true)) {
                let cause = if fits.is_err() {
                    "MCP_RPC_SERIALIZATION_FAILED"
                } else {
                    "MCP_RESPONSE_TOO_LARGE"
                };
                wire.fail(cause);
                wire.wait_reaped().await?;
                wire.complete(id.as_ref())?;
                return Err(io::Error::other(cause));
            }
            let pending = match pending {
                Ok(pending) => pending,
                Err(error) => {
                    wire.fail("MCP_WIRE_STATE_CORRUPT");
                    wire.wait_reaped().await?;
                    return Err(error);
                }
            };
            if let Some(pending) = &pending
                && pending.cancelled.is_cancelled()
            {
                pending
                    .binding
                    .finish(&wire.shutdown)
                    .await
                    .map_err(io::Error::other)?;
                wire.complete(id.as_ref())?;
                return Ok(());
            }
            let interrupted = async {
                match &pending {
                    Some(pending) => pending.cancelled.cancelled().await,
                    None => std::future::pending::<()>().await,
                }
            };
            let written = tokio::select! {
                biased;
                () = wire.shutdown.cancelled() => Err(io::Error::new(io::ErrorKind::Interrupted,"MCP_SHUTDOWN")),
                () = interrupted => Err(io::Error::new(io::ErrorKind::Interrupted,"MCP_DELIVERY_CANCELLED")),
                result = writing => result,
            };
            if let Err(error) = written {
                // A write may already be partial. Fail-stop this connection,
                // never append an error to an uncertain JSON-RPC line.
                wire.fail("MCP_OUTPUT_FAILED");
                wire.wait_reaped().await?;
                wire.complete(id.as_ref())?;
                return Err(error);
            }
            wire.complete(id.as_ref())
        }
    }

    fn receive(
        &mut self,
    ) -> impl std::future::Future<Output = Option<RxJsonRpcMessage<RoleServer>>> + Send {
        self.inner.receive()
    }

    fn close(&mut self) -> impl std::future::Future<Output = io::Result<()>> + Send {
        self.inner.close()
    }
}
