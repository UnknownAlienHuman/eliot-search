//! Typed MCP tools and the ordinary-thread owner of synchronous SDK calls.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc as std_mpsc};
use std::thread;
use std::time::{Duration, Instant};

use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, Implementation, JsonObject, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, schemars, tool, tool_handler, tool_router};
use search_provider_client::{ClientError, QueryClient};
use search_provider_protocol::{
    DEFAULT_WINDOW_PAGE_SIZE, DecodedUtf8Request, MAX_CLIENT_FRAME_BYTES, MAX_REMAINING_MS,
    MAX_WINDOW_TTL_MS, ProtocolError, QueryOutcome, RawExactRequest, SearchRequest, SessionOutcome,
    SessionReply, WindowQuery,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{Config, window};

#[derive(Clone)]
pub struct PreparedReply {
    pub result: CallToolResult,
    pub window: Option<String>,
}

pub type Outcome = Result<PreparedReply, &'static str>;

pub fn text_reply(value: &serde_json::Value, failed: bool, window: Option<&str>) -> Outcome {
    let text = serde_json::to_string(value).map_err(|_| "MCP_SERIALIZATION_FAILED")?;
    let content = vec![rmcp::model::ContentBlock::text(text)];
    let result = if failed {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    };
    Ok(PreparedReply {
        result,
        window: window.map(str::to_owned),
    })
}

const fn default_limit() -> u8 {
    5
}
const fn default_timeout() -> u64 {
    MAX_REMAINING_MS
}
const fn default_page() -> u8 {
    DEFAULT_WINDOW_PAGE_SIZE
}
const fn default_ttl() -> u64 {
    MAX_WINDOW_TTL_MS
}
const fn enabled() -> bool {
    true
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SearchArgs {
    /// Tokenized ranked search, not a Boolean or exact-identifier grammar.
    #[schemars(length(min = 1, max = 4096))]
    query: String,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 64))]
    limit: u8,
    #[serde(default = "default_timeout")]
    #[schemars(range(min = 1, max = 120_000))]
    timeout_ms: u64,
    /// Saved published path hint only; live filesystem status is not checked.
    #[serde(default = "enabled")]
    include_source_info: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ExactMode {
    Raw,
    DecodedUtf8,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExactArgs {
    mode: ExactMode,
    /// Exact UTF-8 literal bytes, at most 4096 bytes; no implicit decoding.
    #[schemars(length(min = 1, max = 4096))]
    literal: String,
    /// Unique canonical source UUIDs. Omitted/null selects all; [] selects none.
    #[schemars(extend("uniqueItems" = true))]
    source_ids: Option<Vec<String>>,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 64))]
    limit: u8,
    #[serde(default = "default_timeout")]
    #[schemars(range(min = 1, max = 120_000))]
    timeout_ms: u64,
    #[serde(default = "enabled")]
    include_source_info: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(
    tag = "mode",
    content = "request",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum QueryArgs {
    Search {
        #[schemars(length(min = 1, max = 4096))]
        query: String,
        #[serde(default = "default_limit")]
        #[schemars(range(min = 1, max = 64))]
        limit: u8,
    },
    Raw {
        #[schemars(length(min = 1, max = 4096))]
        literal: String,
        #[schemars(extend("uniqueItems" = true))]
        source_ids: Option<Vec<String>>,
        #[serde(default = "default_limit")]
        #[schemars(range(min = 1, max = 64))]
        limit: u8,
    },
    DecodedUtf8 {
        #[schemars(length(min = 1, max = 4096))]
        literal: String,
        #[schemars(extend("uniqueItems" = true))]
        source_ids: Option<Vec<String>>,
        #[serde(default = "default_limit")]
        #[schemars(range(min = 1, max = 64))]
        limit: u8,
    },
}

impl QueryArgs {
    fn into_core(self) -> Result<WindowQuery, &'static str> {
        let query = match self {
            Self::Search { query, limit } => WindowQuery::Search(SearchRequest { query, limit }),
            Self::Raw {
                literal,
                source_ids,
                limit,
            } => {
                preflight_ids(source_ids.as_deref())?;
                WindowQuery::Raw(RawExactRequest {
                    literal,
                    source_ids,
                    limit,
                })
            }
            Self::DecodedUtf8 {
                literal,
                source_ids,
                limit,
            } => {
                preflight_ids(source_ids.as_deref())?;
                WindowQuery::DecodedUtf8(DecodedUtf8Request {
                    literal,
                    source_ids,
                    limit,
                })
            }
        };
        query.validate().map_err(ProtocolError::code)?;
        Ok(query)
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct OpenArgs {
    query: QueryArgs,
    #[serde(default = "default_page")]
    #[schemars(range(min = 1, max = 64))]
    page_size: u8,
    #[serde(default = "default_ttl")]
    #[schemars(range(min = 1, max = 120_000))]
    ttl_ms: u64,
    #[serde(default = "default_timeout")]
    #[schemars(range(min = 1, max = 120_000))]
    timeout_ms: u64,
    #[serde(default = "enabled")]
    include_source_info: bool,
}

fn open_args_schema() -> Arc<JsonObject> {
    // Match rmcp's input settings and root normalization, with this finite input inlined.
    let schema = schemars::generate::SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<OpenArgs>();
    let serde_json::Value::Object(mut object) =
        serde_json::to_value(schema).expect("failed to serialize OpenArgs schema")
    else {
        panic!("OpenArgs input schema must be an object");
    };
    assert_eq!(
        object.get("type").and_then(serde_json::Value::as_str),
        Some("object"),
        "OpenArgs input schema must have root type object"
    );
    object.remove("title");
    object.remove("description");
    Arc::new(object)
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ContinueArgs {
    /// Same MCP process only; expired/closed handles cannot be reused.
    window_handle: String,
    #[serde(default = "default_page")]
    #[schemars(range(min = 1, max = 64))]
    page_size: u8,
    #[serde(default = "default_timeout")]
    #[schemars(range(min = 1, max = 120_000))]
    timeout_ms: u64,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExpandArgs {
    window_handle: String,
    #[schemars(range(min = 0, max = 63))]
    item_index: u8,
    /// Explicit bytes before the original span; total context must fit 65536 bytes.
    #[schemars(range(min = 0, max = 65536))]
    before_bytes: u64,
    /// Explicit bytes after the original span; UTF-8 endpoints must remain valid.
    #[schemars(range(min = 0, max = 65536))]
    after_bytes: u64,
    #[serde(default = "default_timeout")]
    #[schemars(range(min = 1, max = 120_000))]
    timeout_ms: u64,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ControlArgs {
    window_handle: String,
    #[serde(default = "default_timeout")]
    #[schemars(range(min = 1, max = 120_000))]
    timeout_ms: u64,
}

pub fn budget(timeout_ms: u64) -> Result<Instant, &'static str> {
    if timeout_ms == 0 || timeout_ms > MAX_REMAINING_MS {
        return Err("QUERY_DURATION_INVALID");
    }
    Instant::now()
        .checked_add(Duration::from_millis(timeout_ms))
        .ok_or("QUERY_DEADLINE")
}

pub fn remaining(deadline: Instant) -> Result<u64, &'static str> {
    let left = deadline
        .saturating_duration_since(Instant::now())
        .as_millis();
    if left == 0 {
        return Err("QUERY_DEADLINE");
    }
    u64::try_from(left.min(u128::from(MAX_REMAINING_MS))).map_err(|_| "QUERY_DEADLINE")
}

fn preflight_ids(ids: Option<&[String]>) -> Result<(), &'static str> {
    // Each canonical UUID alone needs 38 JSON bytes including quotes. Anything
    // larger cannot fit the existing core request frame, even before separators.
    if ids.is_some_and(|ids| ids.len() > MAX_CLIENT_FRAME_BYTES / 38) {
        return Err("QUERY_FRAME_TOO_LARGE");
    }
    Ok(())
}

fn error_reply(code: &'static str) -> PreparedReply {
    let content = vec![rmcp::model::ContentBlock::text(format!(
        "{{\"error\":\"{code}\"}}"
    ))];
    PreparedReply {
        result: CallToolResult::error(content),
        window: None,
    }
}

pub fn reply_result(handle: Option<&str>, reply: SessionReply) -> Outcome {
    let failed = matches!(reply.outcome, SessionOutcome::Failed { .. });
    let reply = serde_json::to_value(reply).map_err(|_| "MCP_SERIALIZATION_FAILED")?;
    let value = handle.map_or_else(
        || json!({"reply": &reply}),
        |handle| json!({"window_handle":handle,"reply": &reply}),
    );
    text_reply(&value, failed, handle)
}

enum Query {
    Search(SearchRequest),
    Raw(RawExactRequest),
    Decoded(DecodedUtf8Request),
}

struct QueryJob {
    request: Query,
    args: Vec<std::ffi::OsString>,
    include_source_info: bool,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    reply: oneshot::Sender<Outcome>,
    permit: OwnedSemaphorePermit,
}

struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

struct Shared {
    config: Arc<Config>,
    shutdown: CancellationToken,
    admission: Arc<Semaphore>,
    query: std_mpsc::SyncSender<QueryJob>,
    windows: Arc<window::Hub>,
    wire: Arc<crate::wire::Wire>,
}

#[derive(Clone)]
pub struct McpServer {
    shared: Arc<Shared>,
    tool_router: ToolRouter<Self>,
}

pub struct Owners {
    shutdown: CancellationToken,
    active_query: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    query: thread::JoinHandle<Result<(), &'static str>>,
    window: tokio::task::JoinHandle<Result<(), &'static str>>,
}

impl Owners {
    pub async fn finish(self) -> Result<(), &'static str> {
        self.shutdown.cancel();
        let (cancel, poisoned) = match self.active_query.lock() {
            Ok(state) => (state.clone(), false),
            Err(state) => (state.into_inner().clone(), true),
        };
        if let Some(cancel) = cancel {
            cancel.store(true, Ordering::Release);
        }
        // Join both owners even when either failed. The ordinary thread owns its
        // own runtime; joining after the async window owner stops cannot starve it.
        let window = self.window.await;
        let query = self.query.join();
        if poisoned {
            return Err("MCP_OWNER_STATE_CORRUPT");
        }
        let window = window.map_err(|_| "MCP_WINDOW_OWNER_PANICKED")?;
        let query = query.map_err(|_| "MCP_QUERY_OWNER_PANICKED")?;
        window?;
        query
    }
}

fn query_worker(
    config: &Config,
    receiver: &std_mpsc::Receiver<QueryJob>,
    active: &Mutex<Option<Arc<AtomicBool>>>,
    shutdown: &CancellationToken,
) -> Result<(), &'static str> {
    while !shutdown.is_cancelled() {
        let QueryJob {
            request,
            args,
            include_source_info,
            deadline,
            cancelled,
            reply,
            permit,
        } = match receiver.recv_timeout(Duration::from_millis(10)) {
            Ok(job) => job,
            Err(std_mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std_mpsc::RecvTimeoutError::Disconnected) => break,
        };
        *active.lock().map_err(|_| "MCP_OWNER_STATE_CORRUPT")? = Some(cancelled.clone());
        if shutdown.is_cancelled() {
            cancelled.store(true, Ordering::Release);
        }
        let completion = (|| {
            let mut client = QueryClient::new()?;
            client.set_include_source_info(include_source_info);
            match request {
                Query::Search(request) => {
                    client.query_owned_child(&config.daemon, &args, request, deadline, &cancelled)
                }
                Query::Raw(request) => client.raw_exact_owned_child(
                    &config.daemon,
                    &args,
                    request,
                    deadline,
                    &cancelled,
                ),
                Query::Decoded(request) => client.decoded_utf8_owned_child(
                    &config.daemon,
                    &args,
                    request,
                    deadline,
                    &cancelled,
                ),
            }
        })();
        *active.lock().map_err(|_| "MCP_OWNER_STATE_CORRUPT")? = None;
        let fatal = completion
            .as_ref()
            .err()
            .is_some_and(|e| matches!(e, ClientError::Wait | ClientError::Cleanup));
        let outcome = completion
            .map_err(ClientError::code)
            .and_then(|completion| {
                let failed = matches!(completion.terminal.outcome, QueryOutcome::Failed { .. });
                // Keep all actual evidence items/citations/hints and the pinned
                // terminal/summary. This is not a statistics-only projection.
                let value = serde_json::to_value(completion.terminal)
                    .map_err(|_| "MCP_SERIALIZATION_FAILED")?;
                text_reply(&value, failed, None)
            });
        drop(permit);
        let _ = reply.send(outcome);
        if fatal {
            shutdown.cancel();
            return Err("MCP_QUERY_CLEANUP_FAILED");
        }
    }
    Ok(())
}

