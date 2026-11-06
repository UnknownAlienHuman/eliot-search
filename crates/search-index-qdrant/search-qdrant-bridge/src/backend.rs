//! One private channel/runtime with bounded generated-client RPCs.

use std::fmt;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use prost::Message;
use qdrant_client::qdrant::{
    HealthCheckRequest, ListCollectionsRequest, collections_client::CollectionsClient,
    points_client::PointsClient, qdrant_client::QdrantClient,
};
use tokio::runtime::{Builder, Handle, Runtime};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{Code, Request, Response, Status};

use crate::{BackendError, BackendLimits};

mod collections;
mod points;
mod query;

/// Concrete vectorless backend; SDK/transport types and credentials stay private.
///
/// The reusable channel is dropped before its one runtime. The operator owns
/// server startup and shutdown. Publication owns fixed input and uncertain
/// effects; this value has no mutation ledger or current-generation state.
pub struct Backend {
    channel: Channel,
    api_key: Option<MetadataValue<Ascii>>,
    limits: BackendLimits,
    runtime: Runtime,
}

impl Backend {
    /// Connects to an explicit loopback gRPC URL and verifies server/authentication.
    ///
    /// The selected server patch is exactly 1.20.0. This does not prove index
    /// readiness or adopt/rebuild any collection. No discovery or auth retry occurs.
    ///
    /// # Errors
    ///
    /// Returns endpoint/limit/credential, transport/authentication/version,
    /// cancellation or deadline failures. Calling from an async runtime is refused.
    pub fn connect(
        endpoint: &str,
        api_key: Option<&str>,
        limits: BackendLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, BackendError> {
        synchronous()?;
        check(deadline, cancelled)?;
        limits.validate()?;
        let tls = validate_endpoint(endpoint)?;
        let api_key = api_key
            .map(|value| {
                if value.is_empty() || value.len() > 4096 {
                    return Err(BackendError::InvalidInput);
                }
                value.parse().map_err(|_| BackendError::InvalidInput)
            })
            .transpose()?;
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("eliot-qdrant")
            .enable_all()
            .build()
            .map_err(|_| BackendError::Transport)?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(BackendError::Deadline)?;
        let mut transport = Endpoint::from_shared(endpoint.to_owned())
            .map_err(|_| BackendError::InvalidEndpoint)?
            .connect_timeout(remaining);
        if tls {
            transport = transport
                .tls_config(ClientTlsConfig::new().with_native_roots())
                .map_err(|_| BackendError::Transport)?;
        }
        let channel = run(&runtime, deadline, cancelled, false, async {
            transport
                .connect()
                .await
                .map_err(|_| BackendError::Transport)
        })?;
        let backend = Self {
            channel,
            api_key,
            limits,
            runtime,
        };
        backend.verify_connection(deadline, cancelled)?;
        Ok(backend)
    }

    fn verify_connection(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), BackendError> {
        let mut root = QdrantClient::new(self.channel.clone())
            .max_encoding_message_size(self.limits.max_request_bytes)
            .max_decoding_message_size(self.limits.max_response_bytes);
        let request = self.request(HealthCheckRequest {}, deadline, cancelled)?;
        let reply = self.call(root.health_check(request), deadline, cancelled, false)?;
        if reply.version != "1.20.0" {
            return Err(BackendError::UnsupportedServer);
        }
        let mut collections = self.collections();
        let request = self.request(ListCollectionsRequest {}, deadline, cancelled)?;
        self.call(collections.list(request), deadline, cancelled, false)?;
        Ok(())
    }

    fn collections(&self) -> CollectionsClient<Channel> {
        CollectionsClient::new(self.channel.clone())
            .max_encoding_message_size(self.limits.max_request_bytes)
            .max_decoding_message_size(self.limits.max_response_bytes)
    }

    fn points(&self) -> PointsClient<Channel> {
        PointsClient::new(self.channel.clone())
            .max_encoding_message_size(self.limits.max_request_bytes)
            .max_decoding_message_size(self.limits.max_response_bytes)
    }

    fn request<T: Message>(
        &self,
        message: T,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Request<T>, BackendError> {
        check(deadline, cancelled)?;
        if message.encoded_len() > self.limits.max_request_bytes {
            return Err(BackendError::RequestTooLarge);
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(BackendError::Deadline)?;
        let mut request = Request::new(message);
        request.set_timeout(remaining);
        if let Some(key) = &self.api_key {
            request.metadata_mut().insert("api-key", key.clone());
        }
        Ok(request)
    }

    fn call<T>(
        &self,
        future: impl Future<Output = Result<Response<T>, Status>>,
        deadline: Instant,
        cancelled: &AtomicBool,
        mutation: bool,
    ) -> Result<T, BackendError> {
        synchronous()?;
        run(&self.runtime, deadline, cancelled, mutation, async {
            future
                .await
                .map(Response::into_inner)
                .map_err(|status| read_error(&status))
        })
    }
}

impl fmt::Debug for Backend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Backend")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

fn validate_endpoint(endpoint: &str) -> Result<bool, BackendError> {
    if endpoint.len() > 256 {
        return Err(BackendError::InvalidEndpoint);
    }
    let (scheme, authority) = endpoint
        .split_once("://")
        .ok_or(BackendError::InvalidEndpoint)?;
    if !matches!(scheme, "http" | "https") {
        return Err(BackendError::InvalidEndpoint);
    }
    let socket: SocketAddr = authority
        .parse()
        .map_err(|_| BackendError::InvalidEndpoint)?;
    if !socket.ip().is_loopback() || socket.port() == 0 {
        return Err(BackendError::InvalidEndpoint);
    }
    Ok(scheme == "https")
}

fn synchronous() -> Result<(), BackendError> {
    if Handle::try_current().is_ok() {
        return Err(BackendError::InvalidInput);
    }
    Ok(())
}

pub fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), BackendError> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(BackendError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(BackendError::Deadline);
    }
    Ok(())
}

fn run<T>(
    runtime: &Runtime,
    deadline: Instant,
    cancelled: &AtomicBool,
    mutation: bool,
    future: impl Future<Output = Result<T, BackendError>>,
) -> Result<T, BackendError> {
    check(deadline, cancelled)?;
    runtime.block_on(async {
        let mut future = std::pin::pin!(future);
        let mut dispatched = false;
        loop {
            if let Err(error) = check(deadline, cancelled) {
                return Err(stopped(error, mutation, dispatched));
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(stopped(BackendError::Deadline, mutation, dispatched));
            };
            dispatched = true;
            match tokio::time::timeout(remaining.min(Duration::from_millis(10)), future.as_mut())
                .await
            {
                Ok(Ok(value)) => return Ok(value),
                Ok(Err(error)) => return Err(stopped(error, mutation, true)),
                Err(_) => {}
            }
        }
    })
}

const fn stopped(error: BackendError, mutation: bool, dispatched: bool) -> BackendError {
    if mutation && dispatched {
        BackendError::UnknownDispatch
    } else {
        error
    }
}

fn read_error(status: &Status) -> BackendError {
    match status.code() {
        Code::Unauthenticated | Code::PermissionDenied => BackendError::Authentication,
        Code::Cancelled => BackendError::Cancelled,
        Code::DeadlineExceeded => BackendError::Deadline,
        Code::Unavailable | Code::Unknown => BackendError::Transport,
        _ => BackendError::BackendRefused,
    }
}
