# W8 reentry — `eliot-search` standalone client

**Status:** package-stage delta only; W8 implementation remains blocked.  
**Base handoff:** exact accepted W1 CLI shell public API/commit.  
**Write owner:** one `eliot-search` package agent.

The W8 agent extends the thin local client from shell/admin connectivity into the generic authenticated
Search client. It still performs no direct redb/CAS/Qdrant/source access and creates no server authority.

## Reentry scope

The agent reads only the W1 public handoff, package assignment/base `FUNCTIONS.md`, this file, the W8
stage/configuration/qualification packets and exact accepted provider-protocol/daemon handoffs.

It does not reread W1 implementation internals, the architecture master, adapters or another package.
The cumulative package line budget is preserved; W8 does not reset it.

## Operations

### `parse_client_invocation(args, environment, limits) -> Result<ClientInvocation, ClientError>`

Parses the closed command/recipe set, explicit scope/view/budget/output mode, pairing/binding action and
relative deadline. Unknown flags, raw Qdrant filters, point IDs, unrestricted paths, plaintext secrets,
provider-specific options and zero-as-unlimited limits fail closed.

### `resolve_local_endpoint(invocation, platform) -> Result<LocalEndpointDescriptor, ClientError>`

Resolves only the configured local provider endpoint and expected installation/profile identity. It does
not scan ports/pipes, attach to any responding daemon or infer trust from same-user/ACL alone.

### `pair_and_bind(endpoint, request, protocol, secret_prompt, context) -> Result<ClientBindingReceipt, ClientError>`

Executes the provider-owned mutual hello, one-time pairing challenge/proof and durable binding flow.
Pairing proof is entered through a guarded prompt/callback and never stored in CLI history, argv,
environment, logs or receipts.

Success returns opaque binding/installation/capability identities only. The client cannot generate or
sign a Search grant.

### `open_session(endpoint, binding, protocol, deadline, cancel) -> Result<ClientSession, ClientError>`

Binds exact provider/client installation incarnations, protocol version, connection sequence and finite
frame/in-flight limits. Replay, role substitution, stale binding and mismatched daemon identity fail
before recipe dispatch.

### `fetch_capabilities(session, request) -> Result<CapabilitySnapshot, ClientError>`

Obtains the binding-filtered descriptor and verifies descriptor/schema/generation consistency. It is a
planning hint only, never authorization; every request/expansion is rechecked by the server.

The CLI does not display foreign membership names/counts/readiness hidden by the descriptor.

### `build_recipe_request(invocation, capabilities) -> Result<ProviderRequest, ClientError>`

Builds only accepted closed recipe versions and bounded request records. It preserves explicit requested
scope, source/workspace view, disclosure ceiling, budget, deadline and client operation ID.

Capability absence produces a typed local error/gap; it does not switch recipes/providers or widen scope.

### `execute_request(session, request, renderer, cancel) -> Result<ClientTerminal, ClientError>`

Sends one request, processes monotonic progress and accepts exactly one terminal response. It preserves
server result variants, coverage, freshness, assurance, denominator kind, ambiguity, omissions and
reason codes.

Cancellation is idempotent. Disconnect/timeout after dispatch is reported as cancelled/expired/
unavailable according to protocol evidence; the CLI never fabricates a successful terminal result.

### `render_progress(event, output_mode, disclosure) -> Result<(), ClientError>`

Renders bounded content-minimized progress without claiming completion, exposing secrets/foreign scope
or treating a progressive card as the terminal answer.

### `render_terminal(result, output_mode, disclosure) -> Result<RenderReceipt, ClientError>`

Renders compact cards/handles and all material gaps. JSON output uses the accepted public schema and
never serializes opaque token internals, raw provider frames or unrestricted absolute paths.

`PARTIAL`, `DEGRADED`, `AMBIGUOUS`, `NO_MATCH_IN_INCOMPLETE_SCOPE`, `UNAUTHORIZED`, `UNAVAILABLE`,
`CANCELLED`, `EXPIRED` and `FAILED` remain distinguishable.