impl McpServer {
    pub fn start(
        config: Config,
        wire: Arc<crate::wire::Wire>,
    ) -> Result<(Self, Owners), &'static str> {
        let config = Arc::new(config);
        let epoch = crate::wire::fresh_epoch()?;
        let shutdown = wire.shutdown.clone();
        let admission = Arc::new(Semaphore::new(1));
        let active_query = Arc::new(Mutex::new(None));
        let (sender, receiver) = std_mpsc::sync_channel(1);
        let thread_config = config.clone();
        let thread_shutdown = shutdown.clone();
        let thread_active = active_query.clone();
        let query = thread::Builder::new()
            .name("eliot-mcp-query".into())
            .spawn(move || {
                let result =
                    query_worker(&thread_config, &receiver, &thread_active, &thread_shutdown);
                if result.is_err() {
                    thread_shutdown.cancel();
                }
                result
            })
            .map_err(|_| "MCP_QUERY_THREAD_START_FAILED")?;
        let (windows, window) = window::Hub::start(config.clone(), shutdown.clone(), epoch);
        let shared = Arc::new(Shared {
            config,
            shutdown: shutdown.clone(),
            admission,
            query: sender,
            windows,
            wire,
        });
        Ok((
            Self {
                shared,
                tool_router: Self::tool_router(),
            },
            Owners {
                shutdown,
                active_query,
                query,
                window,
            },
        ))
    }

    async fn close_delivery_window(&self, handle: Option<&str>) -> Result<(), &'static str> {
        if let Some(handle) = handle
            && let Some(binding) = self.shared.windows.delivery_binding(handle)?
        {
            binding.finish(&self.shared.shutdown).await?;
        }
        Ok(())
    }

    async fn finish_result(
        &self,
        outcome: Outcome,
        context: &RequestContext<RoleServer>,
        permit: Option<OwnedSemaphorePermit>,
    ) -> CallToolResult {
        let mut prepared = outcome.unwrap_or_else(error_reply);
        let mut permit = permit;
        let cancelled = context.ct.is_cancelled();
        let fits =
            crate::wire::result_fits(&prepared.result, &context.id, context.protocol_version());
        if cancelled || !matches!(fits, Ok(true)) {
            let cleanup = self.close_delivery_window(prepared.window.as_deref()).await;
            drop(permit.take());
            prepared = match cleanup {
                Err(code) => {
                    self.shared.wire.fail("MCP_OWNER_CLEANUP_FAILED");
                    error_reply(code)
                }
                Ok(()) if cancelled => error_reply("MCP_REQUEST_CANCELLED"),
                Ok(()) if fits.is_err() => error_reply("MCP_RPC_SERIALIZATION_FAILED"),
                Ok(()) => error_reply("MCP_RESPONSE_TOO_LARGE"),
            };
        }
        if let (Some(handle), Some(permit)) = (prepared.window.as_deref(), permit) {
            match self.shared.windows.delivery_binding(handle) {
                Ok(Some(binding)) => {
                    if self
                        .shared
                        .wire
                        .bind(context.id.clone(), binding, permit)
                        .is_err()
                    {
                        let cleanup = self.close_delivery_window(Some(handle)).await;
                        self.shared.wire.fail("MCP_DELIVERY_STATE_CORRUPT");
                        prepared =
                            error_reply(cleanup.err().unwrap_or("MCP_DELIVERY_STATE_CORRUPT"));
                    }
                }
                Ok(None) => {}
                Err(code) => {
                    self.shared.wire.fail("MCP_OWNER_STATE_CORRUPT");
                    prepared = error_reply(code);
                }
            }
        }
        if !matches!(
            crate::wire::result_fits(&prepared.result, &context.id, context.protocol_version()),
            Ok(true)
        ) {
            // Even an honest error cannot be sent with an unencodable ID.
            // The final transport guard closes only after product cleanup.
            self.shared.wire.fail("MCP_RESPONSE_TOO_LARGE");
        }
        prepared.result
    }

    async fn one_shot(
        &self,
        request: Query,
        include_source_info: bool,
        deadline: Instant,
        context: &RequestContext<RoleServer>,
    ) -> Outcome {
        if self.shared.shutdown.is_cancelled() || context.ct.is_cancelled() {
            return Err("MCP_REQUEST_CANCELLED");
        }
        let ranked = matches!(request, Query::Search(_));
        let args = self.shared.config.arguments(false, ranked)?;
        let permit = self
            .shared
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| "MCP_ROOT_BUSY")?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let _cancel_on_drop = CancelOnDrop(cancelled.clone());
        let (reply, mut answer) = oneshot::channel();
        self.shared
            .query
            .try_send(QueryJob {
                request,
                args,
                include_source_info,
                deadline,
                cancelled: cancelled.clone(),
                reply,
                permit,
            })
            .map_err(|error| match error {
                std_mpsc::TrySendError::Full(_) => "MCP_QUERY_OWNER_BUSY",
                std_mpsc::TrySendError::Disconnected(_) => {
                    self.shared.shutdown.cancel();
                    "MCP_QUERY_OWNER_STOPPED"
                }
            })?;
        tokio::select! {
            biased;
            () = context.ct.cancelled() => {
                cancelled.store(true, Ordering::Release);
                let _ = answer.await;
                Err("MCP_REQUEST_CANCELLED")
            }
            value = &mut answer => value.map_err(|_| "MCP_QUERY_OWNER_STOPPED")?,
        }
    }
}