### `expand_handle(session, handle, request, cancel) -> Result<ClientTerminal, ClientError>`

Sends an ordinary server-authorized `expand_handle@1` request. The client treats the handle as opaque,
does not decode it and cannot use possession to bypass current binding/grant/source state.

### `classify_exit_status(terminal_or_error) -> ClientExitStatus`

Returns stable distinct process statuses for success, truthful partial/degraded result, ambiguity,
unauthorized, unavailable, cancelled, expired, invalid invocation/protocol and internal failure.

A partial result is not exit-success unless the documented machine-consumer mode explicitly defines a
separate partial status; it is never collapsed into complete success.

### `close_session(session, deadline) -> ClientDisconnectReceipt`

Stops new requests, cancels/drains bounded local in-flight work and closes the protocol connection.
Server-owned durable operations remain governed by their operation identities; the client claims only
local cleanup.

## Configuration and capability interaction

The W8 CLI consumes `config/w8-client-edge.toml` only through the accepted client configuration surface.
Profile/adapter presence in config never activates a server capability. The client reports the exact
binding-filtered descriptor and handler unavailability.

Optional ELIOT/Research mappings are separate server leaf profiles. The standalone CLI cannot invoke
memory/admission/finish authority, write back to ELIOT/Research or turn an export into source ownership.

## Cancellation, retry and crash semantics

Parsing/rendering are pure. Pairing/session/request operations use finite deadlines and cancellation.
Request retry creates a new protocol attempt linked to the same caller operation only when the server
contract permits it; the CLI never retries until success or drops the first failed attempt.

Process crash loses local session/progress state. On restart the client reopens a fresh authenticated
session and never assumes an in-flight request succeeded. Opaque durable handles/bindings are reused only
through normal server validation.

## Typed failures

- `CLIENT_INVOCATION_INVALID`
- `CLIENT_ENDPOINT_NOT_FOUND`
- `CLIENT_ENDPOINT_IDENTITY_MISMATCH`
- `CLIENT_PAIRING_REQUIRED`
- `CLIENT_PAIRING_REJECTED`
- `CLIENT_BINDING_EXPIRED_OR_REVOKED`
- `CLIENT_PROTOCOL_MISMATCH`
- `CLIENT_REPLAY_REJECTED`
- `CLIENT_CAPABILITY_UNAVAILABLE`
- `CLIENT_RECIPE_UNSUPPORTED`
- `CLIENT_REQUEST_REJECTED`
- `CLIENT_UNAUTHORIZED`
- `CLIENT_PARTIAL_RESULT`
- `CLIENT_AMBIGUOUS_RESULT`
- `CLIENT_UNAVAILABLE`
- `CLIENT_CANCELLED`
- `CLIENT_DEADLINE_EXCEEDED`
- `CLIENT_DISCONNECTED`
- `CLIENT_RENDER_FAILED`
- `CLIENT_TERMINAL_DUPLICATE`

## Required tests / qualification evidence

- base W1 CLI handoff/API digest required before W8 changes;
- exact command/recipe/version/budget parsing and unknown/load-bearing-field rejection;
- no direct store, source, Qdrant, secret-store or grant-construction dependency;
- endpoint identity and no scan/attach-to-any-responder behavior;
- pairing proof absent from argv/history/environment/log/error/receipt fixtures;
- mutual hello, stale incarnation, replay, role-substitution and binding revoke/expiry fixtures;
- descriptor is not authorization and hides foreign scope metadata;
- exact recipe request canonicalization and no raw Qdrant filter/point/cursor;
- progress monotonicity and exactly one terminal result;
- cancellation/disconnect/deadline and no retry-until-pass behavior;
- handle opacity and live expansion authorization;
- result/exit-status preservation for partial/degraded/ambiguous/unauthorized/unavailable/cancelled/expired;
- JSON/text redaction and no opaque token internals/content/path leakage;
- process restart loses session state and cannot infer prior request success;
- cumulative package line count and package-only diff guard.