#[tool_router(router = tool_router)]
impl McpServer {
    #[tool(
        name = "eliot_search",
        description = "Ranked retained evidence with full verified text/citations, source hints and corpus summary. Default 5 hits; ranking is not exhaustive absence.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let outcome = async {
            let deadline = budget(args.timeout_ms)?;
            let request = SearchRequest {
                query: args.query,
                limit: args.limit,
            };
            request.validate().map_err(ProtocolError::code)?;
            self.one_shot(
                Query::Search(request),
                args.include_source_info,
                deadline,
                &context,
            )
            .await
        }
        .await;
        self.finish_result(outcome, &context, None).await
    }

    #[tool(
        name = "eliot_exact",
        description = "Literal Raw or saved DecodedUtf8 evidence; full scan/completeness facts and byte-exact items. Omitted sources means all published; [] means none.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn exact(
        &self,
        Parameters(args): Parameters<ExactArgs>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let outcome = async {
            let deadline = budget(args.timeout_ms)?;
            preflight_ids(args.source_ids.as_deref())?;
            let request = match args.mode {
                ExactMode::Raw => {
                    let request = RawExactRequest {
                        literal: args.literal,
                        source_ids: args.source_ids,
                        limit: args.limit,
                    };
                    request.validate().map_err(ProtocolError::code)?;
                    Query::Raw(request)
                }
                ExactMode::DecodedUtf8 => {
                    let request = DecodedUtf8Request {
                        literal: args.literal,
                        source_ids: args.source_ids,
                        limit: args.limit,
                    };
                    request.validate().map_err(ProtocolError::code)?;
                    Query::Decoded(request)
                }
            };
            self.one_shot(request, args.include_source_info, deadline, &context)
                .await
        }
        .await;
        self.finish_result(outcome, &context, None).await
    }

    #[tool(
        name = "eliot_window_open",
        description = r#"Open one retained query window, returning its first evidence page and an ephemeral handle. query is a mode/request object, not a string. Example: {"query":{"mode":"search","request":{"query":"TEXT","limit":1}},"page_size":1}. TTL starts before work, never renews, and is at most 120000ms."#,
        input_schema = open_args_schema(),
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn window_open(
        &self,
        Parameters(args): Parameters<OpenArgs>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let mut delivery = None;
        let outcome = async {
            let deadline = budget(args.timeout_ms)?;
            let query = args.query.into_core()?;
            if args.page_size == 0
                || args.page_size > 64
                || args.ttl_ms == 0
                || args.ttl_ms > MAX_WINDOW_TTL_MS
            {
                return Err("QUERY_WINDOW_BOUNDS_INVALID");
            }
            let permit = self
                .shared
                .admission
                .clone()
                .try_acquire_owned()
                .map_err(|_| "MCP_ROOT_BUSY")?;
            delivery = Some(self.shared.wire.acquire_delivery()?);
            self.shared
                .windows
                .open(
                    window::Open {
                        query,
                        page_size: args.page_size,
                        ttl_ms: args.ttl_ms,
                        include_source_info: args.include_source_info,
                        deadline,
                        permit,
                    },
                    &context.ct,
                )
                .await
        }
        .await;
        self.finish_result(outcome, &context, delivery).await
    }

    #[tool(
        name = "eliot_window_continue",
        description = "Next page from the original retained prefix only; never requery or repin. A window stores at most its original 64-item limit.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn window_continue(
        &self,
        Parameters(args): Parameters<ContinueArgs>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let mut delivery = None;
        let outcome = async {
            let deadline = budget(args.timeout_ms)?;
            delivery = Some(self.shared.wire.acquire_delivery()?);
            self.shared
                .windows
                .data(
                    &args.window_handle,
                    window::Data::Continue {
                        page_size: args.page_size,
                    },
                    deadline,
                    &context.ct,
                )
                .await
        }
        .await;
        self.finish_result(outcome, &context, delivery).await
    }

    #[tool(
        name = "eliot_window_expand",
        description = "Verified retained context for an already emitted item. Explicit before/after; total including original span <=65536 bytes. No live-file reads.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn window_expand(
        &self,
        Parameters(args): Parameters<ExpandArgs>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let mut delivery = None;
        let outcome = async {
            let deadline = budget(args.timeout_ms)?;
            delivery = Some(self.shared.wire.acquire_delivery()?);
            self.shared
                .windows
                .data(
                    &args.window_handle,
                    window::Data::Expand {
                        item_index: args.item_index,
                        before_bytes: args.before_bytes,
                        after_bytes: args.after_bytes,
                    },
                    deadline,
                    &context.ct,
                )
                .await
        }
        .await;
        self.finish_result(outcome, &context, delivery).await
    }

    #[tool(
        name = "eliot_window_cancel",
        description = "Cancel correlated active window work, preserving healthy previously emitted context when the core does so. Reports the actual observed outcome; does not close an idle window.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn window_cancel(
        &self,
        Parameters(args): Parameters<ControlArgs>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let outcome = async {
            self.shared
                .windows
                .cancel(&args.window_handle, budget(args.timeout_ms)?, &context.ct)
                .await
        }
        .await;
        self.finish_result(outcome, &context, None).await
    }

    #[tool(
        name = "eliot_window_close",
        description = "Release the window/root and finish/reap its owned child. Unlike window_cancel, Close releases all context.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn window_close(
        &self,
        Parameters(args): Parameters<ControlArgs>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let outcome = async {
            self.shared
                .windows
                .close(&args.window_handle, budget(args.timeout_ms)?, &context.ct)
                .await
        }
        .await;
        self.finish_result(outcome, &context, None).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    async fn on_cancelled(
        &self,
        notification: rmcp::model::CancelledNotificationParam,
        _context: rmcp::service::NotificationContext<RoleServer>,
    ) {
        if let Some(id) = &notification.request_id {
            self.shared.wire.cancel_delivery(id).await;
        }
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::ListToolsResult, rmcp::ErrorData>>
    + Send
    + '_ {
        let supports_cache_hints = context
            .protocol_version()
            .is_some_and(|version| version >= rmcp::model::ProtocolVersion::V_2026_07_28);
        let mut result = rmcp::model::ListToolsResult::with_all_items(self.tool_router.list_all());
        if supports_cache_hints {
            result.ttl_ms = Some(0);
            result.cache_scope = Some(rmcp::model::CacheScope::Public);
        }
        std::future::ready(Ok(result))
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("eliot-search-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("Use small limits (default 5). Evidence is retained/published, not live filesystem truth. Explicitly close windows; handles expire and cannot be replayed after reconnect.")
    }
}
